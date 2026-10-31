// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Reading an uncompressed PDF (R's `pdf(compress = FALSE)` or ours with
//! `.compress(false)`) into what it paints, and comparing two of
//! them. Shared by the `*_matches_r` tests.
//!
//! The reader follows the graphics state (`q`/`Q`, colours, alpha via
//! `gs`, line width, cap, join, dash, the clip rectangle from `W n`) and
//! records every painted path and every text item in order. It is strict:
//! an operator it doesn't know that would change what is drawn panics,
//! rather than being skipped.

use std::collections::HashMap;

/// A PDF's pages, in order.
#[derive(Debug)]
pub struct Pdf {
    pub pages: Vec<Page>,
}

/// What one page paints, in order.
#[derive(Debug, Clone)]
pub struct Page {
    /// The page size in points (`/MediaBox`).
    pub size: (f64, f64),
    pub paths: Vec<Path>,
    pub texts: Vec<Text>,
}

/// A painted path.
#[derive(Debug, Clone)]
pub struct Path {
    /// The painting operator (`S`, `f`, `B`, ...).
    pub op: String,
    /// Points in order; `re` gives its four corners with a positive width
    /// and height, a curve (`c`) its three points.
    pub vertices: Vec<(f64, f64)>,
    /// Whether the path has a curve.
    pub curved: bool,
    /// The fill colour (RGBA, 0 to 1), if filled.
    pub fill: Option<[f64; 4]>,
    /// The stroke, if stroked.
    pub stroke: Option<Stroke>,
    /// The clip rectangle `[x0, y0, x1, y1]` in effect.
    pub clip: [f64; 4],
}

/// How a path is stroked.
#[derive(Debug, Clone, PartialEq)]
pub struct Stroke {
    /// Colour (RGBA, 0 to 1).
    pub col: [f64; 4],
    pub lwd: f64,
    pub cap: i32,
    pub join: i32,
    /// Mitre limit (`M`).
    pub mitre: f64,
    pub dash: Vec<f64>,
}

/// A piece of a `TJ` array.
#[derive(Debug, Clone, PartialEq)]
pub enum Piece {
    Str(String),
    Kern(f64),
}

/// A text item (`Tj` or `TJ`).
#[derive(Debug, Clone)]
pub struct Text {
    /// The string, each byte as a Latin-1 character (escapes decoded; a
    /// `TJ` array's pieces joined).
    pub string: String,
    /// The `TJ` pieces with their kerns; one `Str` for `Tj`.
    pub pieces: Vec<Piece>,
    /// Base font name (`Helvetica-Bold`, ...).
    pub font: String,
    /// Size in points (the `Tf` size times the text matrix's scale).
    pub size: f64,
    /// The text matrix.
    pub matrix: [f64; 6],
    /// The fill colour (RGBA, 0 to 1).
    pub fill: [f64; 4],
    /// The clip rectangle in effect.
    pub clip: [f64; 4],
}

impl Path {
    /// `[x0, y0, x1, y1]` of the vertices.
    pub fn bbox(&self) -> [f64; 4] {
        bbox(&self.vertices)
    }

    /// Whether this is a fill-only path covering (nearly) the whole page
    /// width: a page background.
    pub fn is_page_background(&self, page: (f64, f64)) -> bool {
        let b = self.bbox();
        self.stroke.is_none() && b[2] - b[0] >= 0.99 * page.0
    }
}

impl Text {
    pub fn origin(&self) -> (f64, f64) {
        (self.matrix[4], self.matrix[5])
    }

    /// Rotation in degrees.
    pub fn angle(&self) -> f64 {
        self.matrix[1].atan2(self.matrix[0]).to_degrees()
    }
}

impl Pdf {
    /// The only page; panics if there are several.
    pub fn page(self) -> Page {
        assert_eq!(self.pages.len(), 1, "expected one page");
        self.pages.into_iter().next().unwrap()
    }
}

impl Page {
    /// Drops page backgrounds (see [`Path::is_page_background`]).
    pub fn without_background(mut self) -> Page {
        let size = self.size;
        self.paths.retain(|p| !p.is_page_background(size));
        self
    }
}

