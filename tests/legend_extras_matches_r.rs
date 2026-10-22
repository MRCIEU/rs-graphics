// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks `legend()` on reversed axes, the box style (`box.lwd`,
//! `box.lty`, `box.col`), the title style (`title.col`, `title.adj`,
//! `title.font`), per-entry `text.col`/`text.font`, `plot = FALSE`, and
//! R's per-key vectors (boxes, lines and points mixed with `NA`s,
//! character `pch`, per-key sizes and spacing, `text.width = NA` or per
//! column, `adj`) against a real R `pdf()` device's uncompressed output:
//! every painted
//! path in order (vertices or, for curves, bounding boxes; fill, stroke
//! colour, width, cap, dash), every text item's string, origin, font,
//! size and fill colour, and `legend()`'s return value. Skipped if
//! `Rscript` isn't available.

mod common;

use std::collections::HashMap;
use std::process::Command;

use rs_graphics::prelude::*;

const PAGE_W: f64 = 7.0;
const PAGE_H: f64 = 5.0;

const X: [f64; 5] = [1.0, 2.0, 3.0, 4.0, 5.0];
const Y: [f64; 5] = [2.0, 4.0, 3.0, 5.0, 1.0];

type Draw = fn(&mut Device<PdfBackend>) -> LegendLayout;

