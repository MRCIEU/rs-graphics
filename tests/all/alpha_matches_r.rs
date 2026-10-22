// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks semi-transparent colours on PDF against a real R `pdf()`
//! device's uncompressed output: every painted path (vertices or, for
//! curves, bounding boxes; fill and stroke colour *with alpha*, which R and
//! we both set through `ExtGState` resources and `gs`; width, cap, dash)
//! and every text item (string, origin, font, size, fill with alpha),
//! across clipping (`q`/`Q`) and pages. Skipped if `Rscript` isn't
//! available.

use crate::common;

use std::collections::HashMap;
use std::process::Command;

use rs_graphics::prelude::*;

const PAGE_W: f64 = 6.0;
const PAGE_H: f64 = 5.0;

struct Case {
    name: &'static str,
    r: &'static str,
    ours: fn(&mut Device<PdfBackend>),
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            // Fill and border alphas, the same fill alpha twice, then opaque
            // again.
            name: "rects",
            r: "plot.new()\n\
                rect(0.1, 0.1, 0.5, 0.5, col = '#FF000080', border = '#0000FF40', lwd = 3)\n\
                rect(0.3, 0.3, 0.7, 0.7, col = '#00FF0080')\n\
                rect(0.6, 0.1, 0.9, 0.3, col = 'red')",
            ours: |d| {
                d.plot_new();
                d.rect()
                    .xleft([0.1])
                    .ybottom([0.1])
                    .xright([0.5])
                    .ytop([0.5])
                    .col("#FF000080")
                    .border("#0000FF40")
                    .lwd(3)
                    .draw();
                d.rect()
                    .xleft([0.3])
                    .ybottom([0.3])
                    .xright([0.7])
                    .ytop([0.7])
                    .col("#00FF0080")
                    .draw();
                d.rect()
                    .xleft([0.6])
                    .ybottom([0.1])
                    .xright([0.9])
                    .ytop([0.3])
                    .col("red")
                    .draw();
            },
        },
        Case {
            // Text, symbols with fill and border alphas, a line.
            name: "text_points_lines",
            r: "plot.new()\n\
                text(0.5, 0.8, 'mmm', col = '#00000033')\n\
                points(c(0.2, 0.8), c(0.2, 0.5), pch = 21, bg = '#FFFF0099', col = '#FF00FFCC', cex = 3)\n\
                lines(c(0, 1), c(0.9, 0.1), col = '#0000FF80', lwd = 4)\n\
                points(0.5, 0.5, pch = 19, col = '#FF000080', cex = 2)\n\
                text(0.5, 0.3, 'nnn')",
            ours: |d| {
                d.plot_new();
                d.text()
                    .x([0.5])
                    .y([0.8])
                    .labels("mmm")
                    .col("#00000033")
                    .draw();
                d.points()
                    .x([0.2, 0.8])
                    .y([0.2, 0.5])
                    .pch(21)
                    .bg("#FFFF0099")
                    .col("#FF00FFCC")
                    .cex(3)
                    .draw();
                d.lines()
                    .x([0.0, 1.0])
                    .y([0.9, 0.1])
                    .col("#0000FF80")
                    .lwd(4)
                    .draw();
                d.points()
                    .x([0.5])
                    .y([0.5])
                    .pch(19)
                    .col("#FF000080")
                    .cex(2)
                    .draw();
                d.text().x([0.5]).y([0.3]).labels("nnn").draw();
            },
        },
        Case {
            // Alpha set inside the plot's clip (`q`/`Q`), opaque axes after.
            name: "clipping",
            r: "plot(1:3, col = '#FF000066', pch = 19, cex = 3, xlab = '', ylab = '')\n\
                abline(h = 2, col = '#00000080', lwd = 5)\n\
                box(col = 'blue')",
            ours: |d| {
                d.plot()
                    .x([1, 2, 3])
                    .col("#FF000066")
                    .pch(19)
                    .cex(3)
                    .xlab("")
                    .ylab("")
                    .draw();
                d.abline().h([2]).col("#00000080").lwd(5).draw();
                d.box_().col("blue").draw();
            },
        },
        Case {
            // A new page starts opaque.
            name: "pages",
            r: "plot.new(); rect(0.2, 0.2, 0.8, 0.8, col = '#0000FF80')\n\
                plot.new(); rect(0.2, 0.2, 0.8, 0.8, col = 'blue')",
            ours: |d| {
                d.plot_new();
                d.rect()
                    .xleft([0.2])
                    .ybottom([0.2])
                    .xright([0.8])
                    .ytop([0.8])
                    .col("#0000FF80")
                    .draw();
                d.plot_new();
                d.rect()
                    .xleft([0.2])
                    .ybottom([0.2])
                    .xright([0.8])
                    .ytop([0.8])
                    .col("blue")
                    .draw();
            },
        },
    ]
}