pub fn bbox(v: &[(f64, f64)]) -> [f64; 4] {
    let xs = v.iter().map(|p| p.0);
    let ys = v.iter().map(|p| p.1);
    [
        xs.clone().fold(f64::INFINITY, f64::min),
        ys.clone().fold(f64::INFINITY, f64::min),
        xs.fold(f64::NEG_INFINITY, f64::max),
        ys.fold(f64::NEG_INFINITY, f64::max),
    ]
}

#[derive(Debug, PartialEq)]
enum Tok {
    Str(String),
    Word(String),
}

/// Tokens of a content stream (one `char` per byte, as [`latin1`] gives).
/// `(...)` strings are one token each, with `\` escapes (octal codes,
/// `\(`, `\)`, `\\`) decoded.
fn tokens(content: &str) -> Vec<Tok> {
    let b: Vec<u8> = content
        .chars()
        .map(|c| u8::try_from(c).expect("not Latin-1"))
        .collect();
    let b = &b[..];
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'(' => {
                let mut s = String::new();
                let mut depth = 0;
                i += 1;
                while i < b.len() && (b[i] != b')' || depth > 0) {
                    match b[i] {
                        b'\\' => {
                            i += 1;
                            let n = b[i..]
                                .iter()
                                .take(3)
                                .take_while(|c| (b'0'..=b'7').contains(c))
                                .count();
                            if n > 0 {
                                let code = std::str::from_utf8(&b[i..i + n]).unwrap();
                                s.push(u8::from_str_radix(code, 8).unwrap() as char);
                                i += n;
                                continue;
                            }
                            let c = match b[i] {
                                b'n' => '\n',
                                b'r' => '\r',
                                b't' => '\t',
                                c => c as char,
                            };
                            s.push(c);
                        }
                        b'(' => {
                            depth += 1;
                            s.push('(');
                        }
                        b')' => {
                            depth -= 1;
                            s.push(')');
                        }
                        c => s.push(c as char),
                    }
                    i += 1;
                }
                i += 1;
                out.push(Tok::Str(s));
            }
            b'[' | b']' => {
                out.push(Tok::Word((b[i] as char).to_string()));
                i += 1;
            }
            c if c.is_ascii_whitespace() => i += 1,
            _ => {
                let start = i;
                while i < b.len() && !b[i].is_ascii_whitespace() && !b"([]".contains(&b[i]) {
                    i += 1;
                }
                out.push(Tok::Word(b[start..i].iter().map(|&c| c as char).collect()));
            }
        }
    }
    out
}

/// The file as Latin-1 text (PDF syntax is ASCII; strings may hold any
/// byte).
fn latin1(pdf: &[u8]) -> String {
    pdf.iter().map(|&c| c as char).collect()
}

/// The body of object `num` (between `num 0 obj` and `endobj`).
fn object<'a>(pdf: &'a str, num: &str) -> &'a str {
    let header = format!("\n{num} 0 obj");
    let start = pdf
        .find(&header)
        .unwrap_or_else(|| panic!("object {num} not found"));
    let body = &pdf[start + header.len()..];
    &body[..body.find("endobj").unwrap()]
}

/// Font resource name -> base font: from R's font objects (`/Name /F2
/// /BaseFont /Helvetica-Bold`), else ours, which are fixed.
fn base_fonts(pdf: &str) -> HashMap<String, String> {
    let mut map: HashMap<String, String> = [
        ("/F1", "Helvetica"),
        ("/F2", "Helvetica-Bold"),
        ("/F3", "Helvetica-Oblique"),
        ("/F4", "Helvetica-BoldOblique"),
        ("/F5", "Symbol"),
    ]
    .into_iter()
    .map(|(a, b)| (a.to_string(), b.to_string()))
    .collect();
    if pdf.contains("/Name /F") {
        map.clear();
        for part in pdf.split("/Name ").skip(1) {
            let mut w = part.split_whitespace();
            if let (Some(name), Some("/BaseFont"), Some(base)) = (w.next(), w.next(), w.next()) {
                map.insert(name.to_string(), base.trim_start_matches('/').to_string());
            }
        }
    }
    map
}

