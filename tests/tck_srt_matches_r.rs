// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks `par(tck)` (small, `1` for a grid, an inline `axis(tck =)`
//! overriding it, and `par(tck = NA)` resetting it) and `par(srt)` (on
//! `text()` with an inline `srt` overriding it and axis labels not
//! rotated, on `pie()` labels, also with `pie(srt = )` over it, and on
//! `legend()` labels and title, which R draws with `text()`) against a
//! real R `pdf()` device's uncompressed output. Compares every painted path in order and every text item's
//! string, origin, font, size, fill colour and rotation. Skipped if
//! `Rscript` isn't available.

mod common;

use std::collections::HashMap;
use std::process::Command;

use rs_graphics::prelude::*;

const PAGE_W: f64 = 7.0;
const PAGE_H: f64 = 6.0;

const X: [f64; 5] = [1.0, 2.0, 3.0, 4.0, 5.0];
const Y: [f64; 5] = [2.0, 4.0, 3.0, 5.0, 1.0];

type Draw = fn(&mut Device<PdfBackend>);

struct Case {
    name: &'static str,
    /// R code; `x` and `y` are the vectors above.
    r: &'static str,
    ours: Draw,
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "par_tck_small",
            r: "par(tck = 0.02); plot(x, y, ann = FALSE)",
            ours: |d| {
                d.par().tck(0.02).draw();
                d.plot(&X, &Y).draw();
            },
        },
        Case {
            name: "par_tck_grid",
            r: "par(tck = 1); plot(x, y, ann = FALSE)",
            ours: |d| {
                d.par().tck(1).draw();
                d.plot(&X, &Y).draw();
            },
        },
        Case {
            name: "inline_tck_overrides_par",
            r: "par(tck = -0.05); plot(x, y, ann = FALSE, axes = FALSE)\n\
                axis(1); axis(2, tck = 0.03); box()",
            ours: |d| {
                d.par().tck(-0.05).draw();
                d.plot(&X, &Y).axes(false).draw();
                d.axis(1).draw();
                d.axis(2).tck(0.03).draw();
                d.box_().draw();
            },
        },
        Case {
            name: "par_tck_na_resets",
            r: "par(tck = 0.1); par(tck = NA); plot(x, y, ann = FALSE)",
            ours: |d| {
                d.par().tck(0.1).draw();
                d.par().tck(f64::NAN).draw();
                d.plot(&X, &Y).draw();
            },
        },
        Case {
            name: "par_srt_text",
            r: "par(srt = 45); plot(x, y, ann = FALSE, type = 'n')\n\
                text(x, y, labels = c('a', 'b', 'c', 'd', 'e'))\n\
                text(3, 3, 'f', srt = -30)",
            ours: |d| {
                d.par().srt(45).draw();
                d.plot(&X, &Y).type_("n").draw();
                d.text(&X, &Y, &["a", "b", "c", "d", "e"]).draw();
                d.text(&[3], &[3], &["f"]).srt(-30).draw();
            },
        },
        Case {
            name: "par_srt_pie",
            r: "par(srt = 30); pie(c(1, 2, 3), labels = c('a', 'b', 'c'))",
            ours: |d| {
                d.par().srt(30).draw();
                d.pie(&[1, 2, 3]).labels(["a", "b", "c"]).draw();
            },
        },
        Case {
            name: "pie_inline_srt",
            r: "par(srt = 30); pie(c(1, 2, 3), labels = c('a', 'b', 'c'), srt = -45)",
            ours: |d| {
                d.par().srt(30).draw();
                d.pie(&[1, 2, 3]).labels(["a", "b", "c"]).srt(-45).draw();
            },
        },
        Case {
            name: "par_srt_legend",
            r: "plot(x, y, ann = FALSE); par(srt = 20)\n\
                legend('topleft', legend = c('abc', 'sum'), pch = 1:2, title = 'title')",
            ours: |d| {
                d.plot(&X, &Y).draw();
                d.par().srt(20).draw();
                d.legend("topleft", ["abc", "sum"])
                    .pch([1, 2])
                    .title("title")
                    .draw();
            },
        },
    ]
}

// -------------------------------------------------------------------
// A minimal PDF content-stream interpreter: painted paths, and text with
// font, size and fill colour.
// -------------------------------------------------------------------

#[derive(Clone, Debug)]
struct Gs {
    fill: [f64; 3],
    stroke: [f64; 3],
    lwd: f64,
    cap: i32,
    dash: Vec<f64>,
}

#[derive(Debug)]
struct Painted {
    vertices: Vec<(f64, f64)>,
    curved: bool,
    fill: Option<[f64; 3]>,
    /// `(colour, line width, line cap, dash array)`.
    stroke: Option<([f64; 3], f64, i32, Vec<f64>)>,
}