struct Case {
    name: &'static str,
    /// R code ending in `l <- legend(...)`; `x` and `y` are the vectors
    /// above.
    r: &'static str,
    ours: Draw,
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "reversed_x_keyword",
            r: "plot(x, y, ann = FALSE, xlim = c(6, 0))\n\
                l <- legend('topright', c('a', 'b'), pch = 1:2, lty = 1:2, title = 'T', inset = 0.05)",
            ours: |d| {
                d.plot().x(X).y(Y).xlim(6, 0).draw();
                d.legend()
                    .pos("topright")
                    .labels(["a", "b"])
                    .pch([1, 2])
                    .lty([1, 2])
                    .title("T")
                    .inset((0.05, 0.05))
                    .draw()
            },
        },
        Case {
            name: "reversed_y_xy_fill",
            r: "plot(x, y, ann = FALSE, ylim = c(6, 0))\n\
                l <- legend(1, 1, c('a', 'b'), fill = c('red', 'blue'), xjust = 0.2, yjust = 0.3)",
            ours: |d| {
                d.plot().x(X).y(Y).ylim(6, 0).draw();
                d.legend()
                    .pos((1, 1))
                    .labels(["a", "b"])
                    .fill(["red", "blue"])
                    .xjust(0.2)
                    .yjust(0.3)
                    .draw()
            },
        },
        Case {
            name: "reversed_both_ncol",
            r: "plot(x, y, ann = FALSE, xlim = c(6, 0), ylim = c(6, 0))\n\
                l <- legend('bottomright', c('a', 'b', 'c'), pch = 1:3, ncol = 2, inset = c(0.02, 0.1))",
            ours: |d| {
                d.plot().x(X).y(Y).xlim(6, 0).ylim(6, 0).draw();
                d.legend()
                    .pos("bottomright")
                    .labels(["a", "b", "c"])
                    .pch([1, 2, 3])
                    .ncol(2usize)
                    .inset((0.02, 0.1))
                    .draw()
            },
        },
        Case {
            name: "reversed_x_long_title",
            r: "plot(x, y, ann = FALSE, xlim = c(6, 0))\n\
                l <- legend('bottomleft', c('a', 'b'), pch = 1:2, title = 'mmm nnn mmm nnn')",
            ours: |d| {
                d.plot().x(X).y(Y).xlim(6, 0).draw();
                let keys = LegendOptions::default().pch([1, 2]);
                d.legend()
                    .pos("bottomleft")
                    .labels(["a", "b"])
                    .with(&keys)
                    .title("mmm nnn mmm nnn")
                    .draw()
            },
        },
        Case {
            name: "reversed_x_text_width",
            r: "plot(x, y, ann = FALSE, xlim = c(6, 0))\n\
                l <- legend('center', c('a', 'b'), lty = 1:2, text.width = 1.5)",
            ours: |d| {
                d.plot().x(X).y(Y).xlim(6, 0).draw();
                d.legend()
                    .pos("center")
                    .labels(["a", "b"])
                    .lty([1, 2])
                    .text_width(1.5)
                    .draw()
            },
        },
        Case {
            name: "box_style",
            r: "plot(x, y, ann = FALSE)\n\
                l <- legend('topleft', c('a', 'b'), pch = 1:2, box.lwd = 3, box.lty = 2, \
                            box.col = 'blue', bg = 'grey90')",
            ours: |d| {
                d.plot().x(X).y(Y).draw();
                let keys = LegendOptions::default().pch([1, 2]);
                d.legend()
                    .pos("topleft")
                    .labels(["a", "b"])
                    .with(&keys)
                    .box_lwd(3)
                    .box_lty("dashed")
                    .box_col("blue")
                    .bg("grey90")
                    .draw()
            },
        },
        Case {
            name: "box_from_par_and_no_border",
            r: "plot(x, y, ann = FALSE); par(lwd = 2, lty = 3, fg = 'red', col = 'black')\n\
                legend('top', c('a', 'b'), pch = 1:2)\n\
                l <- legend('bottom', c('a', 'b'), pch = 1:2, box.col = NA, bg = 'yellow')",
            ours: |d| {
                d.plot().x(X).y(Y).draw();
                d.par().lwd(2).lty("dotted").fg("red").draw();
                let keys = LegendOptions::default().pch([1, 2]);
                d.legend().pos("top").labels(["a", "b"]).with(&keys).draw();
                d.legend()
                    .pos("bottom")
                    .labels(["a", "b"])
                    .with(&keys)
                    .box_col(ColorSpec::None)
                    .bg("yellow")
                    .draw()
            },
        },
        Case {
            name: "title_style",
            r: "plot(x, y, ann = FALSE)\n\
                l <- legend('topright', c('a', 'b'), pch = 1:2, title = 'mmm', \
                            title.col = 'red', title.adj = 0, title.font = 2)",
            ours: |d| {
                d.plot().x(X).y(Y).draw();
                let keys = LegendOptions::default().pch([1, 2]);
                d.legend()
                    .pos("topright")
                    .labels(["a", "b"])
                    .with(&keys)
                    .title("mmm")
                    .title_col("red")
                    .title_adj(0)
                    .title_font(2)
                    .draw()
            },
        },
        Case {
            name: "text_per_entry",
            r: "plot(x, y, ann = FALSE)\n\
                l <- legend('bottomright', c('mmmm', 'mmmm', 'a'), pch = 1:3, title = 'nnn', \
                            title.adj = 1, text.col = c('blue', 'darkgreen'), text.font = c(3, 2))",
            ours: |d| {
                d.plot().x(X).y(Y).draw();
                // The bold second label sets the width; the title takes
                // the first colour and font.
                d.legend()
                    .pos("bottomright")
                    .labels(["mmmm", "mmmm", "a"])
                    .pch([1, 2, 3])
                    .title("nnn")
                    .title_adj(1)
                    .text_col(["blue", "darkgreen"])
                    .text_font([3, 2])
                    .draw()
            },
        },
        Case {
            name: "mixed_keys",
            r: "plot(x, y, ann = FALSE)\n\
                l <- legend('topleft', c('a', 'nn', 'ccc', 'dd'), fill = c('red', NA, NA, 'blue'), \
                            border = c('black', NA, NA, 'darkgreen'), lty = c(0, 1, 0, 2), \
                            pch = c(NA, NA, 19, 21), pt.bg = 'yellow', col = c(1, 'red', 'blue', 1))",
            ours: |d| {
                d.plot().x(X).y(Y).draw();
                d.legend()
                    .pos("topleft")
                    .labels(["a", "nn", "ccc", "dd"])
                    .fill([Some("red"), None, None, Some("blue")])
                    .border([Some("black"), None, None, Some("darkgreen")])
                    .lty([0, 1, 0, 2])
                    .pch([None, None, Some(19), Some(21)])
                    .pt_bg("yellow")
                    .col(["black", "red", "blue", "black"])
                    .draw()
            },
        },
        Case {
            name: "lwd_draws_lines_no_merge",
            r: "plot(x, y, ann = FALSE)\n\
                l <- legend('topright', c('a', 'b'), pch = 1:2, lwd = c(1, 3), merge = FALSE, \
                            cex = c(0.8, 1.3))",
            ours: |d| {
                d.plot().x(X).y(Y).draw();
                d.legend()
                    .pos("topright")
                    .labels(["a", "b"])
                    .pch([1, 2])
                    .lwd([1, 3])
                    .merge(false)
                    .cex([0.8, 1.3])
                    .draw()
            },
        },
        Case {
            name: "character_pch",
            r: "plot(x, y, ann = FALSE)\n\
                l <- legend('bottomleft', c('a', 'b', 'c', 'd'), pch = c('x', '+', '\\u00e9', ''), \
                            col = c(1, 2, 4, 1))",
            ours: |d| {
                d.plot().x(X).y(Y).draw();
                d.legend()
                    .pos("bottomleft")
                    .labels(["a", "b", "c", "d"])
                    .pch(["x", "+", "\u{e9}", ""])
                    .col([1, 2, 4, 1])
                    .draw()
            },
        },
        Case {
            name: "character_pch_split",
            r: "plot(x, y, ann = FALSE)\n\
                l <- legend('bottomright', c('mm', 'nn', 'aa'), pch = 'xyz', lty = 1, col = 2:4)",
            ours: |d| {
                d.plot().x(X).y(Y).draw();
                d.legend()
                    .pos("bottomright")
                    .labels(["mm", "nn", "aa"])
                    .pch("xyz")
                    .lty(1)
                    .col([2, 3, 4])
                    .draw()
            },
        },
        Case {
            name: "per_entry_sizes",
            r: "plot(x, y, ann = FALSE)\n\
                l <- legend('top', c('a', 'nn', 'ccc', 'd', 'mm'), ncol = 2, pch = 1, lty = 1, \
                            cex = c(0.8, 1.2, 1, 1.5, 0.7), x.intersp = c(1, 2, 0.5, 1, 0.5), \
                            seg.len = c(1, 3), y.intersp = c(1, 1.8, 1.2), pt.cex = c(1, 2))",
            ours: |d| {
                d.plot().x(X).y(Y).draw();
                d.legend()
                    .pos("top")
                    .labels(["a", "nn", "ccc", "d", "mm"])
                    .ncol(2)
                    .pch(1)
                    .lty(1)
                    .cex([0.8, 1.2, 1.0, 1.5, 0.7])
                    .x_intersp([1.0, 2.0, 0.5, 1.0, 0.5])
                    .seg_len([1, 3])
                    .y_intersp([1.0, 1.8, 1.2])
                    .pt_cex([1, 2])
                    .draw()
            },
        },
        Case {
            name: "text_width_na",
            r: "plot(x, y, ann = FALSE)\n\
                l <- legend('center', c('a', 'mmmmmm', 'cc', 'd'), pch = 1:4, ncol = 3, text.width = NA)",
            ours: |d| {
                d.plot().x(X).y(Y).draw();
                d.legend()
                    .pos("center")
                    .labels(["a", "mmmmmm", "cc", "d"])
                    .pch([1, 2, 3, 4])
                    .ncol(3)
                    .text_width(f64::NAN)
                    .draw()
            },
        },
        Case {
            name: "text_width_per_column",
            r: "plot(x, y, ann = FALSE)\n\
                l <- legend('bottom', c('a', 'b', 'c', 'd', 'e'), pch = 1:5, ncol = 3, \
                            text.width = c(0.5, 1.5))",
            ours: |d| {
                d.plot().x(X).y(Y).draw();
                d.legend()
                    .pos("bottom")
                    .labels(["a", "b", "c", "d", "e"])
                    .pch([1, 2, 3, 4, 5])
                    .ncol(3)
                    .text_width([0.5, 1.5])
                    .draw()
            },
        },
        Case {
            name: "adj",
            r: "plot(x, y, ann = FALSE)\n\
                legend('topright', c('mmm', 'a'), pch = 1:2, adj = c(0.5, 0.2))\n\
                l <- legend('bottomleft', c('mmm', 'g'), pch = 1:2, adj = 0.3)",
            ours: |d| {
                d.plot().x(X).y(Y).draw();
                d.legend()
                    .pos("topright")
                    .labels(["mmm", "a"])
                    .pch([1, 2])
                    .adj(0.5)
                    .vadj(0.2)
                    .draw();
                d.legend()
                    .pos("bottomleft")
                    .labels(["mmm", "g"])
                    .pch([1, 2])
                    .adj(0.3)
                    .vadj(f64::NAN)
                    .draw()
            },
        },
        Case {
            name: "plot_false",
            r: "plot(x, y, ann = FALSE)\n\
                l <- legend('topright', c('a', 'b'), pch = 1:2, title = 'T', plot = FALSE)\n\
                rect(l$rect$left, l$rect$top - l$rect$h, l$rect$left + l$rect$w, l$rect$top, \
                     border = 'red')",
            ours: |d| {
                d.plot().x(X).y(Y).draw();
                let keys = LegendOptions::default().pch([1, 2]);
                let l = d
                    .legend()
                    .pos("topright")
                    .labels(["a", "b"])
                    .with(&keys)
                    .title("T")
                    .plot(false)
                    .draw();
                d.rect()
                    .xleft([l.left])
                    .ybottom([l.top - l.h])
                    .xright([l.left + l.w])
                    .ytop([l.top])
                    .border("red")
                    .draw();
                l
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

/// R's return value, printed as `left top w h text.x... text.y...`.
fn r_layout(line: &str) -> Vec<f64> {
    line.split_whitespace()
        .map(|v| v.parse().unwrap())
        .collect()
}

#[test]
fn legend_extras_match_r() {
    if !common::r_available() {
        return;
    }
    let dir = std::env::temp_dir();
    let cases = cases();
    let mut script = format!("x <- {}\ny <- {}\n", r_vec(&X), r_vec(&Y));
    for c in &cases {
        let path = dir.join(format!("rs_graphics_legend_extras_{}.pdf", c.name));
        script.push_str(&format!(
            "pdf(file = '{}', width = {PAGE_W}, height = {PAGE_H}, compress = FALSE)\n\
             {}\n\
             cat(unlist(l$rect[c('left', 'top', 'w', 'h')]), l$text$x, l$text$y, '\\n')\n\
             invisible(dev.off())\n",
            path.display(),
            c.r
        ));
    }
    let script_path = dir.join("rs_graphics_legend_extras.R");
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
    let stdout = String::from_utf8(out.stdout).unwrap();
    let r_values: Vec<&str> = stdout.lines().collect();
    assert_eq!(r_values.len(), cases.len());

    for (c, r_value) in cases.iter().zip(r_values) {
        let path = dir.join(format!("rs_graphics_legend_extras_{}.pdf", c.name));
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
        let layout = (c.ours)(&mut dev);
        let usr = dev.usr();
        let ours: String = dev
            .into_backend()
            .finish()
            .iter()
            .map(|&c| c as char)
            .collect();
        let (our_paths, our_texts) = interpret(&ours);
        let name = c.name;

        // The return value, to 1/5000 of each axis range.
        let r = r_layout(r_value);
        let (tx, ty) = (
            (usr.1 - usr.0).abs() / 5000.0,
            (usr.3 - usr.2).abs() / 5000.0,
        );
        let mut mine = vec![
            (layout.left, tx),
            (layout.top, ty),
            (layout.w, tx),
            (layout.h, ty),
        ];
        mine.extend(layout.text_x.iter().map(|&v| (v, tx)));
        mine.extend(layout.text_y.iter().map(|&v| (v, ty)));
        assert_eq!(
            mine.len(),
            r.len(),
            "{name}: returned values\nours={layout:?}\nr={r:?}"
        );
        for (k, (&(a, tol), &b)) in mine.iter().zip(&r).enumerate() {
            assert!(
                (a - b).abs() < tol,
                "{name}: returned value {k}: ours={a} r={b}\nours={layout:?}\nr={r:?}"
            );
        }

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