/// `ExtGState` name -> (`/ca`, `/CA`): fill and stroke alpha.
fn ext_gstates(pdf: &str) -> HashMap<String, (Option<f64>, Option<f64>)> {
    let mut map = HashMap::new();
    let mut rest = pdf;
    while let Some(i) = rest.find("/ExtGState <<") {
        let dict = &rest[i + 13..];
        let end = dict.find(">>").unwrap();
        let entries: Vec<&str> = dict[..end].split_whitespace().collect();
        for e in entries.chunks(4) {
            if let [name, num, "0", "R"] = e {
                let tokens: Vec<&str> = object(pdf, num).split_whitespace().collect();
                let value = |key: &str| {
                    tokens
                        .iter()
                        .position(|t| *t == key)
                        .map(|k| tokens[k + 1].trim_end_matches(">>").parse().unwrap())
                };
                map.insert(name.to_string(), (value("/ca"), value("/CA")));
            }
        }
        rest = &dict[end..];
    }
    map
}

/// The pages' content streams and sizes, in page order.
fn page_streams(pdf: &str) -> Vec<(&str, (f64, f64))> {
    let mut out = Vec::new();
    let mut rest = pdf;
    while let Some(i) = rest.find("/Type /Page ") {
        let start = rest[..i].rfind(" obj").unwrap();
        let dict = &rest[start..];
        let dict = &dict[..dict.find("endobj").unwrap()];
        rest = &rest[i + 12..];
        let after = |key: &str| {
            let k = dict.find(key)? + key.len();
            Some(&dict[k..])
        };
        let contents = after("/Contents ").expect("a page without /Contents");
        let num = contents.split_whitespace().next().unwrap();
        // R's pages take the size from /Pages; ours give it per page.
        let media = after("/MediaBox [")
            .or_else(|| {
                let k = pdf.find("/MediaBox [")? + 11;
                Some(&pdf[k..])
            })
            .expect("no /MediaBox");
        let n: Vec<f64> = media[..media.find(']').unwrap()]
            .split_whitespace()
            .map(|v| v.parse().unwrap())
            .collect();
        let body = object(pdf, num);
        let s = body.find("stream").unwrap() + 6;
        let s = s + body[s..].find('\n').unwrap() + 1;
        let e = body.rfind("endstream").unwrap();
        out.push((&body[s..e], (n[2] - n[0], n[3] - n[1])));
    }
    out
}

#[derive(Clone)]
struct Gs {
    fill: [f64; 4],
    stroke: [f64; 4],
    lwd: f64,
    cap: i32,
    join: i32,
    mitre: f64,
    dash: Vec<f64>,
    clip: [f64; 4],
}

/// Reads an uncompressed PDF. Panics on anything it can't follow.
pub fn parse(pdf: &[u8]) -> Pdf {
    let pdf = latin1(pdf);
    assert!(
        !pdf.contains("/FlateDecode"),
        "compressed PDF: write it with compression off"
    );
    let fonts = base_fonts(&pdf);
    let alphas = ext_gstates(&pdf);
    let pages = page_streams(&pdf)
        .into_iter()
        .map(|(content, size)| page(content, size, &fonts, &alphas))
        .collect();
    Pdf { pages }
}