impl Painted {
    fn bbox(&self) -> [f64; 4] {
        let xs = self.vertices.iter().map(|p| p.0);
        let ys = self.vertices.iter().map(|p| p.1);
        [
            xs.clone().fold(f64::INFINITY, f64::min),
            ys.clone().fold(f64::INFINITY, f64::min),
            xs.fold(f64::NEG_INFINITY, f64::max),
            ys.fold(f64::NEG_INFINITY, f64::max),
        ]
    }
}

#[derive(Debug)]
struct Text {
    string: String,
    origin: (f64, f64),
    font: String,
    size: f64,
    fill: [f64; 3],
    /// Rotation in degrees, from the text matrix.
    angle: f64,
}

enum Tok {
    Str(String),
    Word(String),
}

/// Tokens of a content stream; `(...)` strings (with `\` escapes) are
/// one token each.
fn tokens(content: &str) -> Vec<Tok> {
    let b = content.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'(' => {
                let mut s = String::new();
                i += 1;
                while i < b.len() && b[i] != b')' {
                    if b[i] == b'\\' {
                        i += 1;
                    }
                    s.push(b[i] as char);
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
                out.push(Tok::Word(content[start..i].to_string()));
            }
        }
    }
    out
}

/// Font resource name -> base font, from the PDF's font objects (R's
/// `/Name /F2 /BaseFont /Helvetica-Bold`) or, for ours, the fixed ones.
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

fn interpret(pdf: &str) -> (Vec<Painted>, Vec<Text>) {
    let fonts = base_fonts(pdf);
    let mut painted = Vec::new();
    let mut texts = Vec::new();
    let mut rest = pdf;
    while let Some(i) = rest.find("stream\n") {
        let body = &rest[i + 7..];
        let Some(j) = body.find("endstream") else {
            break;
        };
        let content = &body[..j];
        rest = &body[j + 9..];
        if !content.contains(" m") && !content.contains(" re") && !content.contains(" Tf") {
            continue;
        }
        let mut gs = Gs {
            fill: [0.0; 3],
            stroke: [0.0; 3],
            lwd: 1.0,
            cap: 0,
            dash: Vec::new(),
        };
        let mut stack: Vec<Gs> = Vec::new();
        let mut nums: Vec<f64> = Vec::new();
        let mut array: Option<Vec<f64>> = None;
        let mut path: Vec<(f64, f64)> = Vec::new();
        let mut curved = false;
        let (mut font, mut tf_size, mut m) = (String::new(), 0.0, [0.0; 6]);
        let mut last_name = String::new();
        let mut last_str = String::new();
        for t in tokens(content) {
            let t = match t {
                Tok::Str(s) => {
                    last_str = s;
                    continue;
                }
                Tok::Word(w) => w,
            };
            if let Ok(v) = t.parse::<f64>() {
                match &mut array {
                    Some(a) => a.push(v),
                    None => nums.push(v),
                }
                continue;
            }
            let last = |k: usize| nums[nums.len() - k..].to_vec();
            match t.as_str() {
                "[" => array = Some(Vec::new()),
                "]" => {
                    gs.dash = array.take().unwrap_or_default();
                    continue;
                }
                "q" => stack.push(gs.clone()),
                "Q" => gs = stack.pop().unwrap(),
                "rg" | "scn" => gs.fill = last(3).try_into().unwrap(),
                "RG" | "SCN" => gs.stroke = last(3).try_into().unwrap(),
                "g" => gs.fill = [nums[nums.len() - 1]; 3],
                "G" => gs.stroke = [nums[nums.len() - 1]; 3],
                "w" => gs.lwd = nums[nums.len() - 1],
                "J" => gs.cap = nums[nums.len() - 1] as i32,
                "m" | "l" => path.push((nums[nums.len() - 2], nums[nums.len() - 1])),
                "c" => {
                    let n = last(6);
                    path.extend([(n[0], n[1]), (n[2], n[3]), (n[4], n[5])]);
                    curved = true;
                }
                "re" => {
                    // Normalised to a positive width and height.
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
                "Tf" => {
                    font = fonts.get(&last_name).cloned().unwrap_or_default();
                    tf_size = nums[nums.len() - 1];
                }
                "Tm" => m = last(6).try_into().unwrap(),
                "Tj" => texts.push(Text {
                    string: std::mem::take(&mut last_str),
                    origin: (m[4], m[5]),
                    font: font.clone(),
                    size: tf_size * (m[0] * m[0] + m[1] * m[1]).sqrt(),
                    fill: gs.fill,
                    angle: m[1].atan2(m[0]).to_degrees(),
                }),
                "W" | "W*" | "h" => {}
                "n" => {
                    path.clear();
                    curved = false;
                }
                "S" | "s" | "f" | "F" | "f*" | "B" | "B*" | "b" | "b*" => {
                    let fill = t.starts_with(['f', 'F', 'B', 'b']);
                    let stroke = t.starts_with(['S', 's', 'B', 'b']);
                    let p = Painted {
                        vertices: std::mem::take(&mut path),
                        curved: std::mem::take(&mut curved),
                        fill: fill.then_some(gs.fill),
                        stroke: stroke.then(|| (gs.stroke, gs.lwd, gs.cap, gs.dash.clone())),
                    };
                    // Skip only a full-page background fill (neither side
                    // draws one by default, but keep the figure box).
                    let b = p.bbox();
                    if p.stroke.is_some() || b[2] - b[0] < 0.99 * PAGE_W * 72.0 {
                        painted.push(p);
                    }
                }
                w if w.starts_with('/') => last_name = w.to_string(),
                _ => {}
            }
            nums.clear();
        }
    }
    (painted, texts)
}

fn close(a: &[f64], b: &[f64], tol: f64) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < tol)
}

