// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks `bty` (every value through `plot(bty = )`, `par(bty)`,
//! `box(bty = , which = )` and `boxplot()`'s frame) and the inline title
//! parameters (`cex.main`/`col.main`/`font.main` and the `sub` and `lab`
//! ones) of `plot()`, `title()`, `hist()`, `barplot()`, `boxplot()`
//! (which, as R's `bxp()`, passes only the `cex` and `col` ones) and
//! `pie()` against a real R `pdf()` device's uncompressed output. Compares
//! every painted path in order (vertices or, for curves, bounding boxes;
//! fill, stroke colour, width, cap, dash) and every text item's string,
//! origin, font, size and fill colour. Skipped if `Rscript` isn't
//! available.

mod common;

use std::collections::HashMap;
use std::process::Command;

use rs_graphics::prelude::*;

const PAGE_W: f64 = 8.0;
const PAGE_H: f64 = 6.0;

const X: [f64; 5] = [1.0, 2.0, 3.0, 4.0, 5.0];
const Y: [f64; 5] = [2.0, 4.0, 3.0, 5.0, 1.0];
const H: [f64; 12] = [1.0, 2.0, 2.0, 3.0, 3.0, 3.0, 4.0, 4.0, 5.0, 6.0, 6.5, 7.0];

type Draw = fn(&mut Device<PdfBackend>);

struct Case {
    name: &'static str,
    /// R code; `x`, `y` and `h` are the vectors above.
    r: &'static str,
    ours: Draw,
}