fn page(
    content: &str,
    size: (f64, f64),
    fonts: &HashMap<String, String>,
    alphas: &HashMap<String, (Option<f64>, Option<f64>)>,
) -> Page {
    let mut paths = Vec::new();
    let mut texts = Vec::new();
    let mut gs = Gs {
        fill: [0.0, 0.0, 0.0, 1.0],
        stroke: [0.0, 0.0, 0.0, 1.0],
        lwd: 1.0,
        cap: 0,
        join: 0,
        mitre: 10.0,
        dash: Vec::new(),
        clip: [0.0, 0.0, size.0, size.1],
    };
    let mut stack: Vec<Gs> = Vec::new();
    let mut nums: Vec<f64> = Vec::new();
    // An array being read, then the last one read (for `d` or `TJ`).
    let mut array: Option<Vec<Piece>> = None;
    let mut done_array: Option<Vec<Piece>> = None;
    let mut path: Vec<(f64, f64)> = Vec::new();
    let mut curved = false;
    let mut clipping = false;
    let (mut font, mut tf_size, mut m) = (String::new(), 0.0, [1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
    let mut name = String::new();
    let mut string: Option<String> = None;
    for t in tokens(content) {
        let t = match t {
            Tok::Str(s) => {
                match &mut array {
                    Some(a) => a.push(Piece::Str(s)),
                    None => string = Some(s),
                }
                continue;
            }
            Tok::Word(w) => w,
        };
        if let Ok(v) = t.parse::<f64>() {
            match &mut array {
                Some(a) => a.push(Piece::Kern(v)),
                None => nums.push(v),
            }
            continue;
        }
        if t.starts_with('/') {
            name = t;
            continue;
        }
        if t == "[" {
            assert!(array.is_none(), "nested array");
            array = Some(Vec::new());
            continue;
        }
        if t == "]" {
            done_array = Some(array.take().expect("unmatched ]"));
            continue;
        }
        // An array is the operand of the operator right after it.
        let arr = done_array.take();
        assert!(
            arr.is_none() || t == "d" || t == "TJ",
            "an array before `{t}`"
        );
        let last = |k: usize| nums[nums.len() - k..].to_vec();
        match t.as_str() {
            "q" => stack.push(gs.clone()),
            "Q" => gs = stack.pop().expect("Q without q"),
            "rg" | "scn" => gs.fill[..3].copy_from_slice(&last(3)),
            "RG" | "SCN" => gs.stroke[..3].copy_from_slice(&last(3)),
            "g" => gs.fill[..3].fill(nums[nums.len() - 1]),
            "G" => gs.stroke[..3].fill(nums[nums.len() - 1]),
            "cs" | "CS" => {}
            "gs" => {
                let (ca, big_ca) = *alphas
                    .get(&name)
                    .unwrap_or_else(|| panic!("unknown ExtGState {name}"));
                if let Some(a) = ca {
                    gs.fill[3] = a;
                }
                if let Some(a) = big_ca {
                    gs.stroke[3] = a;
                }
            }
            "w" => gs.lwd = nums[nums.len() - 1],
            "J" => gs.cap = nums[nums.len() - 1] as i32,
            "j" => gs.join = nums[nums.len() - 1] as i32,
            "M" => gs.mitre = nums[nums.len() - 1],
            "d" => {
                gs.dash = arr
                    .expect("d without an array")
                    .into_iter()
                    .map(|p| match p {
                        Piece::Kern(v) => v,
                        Piece::Str(_) => panic!("a string in a dash array"),
                    })
                    .collect();
            }
            "m" | "l" => path.push((nums[nums.len() - 2], nums[nums.len() - 1])),
            "c" => {
                let n = last(6);
                path.extend([(n[0], n[1]), (n[2], n[3]), (n[4], n[5])]);
                curved = true;
            }
            "re" => {
                let mut n = last(4);
                if n[2] < 0.0 {
                    n[0] += n[2];
                    n[2] = -n[2];
                }
                if n[3] < 0.0 {
                    n[1] += n[3];
                    n[3] = -n[3];
                }
                path.extend([
                    (n[0], n[1]),
                    (n[0] + n[2], n[1]),
                    (n[0] + n[2], n[1] + n[3]),
                    (n[0], n[1] + n[3]),
                ]);
            }
            "h" => {}
            "W" | "W*" => clipping = true,
            "n" => {
                if std::mem::take(&mut clipping) {
                    gs.clip = bbox(&path);
                }
                path.clear();
                curved = false;
            }
            "S" | "s" | "f" | "F" | "f*" | "B" | "B*" | "b" | "b*" => {
                assert!(!clipping, "a clip path that is also painted");
                let fill = t.starts_with(['f', 'F', 'B', 'b']);
                let stroke = t.starts_with(['S', 's', 'B', 'b']);
                paths.push(Path {
                    vertices: std::mem::take(&mut path),
                    curved: std::mem::take(&mut curved),
                    fill: fill.then_some(gs.fill),
                    stroke: stroke.then(|| Stroke {
                        col: gs.stroke,
                        lwd: gs.lwd,
                        cap: gs.cap,
                        join: gs.join,
                        mitre: gs.mitre,
                        dash: gs.dash.clone(),
                    }),
                    clip: gs.clip,
                    op: t,
                });
            }
            "BT" | "ET" => {}
            "Tf" => {
                font = fonts
                    .get(&name)
                    .unwrap_or_else(|| panic!("unknown font {name}"))
                    .clone();
                tf_size = nums[nums.len() - 1];
            }
            "Tm" => m = last(6).try_into().unwrap(),
            "Tj" | "TJ" => {
                let pieces = if t == "TJ" {
                    arr.expect("TJ without an array")
                } else {
                    vec![Piece::Str(string.take().expect("Tj without a string"))]
                };
                let string = pieces
                    .iter()
                    .filter_map(|p| match p {
                        Piece::Str(s) => Some(s.as_str()),
                        Piece::Kern(_) => None,
                    })
                    .collect();
                texts.push(Text {
                    string,
                    pieces,
                    font: font.clone(),
                    size: tf_size * (m[0] * m[0] + m[1] * m[1]).sqrt(),
                    matrix: m,
                    fill: gs.fill,
                    clip: gs.clip,
                });
            }
            other => panic!("unknown PDF operator `{other}`"),
        }
        nums.clear();
    }
    Page { size, paths, texts }
}

/// What [`assert_same`] compares, and how closely.
#[derive(Clone, Copy)]
pub struct Compare {
    /// Points, for vertices, bounding boxes, text origins and clips.
    pub pos: f64,
    /// Colour components and alpha (0 to 1).
    pub col: f64,
    /// Line widths and dash lengths.
    pub lwd: f64,
    /// Text sizes.
    pub size: f64,
    /// Text rotation, in degrees (R writes the matrix to 2 decimals).
    pub angle: f64,
    /// Compare curved paths by bounding box (else by operator only).
    pub curves_by_bbox: bool,
    /// Compare clip rectangles.
    pub clip: bool,
    /// Compare line joins and mitre limits.
    pub join: bool,
    /// Paths left out of the comparison (on both sides); by default page
    /// backgrounds.
    pub drop_path: fn(&Path, (f64, f64)) -> bool,
    /// Text whose origin isn't compared (e.g. labels with a minus sign:
    /// R's `pdf()` draws `-` as the wider `/minus`).
    pub skip_origin: fn(&Text) -> bool,
}

impl Default for Compare {
    fn default() -> Self {
        Compare {
            pos: 0.05,
            col: 0.005,
            lwd: 0.005,
            size: 0.01,
            angle: 0.1,
            curves_by_bbox: true,
            clip: true,
            join: true,
            drop_path: Path::is_page_background,
            skip_origin: |_| false,
        }
    }
}

pub fn close(a: &[f64], b: &[f64], tol: f64) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < tol)
}