fn r_vec(v: &[f64]) -> String {
    let v: Vec<String> = v.iter().map(|x| x.to_string()).collect();
    format!("c({})", v.join(", "))
}

#[test]
fn tck_and_srt_match_r() {
    if !common::r_available() {
        return;
    }
    let dir = std::env::temp_dir();
    let cases = cases();
    let mut script = format!("x <- {}\ny <- {}\n", r_vec(&X), r_vec(&Y));
    for c in &cases {
        let path = dir.join(format!("rs_graphics_tck_srt_{}.pdf", c.name));
        script.push_str(&format!(
            "pdf(file = '{}', width = {PAGE_W}, height = {PAGE_H}, compress = FALSE)\n\
             {}\ninvisible(dev.off())\n",
            path.display(),
            c.r
        ));
    }
    let script_path = dir.join("rs_graphics_tck_srt.R");
    std::fs::write(&script_path, script).unwrap();
    let out = Command::new("Rscript")
        .arg(&script_path)
        .output()
        .expect("run Rscript");
    assert!(
        out.status.success(),
        "Rscript failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    for c in &cases {
        let path = dir.join(format!("rs_graphics_tck_srt_{}.pdf", c.name));
        let r_pdf = String::from_utf8_lossy(&std::fs::read(&path).unwrap()).into_owned();
        let (r_paths, r_texts) = interpret(&r_pdf);
        let mut dev = Device::new(
            PdfBackend::new(PAGE_W, PAGE_H).with_compression(false),
            Par::default(),
        );
        (c.ours)(&mut dev);
        let ours = String::from_utf8_lossy(&dev.into_backend().finish()).into_owned();
        let (our_paths, our_texts) = interpret(&ours);
        let name = c.name;

        assert_eq!(
            our_paths.len(),
            r_paths.len(),
            "{name}: painted path count\nours={our_paths:#?}\nr={r_paths:#?}"
        );
        for (k, (a, b)) in our_paths.iter().zip(&r_paths).enumerate() {
            let ctx = format!("{name}, path {k}:\n ours={a:?}\n r=   {b:?}");
            assert_eq!(a.curved, b.curved, "curvedness differs. {ctx}");
            if a.curved {
                assert!(close(&a.bbox(), &b.bbox(), 0.05), "bbox differs. {ctx}");
            } else {
                let flat = |p: &Painted| -> Vec<f64> {
                    p.vertices.iter().flat_map(|&(x, y)| [x, y]).collect()
                };
                assert!(close(&flat(a), &flat(b), 0.05), "vertices differ. {ctx}");
            }
            assert_eq!(a.fill.is_some(), b.fill.is_some(), "fill differs. {ctx}");
            assert_eq!(
                a.stroke.is_some(),
                b.stroke.is_some(),
                "stroke differs. {ctx}"
            );
            if let (Some(fa), Some(fb)) = (a.fill, b.fill) {
                assert!(close(&fa, &fb, 0.005), "fill colour differs. {ctx}");
            }
            if let (Some(sa), Some(sb)) = (&a.stroke, &b.stroke) {
                assert!(close(&sa.0, &sb.0, 0.005), "stroke colour differs. {ctx}");
                assert!((sa.1 - sb.1).abs() < 0.005, "line width differs. {ctx}");
                assert_eq!(sa.2, sb.2, "line cap differs. {ctx}");
                assert!(close(&sa.3, &sb.3, 0.005), "dash pattern differs. {ctx}");
            }
        }

        let strings = |t: &[Text]| t.iter().map(|t| t.string.clone()).collect::<Vec<_>>();
        assert_eq!(strings(&our_texts), strings(&r_texts), "{name}: text items");
        for (a, b) in our_texts.iter().zip(&r_texts) {
            let ctx = format!("{name}: text\n ours={a:?}\n r=   {b:?}");
            assert_eq!(a.font, b.font, "font differs. {ctx}");
            assert!((a.size - b.size).abs() < 0.01, "size differs. {ctx}");
            assert!(close(&a.fill, &b.fill, 0.005), "text colour differs. {ctx}");
            // R writes the text matrix to a few decimals.
            assert!((a.angle - b.angle).abs() < 0.1, "rotation differs. {ctx}");
            assert!(
                close(&[a.origin.0, a.origin.1], &[b.origin.0, b.origin.1], 0.05),
                "origin differs. {ctx}"
            );
        }
    }
}