fn groups() -> Vec<Vec<f64>> {
    vec![vec![1.0, 2.0, 3.0, 4.0, 9.0], vec![2.0, 3.0, 5.0, 6.0]]
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "bty_every_value",
            r: "par(mfrow = c(2, 4)); for (b in c('o', 'l', '7', 'c', 'u', ']', 'n', '[')) \
                plot(x, y, ann = FALSE, bty = b)",
            ours: |d| {
                d.par().mfrow(2, 4).draw();
                for b in ["o", "l", "7", "c", "u", "]", "n", "["] {
                    d.plot(&X, &Y).bty(b).draw();
                }
            },
        },
        Case {
            name: "par_bty",
            r: "par(bty = 'L'); plot(x, y, ann = FALSE)",
            ours: |d| {
                d.par().bty("L").draw();
                d.plot(&X, &Y).draw();
            },
        },
        Case {
            name: "box_bty_which",
            r: "plot(x, y, ann = FALSE, axes = FALSE)\n\
                box(bty = 'u', lwd = 2, col = 'red')\n\
                box(which = 'figure', bty = 'l', col = 'blue')",
            ours: |d| {
                d.plot(&X, &Y).axes(false).draw();
                d.box_().bty("u").lwd(2).col("red").draw();
                d.box_().which(BoxWhich::Figure).bty("l").col("blue").draw();
            },
        },
        Case {
            name: "boxplot_par_bty",
            r: "par(bty = '7'); boxplot(list(c(1, 2, 3, 4, 9), c(2, 3, 5, 6)))",
            ours: |d| {
                d.par().bty("7").draw();
                d.boxplot(&groups()).draw();
            },
        },
        Case {
            name: "plot_title_pars",
            r: "plot(x, y, main = 'title', sub = 'sub', xlab = 'xlab', ylab = 'ylab', \
                cex.main = 2, col.main = 'red', font.main = 3, \
                cex.sub = 0.8, col.sub = 'blue', font.sub = 2, \
                cex.lab = 1.5, col.lab = 'darkgreen', font.lab = 4)",
            ours: |d| {
                d.plot(&X, &Y)
                    .main("title")
                    .sub("sub")
                    .xlab("xlab")
                    .ylab("ylab")
                    .cex_main(2)
                    .col_main("red")
                    .font_main(3)
                    .cex_sub(0.8)
                    .col_sub("blue")
                    .font_sub(2)
                    .cex_lab(1.5)
                    .col_lab("darkgreen")
                    .font_lab(4)
                    .draw();
            },
        },
        Case {
            name: "title_pars_and_par_defaults",
            r: "par(cex.main = 1.5, col.lab = 'orange')\n\
                plot(x, y, ann = FALSE)\n\
                title(main = 'title', xlab = 'xlab', ylab = 'ylab', col.main = 'purple', \
                font.main = 1, font.lab = 3)",
            ours: |d| {
                d.par().cex_main(1.5).col_lab("orange").draw();
                d.plot(&X, &Y).draw();
                d.title()
                    .main("title")
                    .xlab("xlab")
                    .ylab("ylab")
                    .col_main("purple")
                    .font_main(1)
                    .font_lab(3)
                    .draw();
            },
        },
        Case {
            name: "hist_and_barplot_title_pars",
            r: "par(mfrow = c(1, 2))\n\
                hist(h, main = 'title', xlab = 'xlab', ylab = 'n', cex.main = 0.9, \
                col.main = 'red', font.lab = 3, cex.lab = 1.2)\n\
                barplot(c(3, 5, 2), main = 'title', sub = 'sub', xlab = 'xlab', ylab = 'ylab', \
                font.main = 1, col.sub = 'blue', cex.sub = 1.3, col.lab = 'grey40')",
            ours: |d| {
                d.par().mfrow(1, 2).draw();
                d.hist(&H)
                    .main("title")
                    .xlab("xlab")
                    .ylab("n")
                    .cex_main(0.9)
                    .col_main("red")
                    .font_lab(3)
                    .cex_lab(1.2)
                    .draw();
                d.barplot([3, 5, 2])
                    .main("title")
                    .sub("sub")
                    .xlab("xlab")
                    .ylab("ylab")
                    .font_main(1)
                    .col_sub("blue")
                    .cex_sub(1.3)
                    .col_lab("grey40")
                    .draw();
            },
        },
        Case {
            // R's bxp() passes only cex.* and col.* to title(): font.main
            // has no effect, so it isn't an option here.
            name: "boxplot_title_pars",
            r: "boxplot(list(c(1, 2, 3, 4, 9), c(2, 3, 5, 6)), main = 'title', sub = 'sub', \
                xlab = 'xlab', ylab = 'ylab', cex.main = 2, col.main = 'red', font.main = 3, \
                cex.sub = 0.7, col.sub = 'blue', col.lab = 'darkgreen', cex.lab = 1.4)",
            ours: |d| {
                d.boxplot(&groups())
                    .main("title")
                    .sub("sub")
                    .xlab("xlab")
                    .ylab("ylab")
                    .cex_main(2)
                    .col_main("red")
                    .cex_sub(0.7)
                    .col_sub("blue")
                    .col_lab("darkgreen")
                    .cex_lab(1.4)
                    .draw();
            },
        },
        Case {
            name: "pie_title_pars",
            r: "pie(c(1, 2, 3), labels = c('a', 'b', 'c'), main = 'title', cex.main = 0.8, \
                col.main = 'red', font.main = 4)",
            ours: |d| {
                d.pie(&[1, 2, 3])
                    .labels(["a", "b", "c"])
                    .main("title")
                    .cex_main(0.8)
                    .col_main("red")
                    .font_main(4)
                    .draw();
            },
        },
        Case {
            name: "title_adj",
            r: "plot.new(); plot.window(c(0, 10), c(0, 10))\n\
                par(adj = 0); title(main = 'M', sub = 'S', xlab = 'X', ylab = 'Y')\n\
                title(main = 'N', xlab = 'W', ylab = 'Z', adj = 1)\n\
                par(adj = 0.5); title(sub = 'T', adj = 0.25)\n\
                title(main = expression(alpha^2), adj = 0.75)",
            ours: |d| {
                d.plot_new();
                d.plot_window((0, 10), (0, 10)).draw();
                d.par().adj(0).draw();
                d.title().main("M").sub("S").xlab("X").ylab("Y").draw();
                d.title().main("N").xlab("W").ylab("Z").adj(1).draw();
                d.par().adj(0.5).draw();
                d.title().sub("T").adj(0.25).draw();
                d.title().main(Label::math("alpha^2")).adj(0.75).draw();
            },
        },
        Case {
            name: "title_line_outer",
            r: "par(oma = c(2, 3, 3, 1)); plot.new(); plot.window(c(0, 10), c(0, 10))\n\
                title(main = 'M', sub = 'S', xlab = 'X', ylab = 'Y', line = 1)\n\
                title(main = 'N', outer = TRUE)\n\
                title(main = 'O', sub = 'T', xlab = 'W', ylab = 'Z', outer = TRUE, line = 0.5, adj = 0)\n\
                title(main = expression(beta^2), line = 2.5)",
            ours: |d| {
                d.par().oma([2, 3, 3, 1]).draw();
                d.plot_new();
                d.plot_window((0, 10), (0, 10)).draw();
                d.title()
                    .main("M")
                    .sub("S")
                    .xlab("X")
                    .ylab("Y")
                    .line(1)
                    .draw();
                d.title().main("N").outer(true).draw();
                d.title()
                    .main("O")
                    .sub("T")
                    .xlab("W")
                    .ylab("Z")
                    .outer(true)
                    .line(0.5)
                    .adj(0)
                    .draw();
                d.title().main(Label::math("beta^2")).line(2.5).draw();
            },
        },
        Case {
            name: "plot_window_pars_and_panels",
            r: "par(mfrow = c(2, 2))\n\
                plot(x, y, asp = 1, ann = FALSE)\n\
                plot(x, y, xaxs = 'i', yaxs = 'i', lab = c(10, 3, 7), ann = FALSE)\n\
                plot(x, y, ann = FALSE, panel.first = abline(h = 1:5, col = 'grey'),\n\
                     panel.last = points(3, 3, pch = 19, col = 'red'))\n\
                plot.new(); plot.window(c(0, 10), c(0, 10), xaxs = 'i', lab = c(3, 8, 7))\n\
                axis(1); axis(2)\n\
                plot.window(c(0, 10), c(0, 10)); axis(3)",
            ours: |d| {
                d.par().mfrow(2, 2).draw();
                d.plot(&X, &Y).asp(1).draw();
                d.plot(&X, &Y).xaxs("i").yaxs("i").lab([10, 3, 7]).draw();
                let mut grid = Figure::new();
                grid.abline().h([1, 2, 3, 4, 5]).col("grey").draw();
                let mut mark = Figure::new();
                mark.points(&[3], &[3]).pch(19).col("red").draw();
                d.plot(&X, &Y).panel_first(grid).panel_last(mark).draw();
                d.plot_new();
                d.plot_window((0, 10), (0, 10))
                    .xaxs("i")
                    .lab([3, 8, 7])
                    .draw();
                d.axis(1).draw();
                d.axis(2).draw();
                // The inline xaxs and lab didn't last.
                d.plot_window((0, 10), (0, 10)).draw();
                d.axis(3).draw();
            },
        },
        Case {
            name: "mtext_recycling",
            r: "plot.new(); plot.window(c(0, 10), c(0, 5))\n\
                mtext(c('a', 'b', 'c'), side = 1, at = c(2, 5, 8), line = c(0, 1),\n\
                      col = c('red', 'blue'), cex = c(1, 1.5, 2), font = c(1, 2))\n\
                mtext('x', side = 3, at = c(1, 4, 9), adj = c(0, 1))\n\
                mtext(c('p', 'q'), side = 2, line = c(1, 2.5))\n\
                mtext(expression(alpha, beta), side = 4, at = c(1, 4))",
            ours: |d| {
                d.plot_new();
                d.plot_window((0, 10), (0, 5)).draw();
                d.mtext(["a", "b", "c"], 1)
                    .at([2, 5, 8])
                    .line([0, 1])
                    .col(["red", "blue"])
                    .cex([1.0, 1.5, 2.0])
                    .font([1, 2])
                    .draw();
                d.mtext("x", 3).at([1, 4, 9]).adj([0, 1]).draw();
                d.mtext(["p", "q"], 2).line([1.0, 2.5]).draw();
                d.mtext([Label::math("alpha"), Label::math("beta")], 4)
                    .at([1, 4])
                    .draw();
            },
        },
        Case {
            name: "pie_inline_cex_font",
            r: "pie(c(1, 2, 3), labels = c('a', 'b', 'c'), main = 'M', cex = 1.8, font = 3)",
            ours: |d| {
                d.pie(&[1, 2, 3])
                    .labels(["a", "b", "c"])
                    .main("M")
                    .cex(1.8)
                    .font(3)
                    .draw();
            },
        },
        Case {
            name: "hist_labels_col_border",
            r: "par(mfrow = c(1, 3))\n\
                hist(h, labels = TRUE, col = c('red', 'blue'), border = c('black', 'green'),\n\
                     lty = c(1, 2), ann = FALSE)\n\
                hist(h, breaks = c(0, 3, 7), labels = TRUE, ann = FALSE)\n\
                hist(h, breaks = c(0, 4, 7), labels = c('p', 'q'), ann = FALSE)",
            ours: |d| {
                d.par().mfrow(1, 3).draw();
                d.hist(&H)
                    .labels(true)
                    .col(["red", "blue"])
                    .border(["black", "green"])
                    .lty(["solid", "dashed"])
                    .ann(false)
                    .draw();
                d.hist(&H).breaks([0, 3, 7]).labels(true).ann(false).draw();
                d.hist(&H)
                    .breaks([0, 4, 7])
                    .labels(["p", "q"])
                    .ann(false)
                    .draw();
            },
        },
        Case {
            name: "frame_names_xpd_plot_false",
            r: "par(mfrow = c(2, 3))\n\
                plot(x, y, frame.plot = FALSE, ann = FALSE)\n\
                plot(x, y, axes = FALSE, frame.plot = TRUE, ann = FALSE)\n\
                boxplot(list(a = c(1, 2, 3, 9)), show.names = TRUE)\n\
                boxplot(list(a = c(1, 2, 3, 9), b = c(2, 3, 4)), show.names = FALSE, frame.plot = FALSE)\n\
                barplot(c(3, 5, 2), ylim = c(0, 4), xpd = FALSE)\n\
                invisible(hist(h, plot = FALSE)); invisible(barplot(c(1, 2), plot = FALSE))",
            ours: |d| {
                d.par().mfrow(2, 3).draw();
                d.plot(&X, &Y).frame_plot(false).draw();
                d.plot(&X, &Y).axes(false).frame_plot(true).draw();
                d.boxplot(&[vec![1, 2, 3, 9]])
                    .names(["a"])
                    .show_names(true)
                    .draw();
                d.boxplot(&[vec![1, 2, 3, 9], vec![2, 3, 4]])
                    .names(["a", "b"])
                    .show_names(false)
                    .frame_plot(false)
                    .draw();
                d.barplot([3, 5, 2]).ylim(0, 4).xpd(false).draw();
                d.hist(&H).plot(false).draw();
                d.barplot([1, 2]).plot(false).draw();
            },
        },
        Case {
            name: "plot_inline_axis_pars",
            r: "par(mfrow = c(1, 2))\n\
                plot(x, y, las = 1, cex.axis = 0.8, col.axis = 'blue', font.axis = 2,\n\
                     mgp = c(2, 0.6, 0), tck = 0.02, xlab = 'X', ylab = 'Y')\n\
                plot(x, y, tcl = -0.2, las = 2, ann = FALSE)",
            ours: |d| {
                d.par().mfrow(1, 2).draw();
                d.plot(&X, &Y)
                    .las(1)
                    .cex_axis(0.8)
                    .col_axis("blue")
                    .font_axis(2)
                    .mgp([2.0, 0.6, 0.0])
                    .tck(0.02)
                    .xlab("X")
                    .ylab("Y")
                    .draw();
                d.plot(&X, &Y).tcl(-0.2).las(2).draw();
            },
        },
        Case {
            name: "hist_barplot_inline_axis_pars",
            r: "par(mfrow = c(1, 2))\n\
                hist(h, las = 1, cex.axis = 1.2, mgp = c(2.5, 0.8, 0), tcl = 0.3, gap.axis = 3,\n\
                     main = 'M', xlab = 'X', ylab = 'Y')\n\
                barplot(c(3, 5, 2), names.arg = c('a', 'b', 'c'), las = 2, cex.axis = 0.7,\n\
                        col.axis = 'red', mgp = c(2, 0.5, 0), ylab = 'Y')",
            ours: |d| {
                d.par().mfrow(1, 2).draw();
                d.hist(&H)
                    .las(1)
                    .cex_axis(1.2)
                    .mgp([2.5, 0.8, 0.0])
                    .tcl(0.3)
                    .gap_axis(3)
                    .main("M")
                    .xlab("X")
                    .ylab("Y")
                    .draw();
                d.barplot([3, 5, 2])
                    .names(["a", "b", "c"])
                    .las(2)
                    .cex_axis(0.7)
                    .col_axis("red")
                    .mgp([2.0, 0.5, 0.0])
                    .ylab("Y")
                    .draw();
            },
        },
        Case {
            name: "barplot_names_axis",
            r: "par(mfrow = c(1, 2))\n\
                barplot(c(3, 5, 2), names.arg = c('a', 'b', 'c'), cex.names = 1.5, axis.lty = 1,\n\
                        cex.axis = 0.7)\n\
                barplot(c(3, 5, 2), names.arg = c('a', 'b', 'c'), axisnames = FALSE, horiz = TRUE)",
            ours: |d| {
                d.par().mfrow(1, 2).draw();
                d.barplot([3, 5, 2])
                    .names(["a", "b", "c"])
                    .cex_names(1.5)
                    .axis_lty("solid")
                    .cex_axis(0.7)
                    .draw();
                d.barplot([3, 5, 2])
                    .names(["a", "b", "c"])
                    .axisnames(false)
                    .horiz(true)
                    .draw();
            },
        },
        Case {
            name: "boxplot_inline_axis_pars",
            r: "par(mfrow = c(1, 2))\n\
                boxplot(list(a = c(1, 2, 3, 9), b = c(2, 3, 4)), las = 2, cex.axis = 1.3,\n\
                        col.axis = 'darkgreen', yaxt = 'n')\n\
                boxplot(list(a = c(1, 2, 3, 9), b = c(2, 3, 4)), horizontal = TRUE, las = 1,\n\
                        xaxp = c(0, 10, 2), gap.axis = 0)",
            ours: |d| {
                d.par().mfrow(1, 2).draw();
                let groups = [vec![1.0, 2.0, 3.0, 9.0], vec![2.0, 3.0, 4.0]];
                d.boxplot(&groups)
                    .names(["a", "b"])
                    .las(2)
                    .cex_axis(1.3)
                    .col_axis("darkgreen")
                    .yaxt("n")
                    .draw();
                d.boxplot(&groups)
                    .names(["a", "b"])
                    .horizontal(true)
                    .las(1)
                    .xaxp([0, 10, 2])
                    .gap_axis(0)
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
fn bty_and_title_pars_match_r() {
    if !common::r_available() {
        return;
    }
    let dir = std::env::temp_dir();
    let cases = cases();
    let mut script = format!(
        "x <- {}\ny <- {}\nh <- {}\n",
        r_vec(&X),
        r_vec(&Y),
        r_vec(&H)
    );
    for c in &cases {
        let path = dir.join(format!("rs_graphics_bty_title_{}.pdf", c.name));
        script.push_str(&format!(
            "pdf(file = '{}', width = {PAGE_W}, height = {PAGE_H}, compress = FALSE)\n\
             {}\ninvisible(dev.off())\n",
            path.display(),
            c.r
        ));
    }
    let script_path = dir.join("rs_graphics_bty_title.R");
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
        let path = dir.join(format!("rs_graphics_bty_title_{}.pdf", c.name));
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
            assert!(
                close(&[a.origin.0, a.origin.1], &[b.origin.0, b.origin.1], 0.05),
                "origin differs. {ctx}"
            );
        }
    }
}

