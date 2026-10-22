// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks shading lines (`density`/`angle`) in `polygon()`,
//! `rect()`, `barplot()` and `legend()` against a real R `pdf()` device's
//! uncompressed output: every painted path in order (vertices; fill,
//! stroke colour, width, cap, dash) and every text item's string, origin,
//! font, size and fill colour. Covers both of `polygon.fullhatch()`'s
//! branches and their boundaries, winding inside a self-intersecting
//! polygon, `NA`/0/negative densities, reversed axes and a log axis
//! (no shading, as R).
//!
//! An all-`NA` `density` is no shading. R checks this with
//! `is.numeric(density)`, so a logical `NA` (what `density = NA` gives)
//! shades all the same, putting a border in the fill colour; the R code
//! uses `NA_real_`, which R treats as intended.
//!
//! Not compared: R's `rect()` with a vector `lwd` and shading (R recycles
//! it over the lines of each shading line instead of per rectangle), and
//! `barplot()` legends with fewer `density` values than labels (R
//! reverses them before recycling). With `orderH`, R keeps `density` and
//! `angle` in segment order while the colours follow the rows; ours
//! follow the rows too (see `barplot_order_h`). Skipped if `Rscript` isn't available.

use crate::common;

use std::collections::HashMap;
use std::process::Command;

use rs_graphics::prelude::*;

const PAGE_W: f64 = 7.0;
const PAGE_H: f64 = 5.0;

const M: [[f64; 3]; 4] = [
    [1.0, 4.0, 2.0],
    [3.0, 1.0, 3.0],
    [2.0, 2.0, 5.0],
    [4.0, 3.0, 1.0],
];

fn mat() -> Vec<Vec<f64>> {
    M.iter().map(|c| c.to_vec()).collect()
}

fn r_matrix() -> String {
    let cols: Vec<String> = M.iter().map(|c| r_vec(c)).collect();
    format!("cbind({})", cols.join(", "))
}

const NAN: f64 = f64::NAN;

struct Case {
    name: &'static str,
    /// R code; `m` is the matrix above.
    r: &'static str,
    ours: fn(&mut Device<PdfBackend>),
}

macro_rules! window {
    () => {
        "plot.new(); plot.window(c(0, 10), c(0, 10))\n"
    };
}

fn window(d: &mut Device<PdfBackend>, xlim: (f64, f64), ylim: (f64, f64)) {
    d.plot_new();
    d.plot_window()
        .xlim(xlim.0, xlim.1)
        .ylim(ylim.0, ylim.1)
        .draw();
}

/// Five polygons (NA-separated): a triangle, a square, a thin slanted
/// quadrilateral, a pentagon and a triangle.
const PX: [f64; 23] = [
    0.5, 3.0, 1.5, NAN, 4.0, 6.0, 6.0, 4.0, NAN, 7.0, 9.5, 8.5, 6.5, NAN, 1.0, 3.0, 3.5, 2.0, 0.5,
    NAN, 5.0, 9.0, 6.0,
];
const PY: [f64; 23] = [
    6.0, 6.5, 9.5, NAN, 6.0, 6.0, 9.0, 9.0, NAN, 6.0, 7.0, 9.5, 8.0, NAN, 0.5, 0.5, 2.5, 4.5, 2.5,
    NAN, 1.0, 1.5, 4.5,
];

fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "angles",
            r: concat!(
                window!(),
                "px <- c(0.5, 3, 1.5, NA, 4, 6, 6, 4, NA, 7, 9.5, 8.5, 6.5, NA, 1, 3, 3.5, 2, 0.5, NA, 5, 9, 6)\n",
                "py <- c(6, 6.5, 9.5, NA, 6, 6, 9, 9, NA, 6, 7, 9.5, 8, NA, 0.5, 0.5, 2.5, 4.5, 2.5, NA, 1, 1.5, 4.5)\n",
                "polygon(px, py, density = 8, angle = c(0, 30, 45, 100, 160), col = c('red', NA, 'blue'), lty = c(1, 2), lwd = 1.5)\n",
                "polygon(px, py - 0.3, density = c(5, 12), angle = c(90, 135, -30, 44.9, 135.1), border = 'grey')"
            ),
            ours: |d| {
                window(d, (0.0, 10.0), (0.0, 10.0));
                d.polygon()
                    .x(PX)
                    .y(PY)
                    .density(8)
                    .angle([0, 30, 45, 100, 160])
                    .col([Some("red"), None, Some("blue")])
                    .lty([1, 2])
                    .lwd(1.5)
                    .draw();
                d.polygon()
                    .x(PX)
                    .y(PY.map(|v| v - 0.3))
                    .density([5, 12])
                    .angle([90.0, 135.0, -30.0, 44.9, 135.1])
                    .border("grey")
                    .draw();
            },
        },
        Case {
            // A pentagram: its centre has winding number 2, so it is shaded.
            name: "star",
            r: concat!(
                window!(),
                "a <- pi / 2 + (0:4) * 4 * pi / 5\n",
                "polygon(5 + 4 * cos(a), 5 + 4 * sin(a), density = 12, angle = 20, border = 'darkgreen')"
            ),
            ours: |d| {
                window(d, (0.0, 10.0), (0.0, 10.0));
                let a: Vec<f64> = (0..5)
                    .map(|k| {
                        std::f64::consts::PI / 2.0 + k as f64 * 4.0 * std::f64::consts::PI / 5.0
                    })
                    .collect();
                let x: Vec<f64> = a.iter().map(|a| 5.0 + 4.0 * a.cos()).collect();
                let y: Vec<f64> = a.iter().map(|a| 5.0 + 4.0 * a.sin()).collect();
                d.polygon()
                    .x(&x)
                    .y(&y)
                    .density(12)
                    .angle(20)
                    .border("darkgreen")
                    .draw();
            },
        },
        Case {
            // `NA` and negative densities fill, 0 draws only the border.
            name: "density_mix",
            r: concat!(
                window!(),
                "polygon(c(1, 3, 3, 1, NA, 4, 6, 6, 4, NA, 7, 9, 9, 7, NA, 1, 3, 3, 1),\n",
                "        c(1, 1, 3, 3, NA, 1, 1, 3, 3, NA, 1, 1, 3, 3, NA, 5, 5, 8, 8),\n",
                "        density = c(NA, 0, 15, -1), col = c('pink', 'green', 'blue', 'orange'), border = c('black', 'red'))"
            ),
            ours: |d| {
                window(d, (0.0, 10.0), (0.0, 10.0));
                d.polygon()
                    .x([
                        1.0, 3.0, 3.0, 1.0, NAN, 4.0, 6.0, 6.0, 4.0, NAN, 7.0, 9.0, 9.0, 7.0, NAN,
                        1.0, 3.0, 3.0, 1.0,
                    ])
                    .y([
                        1.0, 1.0, 3.0, 3.0, NAN, 1.0, 1.0, 3.0, 3.0, NAN, 1.0, 1.0, 3.0, 3.0, NAN,
                        5.0, 5.0, 8.0, 8.0,
                    ])
                    .density([None, Some(0.0), Some(15.0), Some(-1.0)])
                    .col(["pink", "green", "blue", "orange"])
                    .border(["black", "red"])
                    .draw();
            },
        },
        Case {
            // The border defaults to the shading colour; an all-`NA`
            // density is an ordinary rectangle.
            name: "rect",
            r: concat!(
                window!(),
                "rect(c(1, 6, 7), 1, c(3, 4, 9), c(4, 5, 3), density = c(5, 10, 20), angle = c(45, -30, 90),\n",
                "     col = c(NA, 'red'), lty = c(1, 2, 3), lwd = 2)\n",
                "rect(1, 6, 3, 8, density = NA_real_, col = 'grey')\n",
                "rect(4, 6, 6, 8, density = 0, col = 'purple')\n",
                "rect(7, 6, 9, 8, density = 7, border = 'black')"
            ),
            ours: |d| {
                window(d, (0.0, 10.0), (0.0, 10.0));
                d.rect()
                    .xleft([1, 6, 7])
                    .ybottom([1])
                    .xright([3, 4, 9])
                    .ytop([4, 5, 3])
                    .density([5, 10, 20])
                    .angle([45, -30, 90])
                    .col([None, Some("red")])
                    .lty([1, 2, 3])
                    .lwd(2)
                    .draw();
                d.rect()
                    .xleft([1])
                    .ybottom([6])
                    .xright([3])
                    .ytop([8])
                    .density([None::<f64>])
                    .col("grey")
                    .draw();
                d.rect()
                    .xleft([4])
                    .ybottom([6])
                    .xright([6])
                    .ytop([8])
                    .density(0)
                    .col("purple")
                    .draw();
                d.rect()
                    .xleft([7])
                    .ybottom([6])
                    .xright([9])
                    .ytop([8])
                    .density(7)
                    .border("black")
                    .draw();
            },
        },
        Case {
            name: "reversed_x",
            r: concat!(
                "plot.new(); plot.window(c(10, 0), c(0, 10))\n",
                "polygon(c(1, 6, 4), c(1, 2, 8), density = 10, angle = 30)\n",
                "rect(6, 5, 9, 9, density = 6, angle = 120, col = 'blue')"
            ),
            ours: |d| {
                window(d, (10.0, 0.0), (0.0, 10.0));
                d.polygon()
                    .x([1, 6, 4])
                    .y([1, 2, 8])
                    .density(10)
                    .angle(30)
                    .draw();
                d.rect()
                    .xleft([6])
                    .ybottom([5])
                    .xright([9])
                    .ytop([9])
                    .density(6)
                    .angle(120)
                    .col("blue")
                    .draw();
            },
        },
        Case {
            name: "reversed_y",
            r: concat!(
                "plot.new(); plot.window(c(0, 5), c(10, 0))\n",
                "polygon(c(0.5, 4, 3, 1), c(1, 2, 9, 7), density = 9, angle = 60, col = 'red')"
            ),
            ours: |d| {
                window(d, (0.0, 5.0), (10.0, 0.0));
                d.polygon()
                    .x([0.5, 4.0, 3.0, 1.0])
                    .y([1, 2, 9, 7])
                    .density(9)
                    .angle(60)
                    .col("red")
                    .draw();
            },
        },
        Case {
            // R doesn't shade on a log axis (it warns); fills and borders
            // are drawn.
            name: "log",
            r: concat!(
                "plot.new(); plot.window(c(1, 100), c(0, 10), log = 'x')\n",
                "suppressWarnings(polygon(c(2, 50, 50, 2, NA, 2, 50, 50, 2), c(1, 1, 4, 4, NA, 6, 6, 9, 9),\n",
                "        density = c(10, NA), col = c('red', 'blue')))"
            ),
            ours: |d| {
                d.plot_new();
                d.plot_window().xlim(1, 100).ylim(0, 10).xlog(true).draw();
                d.polygon()
                    .x([2.0, 50.0, 50.0, 2.0, NAN, 2.0, 50.0, 50.0, 2.0])
                    .y([1.0, 1.0, 4.0, 4.0, NAN, 6.0, 6.0, 9.0, 9.0])
                    .density([Some(10), None])
                    .col(["red", "blue"])
                    .draw();
            },
        },
        Case {
            name: "barplot_beside",
            r: "barplot(m, beside = TRUE, density = c(10, 20, NA), angle = c(45, 135, 0), \
                col = c('red', 'blue', 'green'), legend.text = c('r1', 'r2', 'r3'))",
            ours: |d| {
                d.barplot()
                    .height(mat())
                    .beside(true)
                    .density([Some(10), Some(20), None])
                    .angle([45, 135, 0])
                    .col(["red", "blue", "green"])
                    .legend_text(["r1", "r2", "r3"])
                    .draw();
            },
        },
        Case {
            name: "barplot_stacked",
            r: "barplot(m, density = c(8, 16, 24), angle = 60, legend.text = c('r1', 'r2', 'r3'))",
            ours: |d| {
                d.barplot()
                    .height(mat())
                    .density([8, 16, 24])
                    .angle(60)
                    .legend_text(["r1", "r2", "r3"])
                    .draw();
            },
        },
        Case {
            // With `orderH`, a segment's shading follows its row, as its
            // colour does. R keeps `density` in segment order instead; all
            // columns here sort the same way (rows 2, 3, 1), so R gets the
            // rows' densities in that order.
            name: "barplot_order_h",
            r: "barplot(cbind(c(1, 3, 2), c(2, 5, 4), c(1, 4, 3)), order = 'decr', \
                density = c(16, 24, 8), col = c('red', 'green', 'blue'), angle = c(30, 60, 120))",
            ours: |d| {
                d.barplot()
                    .height(vec![
                        vec![1.0, 3.0, 2.0],
                        vec![2.0, 5.0, 4.0],
                        vec![1.0, 4.0, 3.0],
                    ])
                    .order_h("decr")
                    .density([8, 16, 24])
                    .col(["red", "green", "blue"])
                    .angle([120, 30, 60])
                    .draw();
            },
        },
        Case {
            name: "barplot_horiz",
            r: "barplot(c(3, 1, 2, 4), horiz = TRUE, density = c(10, NA), angle = c(30, 0), col = c('red', 'blue'))",
            ours: |d| {
                d.barplot()
                    .height([3, 1, 2, 4])
                    .horiz(true)
                    .density([Some(10), None])
                    .angle([30, 0])
                    .col(["red", "blue"])
                    .draw();
            },
        },
        Case {
            // `density` alone draws boxes, shaded in `par(fg)`.
            name: "legend",
            r: concat!(
                window!(),
                "legend('topleft', legend = c('a', 'b', 'c'), density = c(10, NA, 20), angle = c(45, 0, 135),\n",
                "       fill = c('red', 'blue', 'green'))\n",
                "legend('bottomright', legend = c('d', 'e'), density = 15, cex = 2)"
            ),
            ours: |d| {
                window(d, (0.0, 10.0), (0.0, 10.0));
                d.legend()
                    .pos("topleft")
                    .labels(["a", "b", "c"])
                    .density([Some(10), None, Some(20)])
                    .angle([45, 0, 135])
                    .fill(["red", "blue", "green"])
                    .draw();
                d.legend()
                    .pos("bottomright")
                    .labels(["d", "e"])
                    .density(15)
                    .cex(2)
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
fn hatching_matches_r() {
    if !common::r_available() {
        return;
    }
    let dir = std::env::temp_dir();
    let cases = cases();
    let mut script = format!("m <- {}\n", r_matrix());
    for c in &cases {
        let path = dir.join(format!("rs_graphics_hatch_{}.pdf", c.name));
        script.push_str(&format!(
            "pdf(file = '{}', width = {PAGE_W}, height = {PAGE_H}, compress = FALSE)\n{}\ninvisible(dev.off())\n",
            path.display(),
            c.r
        ));
    }
    let script_path = dir.join("rs_graphics_hatch.R");
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
        let path = dir.join(format!("rs_graphics_hatch_{}.pdf", c.name));
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