fn flat(v: &[(f64, f64)]) -> Vec<f64> {
    v.iter().flat_map(|&(x, y)| [x, y]).collect()
}

/// Asserts two PDFs have the same number of pages and each pair is the
/// same ([`assert_same`]), leaving out `how.drop_path`'s paths.
pub fn assert_same_pdf(name: &str, ours: Pdf, r: Pdf, how: &Compare) {
    assert_eq!(ours.pages.len(), r.pages.len(), "{name}: page count");
    let several = ours.pages.len() > 1;
    for (k, (a, b)) in ours.pages.into_iter().zip(r.pages).enumerate() {
        let name = if several {
            format!("{name}, page {}", k + 1)
        } else {
            name.to_string()
        };
        let keep = |mut p: Page| {
            let size = p.size;
            p.paths.retain(|q| !(how.drop_path)(q, size));
            p
        };
        assert_same(&name, &keep(a), &keep(b), how);
    }
}

/// Asserts two pages paint the same paths and text, in order: for paths
/// the operator, vertices (curves by bounding box), fill, stroke colour,
/// width, cap, join, dash and clip; for text the string, font, size,
/// colour, origin, rotation and clip. Kerns aren't compared (see
/// `kerning_matches_r`).
pub fn assert_same(name: &str, ours: &Page, r: &Page, how: &Compare) {
    assert_eq!(
        ours.paths.len(),
        r.paths.len(),
        "{name}: painted path count\nours={:#?}\nr={:#?}",
        ours.paths,
        r.paths
    );
    for (k, (a, b)) in ours.paths.iter().zip(&r.paths).enumerate() {
        let ctx = format!("{name}, path {k}:\n ours={a:?}\n r=   {b:?}");
        assert_eq!(a.op, b.op, "paint operator differs. {ctx}");
        assert_eq!(a.curved, b.curved, "curvedness differs. {ctx}");
        if !a.curved {
            assert!(
                close(&flat(&a.vertices), &flat(&b.vertices), how.pos),
                "vertices differ. {ctx}"
            );
        } else if how.curves_by_bbox {
            assert!(close(&a.bbox(), &b.bbox(), how.pos), "bbox differs. {ctx}");
        }
        if let (Some(fa), Some(fb)) = (a.fill, b.fill) {
            assert!(close(&fa, &fb, how.col), "fill colour differs. {ctx}");
        }
        if let (Some(sa), Some(sb)) = (&a.stroke, &b.stroke) {
            assert!(
                close(&sa.col, &sb.col, how.col),
                "stroke colour differs. {ctx}"
            );
            assert!(
                (sa.lwd - sb.lwd).abs() < how.lwd,
                "line width differs. {ctx}"
            );
            assert_eq!(sa.cap, sb.cap, "line cap differs. {ctx}");
            if how.join {
                assert_eq!(sa.join, sb.join, "line join differs. {ctx}");
                assert!(
                    (sa.mitre - sb.mitre).abs() < 1e-6,
                    "mitre limit differs. {ctx}"
                );
            }
            assert!(
                close(&sa.dash, &sb.dash, how.lwd),
                "dash pattern differs. {ctx}"
            );
        }
        if how.clip {
            assert!(close(&a.clip, &b.clip, how.pos), "clip differs. {ctx}");
        }
    }

    let strings = |t: &[Text]| t.iter().map(|t| t.string.clone()).collect::<Vec<_>>();
    assert_eq!(
        strings(&ours.texts),
        strings(&r.texts),
        "{name}: text items"
    );
    for (a, b) in ours.texts.iter().zip(&r.texts) {
        let ctx = format!("{name}: text\n ours={a:?}\n r=   {b:?}");
        assert_eq!(a.font, b.font, "font differs. {ctx}");
        assert!((a.size - b.size).abs() < how.size, "size differs. {ctx}");
        assert!(
            close(&a.fill, &b.fill, how.col),
            "text colour differs. {ctx}"
        );
        let (oa, ob) = (a.origin(), b.origin());
        assert!(
            (how.skip_origin)(a) || close(&[oa.0, oa.1], &[ob.0, ob.1], how.pos),
            "origin differs. {ctx}"
        );
        assert!(
            (a.angle() - b.angle()).abs() < how.angle,
            "rotation differs. {ctx}"
        );
        if how.clip {
            assert!(close(&a.clip, &b.clip, how.pos), "clip differs. {ctx}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal one-page PDF around `content`, in our backend's layout.
    fn pdf(content: &str) -> Vec<u8> {
        format!(
            "%PDF-1.4\n1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n\
             2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n\
             3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100] \
             /Contents 4 0 R >>\nendobj\n\
             4 0 obj\n<< /Length 0 >>\nstream\n{content}endstream\nendobj\n\
             5 0 obj\n<< /Type /ExtGState /ca 0.5 >>\nendobj\n\
             6 0 obj\n<< /ExtGState << /Ga128 5 0 R >> >>\nendobj\n"
        )
        .into_bytes()
    }

    #[test]
    fn a_tj_array_is_text_not_a_dash() {
        let p = parse(&pdf(
            "[3 2] 0 d\nBT /F1 12 Tf 1 0 0 1 10 20 Tm [(T) 74 (o)] TJ ET\n\
             0 0 m 10 10 l S\n",
        ))
        .page();
        assert_eq!(p.texts[0].string, "To");
        assert_eq!(
            p.texts[0].pieces,
            [
                Piece::Str("T".into()),
                Piece::Kern(74.0),
                Piece::Str("o".into())
            ]
        );
        assert_eq!(p.paths[0].stroke.as_ref().unwrap().dash, [3.0, 2.0]);
        assert_eq!(p.texts[0].font, "Helvetica");
        assert_eq!(p.texts[0].size, 12.0);
    }

    #[test]
    fn clips_follow_q_and_q() {
        let p = parse(&pdf(
            "q 10 10 50 40 re W n\nq 20 20 5 5 re W n 0 0 m 1 1 l S Q\n\
             0 0 m 2 2 l S Q\n0 0 m 3 3 l S\n",
        ))
        .page();
        let clips: Vec<_> = p.paths.iter().map(|p| p.clip).collect();
        assert_eq!(
            clips,
            [
                [20.0, 20.0, 25.0, 25.0],
                [10.0, 10.0, 60.0, 50.0],
                [0.0, 0.0, 200.0, 100.0]
            ]
        );
    }

    #[test]
    fn negative_rectangles_are_normalised() {
        let p = parse(&pdf("10 50 20 -30 re f\n")).page();
        assert_eq!(p.paths[0].bbox(), [10.0, 20.0, 30.0, 50.0]);
        assert_eq!(p.paths[0].op, "f");
    }

    #[test]
    fn escapes_alpha_colours_and_styles() {
        let p = parse(&pdf(
            "1 0 0 rg 0 0 1 RG 3 w 2 J 1 j /Ga128 gs\n0 0 m 5 5 l b\n\
             BT /F2 1 Tf 10 0 0 10 1 2 Tm (caf\\351 \\(x\\)) Tj ET\n",
        ))
        .page();
        let path = &p.paths[0];
        assert_eq!(path.fill, Some([1.0, 0.0, 0.0, 0.5]));
        let s = path.stroke.as_ref().unwrap();
        assert_eq!(
            (s.col, s.lwd, s.cap, s.join),
            ([0.0, 0.0, 1.0, 1.0], 3.0, 2, 1)
        );
        assert_eq!(p.texts[0].string, "café (x)");
        assert_eq!(p.texts[0].font, "Helvetica-Bold");
        assert_eq!(p.texts[0].size, 10.0);
    }

    #[test]
    fn raw_latin1_bytes() {
        let mut b = pdf("BT /F1 12 Tf 1 0 0 1 0 0 Tm (caf#) Tj ET\n");
        let k = b.iter().position(|&c| c == b'#').unwrap();
        b[k] = 0xE9;
        assert_eq!(parse(&b).page().texts[0].string, "caf\u{e9}");
    }

    #[test]
    fn several_pages_in_order() {
        let doc = "%PDF-1.4\n1 0 obj\n<< /Type /Pages /Kids [2 0 R 4 0 R] \
                   /MediaBox [0 0 100 50] >>\nendobj\n\
                   2 0 obj\n<< /Type /Page /Parent 1 0 R /Contents 3 0 R >>\nendobj\n\
                   3 0 obj\n<< >>\nstream\n0 0 m 1 1 l S\nendstream\nendobj\n\
                   4 0 obj\n<< /Type /Page /Parent 1 0 R /Contents 5 0 R >>\nendobj\n\
                   5 0 obj\n<< >>\nstream\n0 0 10 10 re f\n1 1 5 5 re f\nendstream\nendobj\n";
        let pdf = parse(doc.as_bytes());
        let n: Vec<_> = pdf.pages.iter().map(|p| p.paths.len()).collect();
        assert_eq!(n, [1, 2]);
        assert_eq!(pdf.pages[1].size, (100.0, 50.0));
    }

    #[test]
    #[should_panic(expected = "unknown PDF operator")]
    fn unknown_operators_panic() {
        parse(&pdf("0 0 m 1 1 l 1 0 0 1 5 5 cm S\n"));
    }

    #[test]
    #[should_panic(expected = "an array before")]
    fn a_stray_array_panics() {
        parse(&pdf("[1 2] 0 0 m S\n"));
    }
}
