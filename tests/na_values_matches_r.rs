// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks `None` elements (R's `NA`) in the recycled style vectors
//! against a real R `pdf()` device's uncompressed output: every painted
//! path in order (vertices or, for curves, bounding boxes; fill, stroke
//! colour, width, cap, dash) and every text item's string, origin, font,
//! size and fill colour.
//!
//! Where R defines `NA` (a skipped point, segment, line or symbol, or the
//! default text size and font), the R code passes `NA`. Where R errors
//! (`lty = NA` outside `legend()`, legend `cex`) or leaves it undefined
//! (`lwd = NA` for points), `None` still skips the element (or, for text
//! sizes, fonts and legend spacing, means the default), and the R code
//! draws that instead: the same call without the element, or with the
//! default. Skipped if `Rscript` isn't available.

mod common;

use std::collections::HashMap;
use std::process::Command;

use rs_graphics::prelude::*;

const PAGE_W: f64 = 7.0;
const PAGE_H: f64 = 5.0;

const X: [f64; 5] = [1.0, 2.0, 3.0, 4.0, 5.0];
const Y: [f64; 5] = [2.0, 4.0, 3.0, 5.0, 1.0];

struct Case {
    name: &'static str,
    /// R code; `x` and `y` are the vectors above.
    r: &'static str,
    ours: fn(&mut Device<PdfBackend>),
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "points",
            r: "plot(x, y, ann = FALSE, pch = c(1, NA, 3, 19, 2), cex = c(1, 1, NA, 2, 1), \
                col = c(1, 2, 3, NA, 4))",
            ours: |d| {
                d.plot(&X, &Y)
                    .pch([Some(1), None, Some(3), Some(19), Some(2)])
                    .cex([Some(1), Some(1), None, Some(2), Some(1)])
                    .col([Some(1), Some(2), Some(3), None, Some(4)])
                    .draw();
            },
        },
        Case {
            // R draws a point with `lwd = NA` at an undefined width.
            name: "points_lwd",
            r: "plot(x, y, ann = FALSE, type = 'n'); points(x[-2], y[-2], lwd = c(2, 3, 1, 2))",
            ours: |d| {
                d.plot(&X, &Y).type_("n").draw();
                d.points(&X, &Y)
                    .lwd([Some(2), None, Some(3), Some(1), Some(2)])
                    .draw();
            },
        },
        Case {
            // R uses `par(lwd)` for `lines(lwd = NA)`.
            name: "lines_lwd",
            r: "plot(x, y, ann = FALSE, type = 'n'); par(lwd = 2); lines(x, y, lwd = NA)",
            ours: |d| {
                d.plot(&X, &Y).type_("n").draw();
                d.par().lwd(2).draw();
                d.lines(&X, &Y).lwd([None::<f64>]).draw();
            },
        },
        Case {
            name: "segments_arrows",
            r: "plot(x, y, ann = FALSE, type = 'n')\n\
                segments(x, y, x + 0.5, y + 0.5, lwd = c(2, NA, 1, 1, 3), col = c(1, 2, NA, 4, 5))\n\
                arrows(x, y - 0.5, x + 0.3, y - 0.2, lwd = c(NA, 2))",
            ours: |d| {
                d.plot(&X, &Y).type_("n").draw();
                let (x1, y1) = (X.map(|v| v + 0.5), Y.map(|v| v + 0.5));
                d.segments(&X, &Y, &x1, &y1)
                    .lwd([Some(2), None, Some(1), Some(1), Some(3)])
                    .col([Some(1), Some(2), None, Some(4), Some(5)])
                    .draw();
                let (y0, x2, y2) = (Y.map(|v| v - 0.5), X.map(|v| v + 0.3), Y.map(|v| v - 0.2));
                d.arrows(&X, &y0, &x2, &y2).lwd([None, Some(2)]).draw();
            },
        },
        Case {
            // R errors for `lty = NA`.
            name: "segments_lty",
            r: "plot(x, y, ann = FALSE, type = 'n'); segments(x[-4], y[-4], x[-4] + 0.5, y[-4], lty = c(1, 2, 3, 5))",
            ours: |d| {
                d.plot(&X, &Y).type_("n").draw();
                d.segments(&X, &Y, &X.map(|v| v + 0.5), &Y)
                    .lty([Some(1), Some(2), Some(3), None, Some(5)])
                    .draw();
            },
        },
        Case {
            // `lwd = NA` leaves out a rectangle's border in R; `lty = NA`
            // errors (`None` leaves out the border too).
            name: "rect",
            r: "plot(x, y, ann = FALSE, type = 'n')\n\
                rect(x, y, x + 0.4, y + 0.3, col = 'grey', lwd = c(1, NA, 2, 1, 1))\n\
                rect(c(1, 3), 0.5, c(1.4, 3.4), 0.8, lty = c(1, 3))\n\
                rect(2, 0.5, 2.4, 0.8, border = NA, col = 'pink')",
            ours: |d| {
                d.plot(&X, &Y).type_("n").draw();
                d.rect(&X, &Y, &X.map(|v| v + 0.4), &Y.map(|v| v + 0.3))
                    .col("grey")
                    .lwd([Some(1), None, Some(2), Some(1), Some(1)])
                    .draw();
                d.rect(&[1.0, 2.0, 3.0], &[0.5], &[1.4, 2.4, 3.4], &[0.8])
                    .lty([Some(1), None, Some(3)])
                    .draw();
                d.rect(&[2.0], &[0.5], &[2.4], &[0.8])
                    .col("pink")
                    .lty([None::<LineType>])
                    .draw();
            },
        },
        Case {
            // R errors for `lty = NA`; `None` leaves out the border.
            name: "polygon_lty",
            r: "plot(x, y, ann = FALSE, type = 'n')\n\
                polygon(c(1, 2, 2, NA, 3, 4, 4, NA, 4, 5, 5), c(1, 1, 2, NA, 1, 1, 2, NA, 1, 1, 2), \
                        col = c('red', 'green', 'blue'), border = c(1, NA, 1), lty = c(1, 1, 2))",
            ours: |d| {
                d.plot(&X, &Y).type_("n").draw();
                let nan = f64::NAN;
                d.polygon(
                    &[1.0, 2.0, 2.0, nan, 3.0, 4.0, 4.0, nan, 4.0, 5.0, 5.0],
                    &[1.0, 1.0, 2.0, nan, 1.0, 1.0, 2.0, nan, 1.0, 1.0, 2.0],
                )
                .col([Some("red"), Some("green"), Some("blue")])
                .lty([Some(1), None, Some(2)])
                .draw();
            },
        },
        Case {
            name: "text_mtext",
            r: "plot(x, y, ann = FALSE, type = 'n')\n\
                text(x, y, c('a', 'b', 'c', 'd', 'e'), cex = c(1, NA, 2), font = c(2, 3, NA))\n\
                mtext(c('m', 'n'), at = c(2, 4), cex = c(NA, 1.5), font = c(2, NA))",
            ours: |d| {
                d.plot(&X, &Y).type_("n").draw();
                d.text(&X, &Y, &["a", "b", "c", "d", "e"])
                    .cex([Some(1), None, Some(2)])
                    .font([Some(2), Some(3), None])
                    .draw();
                d.mtext(["m", "n"], 3)
                    .at([2, 4])
                    .cex([None, Some(1.5)])
                    .font([Some(2), None])
                    .draw();
            },
        },
        Case {
            // `lwd = NA` skips a line in R; `lty = NA` errors.
            name: "abline",
            r: "plot(x, y, ann = FALSE, type = 'n')\n\
                abline(h = 1:4, lwd = c(1, NA, 2, 1))\n\
                abline(v = c(1, 3), lty = c(2, 3))",
            ours: |d| {
                d.plot(&X, &Y).type_("n").draw();
                d.abline()
                    .h([1, 2, 3, 4])
                    .lwd([Some(1), None, Some(2), Some(1)])
                    .draw();
                d.abline().v([1, 2, 3]).lty([Some(2), None, Some(3)]).draw();
            },
        },
        Case {
            // R skips `NA` symbols and `medlwd`; `medlty = NA` errors, so R
            // draws a blank median there.
            name: "boxplot",
            r: "boxplot(list(c(1:10, 30), c(2:12, 40), 3:9), medpch = c(19, NA, 1), \
                medlwd = c(3, NA, 2), outpch = c(NA, 19), outcex = c(1, 2))\n\
                boxplot(list(1:10, 2:12), medlty = c(0, 1), add = TRUE, at = c(1.5, 2.5), boxwex = 0.2)",
            ours: |d| {
                let g = [
                    (1..=10).map(f64::from).chain([30.0]).collect::<Vec<_>>(),
                    (2..=12).map(f64::from).chain([40.0]).collect(),
                    (3..=9).map(f64::from).collect(),
                ];
                d.boxplot(&g[..])
                    .medpch([Some(19), None, Some(1)])
                    .medlwd([Some(3), None, Some(2)])
                    .outpch([None, Some(19)])
                    .outcex([Some(1), Some(2)])
                    .draw();
                let g2 = [
                    (1..=10).map(f64::from).collect::<Vec<_>>(),
                    (2..=12).map(f64::from).collect(),
                ];
                d.boxplot(&g2[..])
                    .medlty([None, Some(1)])
                    .add(true)
                    .at([1.5, 2.5])
                    .boxwex([0.2])
                    .draw();
            },
        },
        Case {
            // R errors for `lty = NA`; `None` leaves out the bar's border, as
            // an `NA` border does.
            name: "hist_lty",
            r: "hist(c(1, 2, 2, 3, 3, 3, 4, 4), col = 'grey', border = c(1, NA), ann = FALSE)",
            ours: |d| {
                d.hist(&[1.0, 2.0, 2.0, 3.0, 3.0, 3.0, 4.0, 4.0])
                    .col("grey")
                    .lty([Some(LineType::SOLID), None])
                    .ann(false)
                    .draw();
            },
        },
        Case {
            name: "legend_keys",
            r: "plot(x, y, ann = FALSE, type = 'n')\n\
                legend('topleft', c('a', 'mm', 'n'), pch = c(19, NA, 1), lty = c(NA, 1, 2), \
                       lwd = c(1, 2, NA), pt.cex = c(2, 1, NA), text.font = c(NA, 2, 3))",
            ours: |d| {
                d.plot(&X, &Y).type_("n").draw();
                d.legend("topleft", ["a", "mm", "n"])
                    .pch([Some(19), None, Some(1)])
                    .lty([None, Some(1), Some(2)])
                    .lwd([Some(1), Some(2), None])
                    .pt_cex([Some(2), Some(1), None])
                    .text_font([None, Some(2), Some(3)])
                    .draw();
            },
        },
        Case {
            // R errors for an `NA` `cex` and breaks for `NA` spacing; `None`
            // is the default there, and `pt.lwd = NA` (undefined in R)
            // skips the point.
            name: "legend_defaults",
            r: "plot(x, y, ann = FALSE, type = 'n')\n\
                legend('bottomright', c('a', 'mm', 'n'), pch = c(1, 2, NA), lty = 1, \
                       cex = c(1.5, 1, 1), x.intersp = c(2, 1, 1), seg.len = c(2, 3, 2), \
                       y.intersp = c(1, 1.5, 1), pt.lwd = c(2, 1, 1))",
            ours: |d| {
                d.plot(&X, &Y).type_("n").draw();
                d.legend("bottomright", ["a", "mm", "n"])
                    .pch([1, 2, 3])
                    .lty(1)
                    .cex([Some(1.5), None, Some(1.0)])
                    .x_intersp([Some(2), None, Some(1)])
                    .seg_len([None, Some(3), Some(2)])
                    .y_intersp([None, Some(1.5), None])
                    .pt_lwd([Some(2), Some(1), None])
                    .draw();
            },
        },
    ]
}

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
fn na_values_match_r() {
    if !common::r_available() {
        return;
    }
    let dir = std::env::temp_dir();
    let cases = cases();
    let mut script = format!("x <- {}\ny <- {}\n", r_vec(&X), r_vec(&Y));
    for c in &cases {
        let path = dir.join(format!("rs_graphics_na_values_{}.pdf", c.name));
        script.push_str(&format!(
            "pdf(file = '{}', width = {PAGE_W}, height = {PAGE_H}, compress = FALSE)\n{}\ninvisible(dev.off())\n",
            path.display(),
            c.r
        ));
    }
    let script_path = dir.join("rs_graphics_na_values.R");
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
        let path = dir.join(format!("rs_graphics_na_values_{}.pdf", c.name));
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