/// Each `ExtGState` resource name and its fill (`/ca`) and stroke (`/CA`)
/// alphas.
fn ext_gstates(pdf: &str) -> HashMap<String, (Option<f64>, Option<f64>)> {
    let mut map = HashMap::new();
    let mut rest = pdf;
    while let Some(i) = rest.find("/ExtGState <<") {
        let dict = &rest[i + 13..];
        let end = dict.find(">>").unwrap();
        let entries: Vec<&str> = dict[..end].split_whitespace().collect();
        for e in entries.chunks(4) {
            if let [name, num, "0", "R"] = e {
                let header = format!("\n{num} 0 obj");
                let body = &pdf[pdf.find(&header).unwrap()..];
                let body = &body[..body.find("endobj").unwrap()];
                let tokens: Vec<&str> = body.split_whitespace().collect();
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

#[derive(Clone, Debug)]
struct Gs {
    fill: [f64; 4],
    stroke: [f64; 4],
    lwd: f64,
    cap: i32,
    dash: Vec<f64>,
}

#[derive(Debug)]
struct Painted {
    vertices: Vec<(f64, f64)>,
    curved: bool,
    /// Red, green, blue and alpha.
    fill: Option<[f64; 4]>,
    /// `(colour with alpha, line width, line cap, dash array)`.
    stroke: Option<([f64; 4], f64, i32, Vec<f64>)>,
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
    fill: [f64; 4],
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
                        // An octal escape (ours, for Latin-1 bytes).
                        let n = b[i..]
                            .iter()
                            .take(3)
                            .take_while(|c| c.is_ascii_digit())
                            .count();
                        if n > 0 {
                            let code = std::str::from_utf8(&b[i..i + n]).unwrap();
                            s.push(u8::from_str_radix(code, 8).unwrap() as char);
                            i += n;
                            continue;
                        }
                    }
                    let ch = content[i..].chars().next().unwrap();
                    s.push(ch);
                    i += ch.len_utf8();
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
    let alphas = ext_gstates(pdf);
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
            fill: [0.0, 0.0, 0.0, 1.0],
            stroke: [0.0, 0.0, 0.0, 1.0],
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
                "rg" | "scn" => gs.fill[..3].copy_from_slice(&last(3)),
                "RG" | "SCN" => gs.stroke[..3].copy_from_slice(&last(3)),
                "g" => gs.fill[..3].fill(nums[nums.len() - 1]),
                "G" => gs.stroke[..3].fill(nums[nums.len() - 1]),
                "gs" => {
                    let (ca, big_ca) = alphas.get(&last_name).copied().unwrap_or_default();
                    if let Some(a) = ca {
                        gs.fill[3] = a;
                    }
                    if let Some(a) = big_ca {
                        gs.stroke[3] = a;
                    }
                }
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

#[test]
fn semi_transparent_colours_match_r() {
    if !common::r_available() {
        return;
    }
    let dir = std::env::temp_dir();
    let cases = cases();
    let mut script = String::new();
    for c in &cases {
        let path = dir.join(format!("rs_graphics_alpha_{}.pdf", c.name));
        script.push_str(&format!(
            "pdf(file = '{}', width = {PAGE_W}, height = {PAGE_H}, compress = FALSE)\n{}\ninvisible(dev.off())\n",
            path.display(),
            c.r
        ));
    }
    let script_path = dir.join("rs_graphics_alpha.R");
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
        let path = dir.join(format!("rs_graphics_alpha_{}.pdf", c.name));
        let r_pdf: String = std::fs::read(&path)
            .unwrap()
            .iter()
            .map(|&c| c as char)
            .collect();
        let (r_paths, r_texts) = interpret(&r_pdf);
        let mut dev = Device::new(
            PdfBackend::new(PAGE_W, PAGE_H).with_compression(false),
            Par::default(),
        );
        (c.ours)(&mut dev);
        let ours: String = dev
            .into_backend()
            .finish()
            .iter()
            .map(|&c| c as char)
            .collect();
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
            assert!(
                close(&[a.origin.0, a.origin.1], &[b.origin.0, b.origin.1], 0.05),
                "origin differs. {ctx}"
            );
        }
    }
}