/// An `NA` (`NaN`) `mtext()` line leaves that label out. R writes it with a
/// `nan` coordinate, which makes the PDF invalid (an R bug, not copied).
#[test]
fn mtext_nan_line_skips_the_label() {
    let mut dev = Device::new(
        PdfBackend::new(PAGE_W, PAGE_H).with_compression(false),
        Par::default(),
    );
    dev.plot_new();
    dev.mtext(["p", "q"], 2).line([1.0, f64::NAN]).draw();
    let pdf = String::from_utf8_lossy(&dev.into_backend().finish()).into_owned();
    let (_, texts) = interpret(&pdf);
    let strings: Vec<&str> = texts.iter().map(|t| t.string.as_str()).collect();
    assert_eq!(strings, ["p"]);
    for t in &texts {
        assert!(t.origin.0.is_finite() && t.origin.1.is_finite());
    }
}

/// `plot = FALSE` returns what drawing returns, and draws nothing.
#[test]
fn plot_false_returns_the_result_without_drawing() {
    let blank = || {
        Device::new(
            PdfBackend::new(PAGE_W, PAGE_H).with_compression(false),
            Par::default(),
        )
    };
    let (mut a, mut b) = (blank(), blank());
    assert_eq!(
        a.barplot([3, 5, 2]).plot(false).draw(),
        b.barplot([3, 5, 2]).draw()
    );
    let h = a.hist(&H).plot(false).draw();
    let drawn = b.hist(&H).draw();
    assert_eq!((h.breaks, h.counts), (drawn.breaks, drawn.counts));
    let (paths, texts) = interpret(&String::from_utf8_lossy(&a.into_backend().finish()));
    assert!(paths.is_empty() && texts.is_empty());
}
