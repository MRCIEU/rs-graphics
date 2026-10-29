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
//! every painted path and text item in order (see
//! `common::pdf::assert_same`), and `legend()`'s return value. Skipped if
//! `Rscript` isn't available.

use crate::common;
use crate::common::{pdf, r};

use rs_graphics::prelude::*;

const PAGE_W: f64 = 7.0;
const PAGE_H: f64 = 5.0;

const X: [f64; 5] = [1.0, 2.0, 3.0, 4.0, 5.0];
const Y: [f64; 5] = [2.0, 4.0, 3.0, 5.0, 1.0];

type Draw = fn(&mut Device<AnyBackend>) -> LegendLayout;

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
                d.plot().x(X).y(Y).xlim([6, 0]).draw();
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
                d.plot().x(X).y(Y).ylim([6, 0]).draw();
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
                d.plot().x(X).y(Y).xlim([6, 0]).ylim([6, 0]).draw();
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
                d.plot().x(X).y(Y).xlim([6, 0]).draw();
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
                d.plot().x(X).y(Y).xlim([6, 0]).draw();
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
            // Any `bty` but "n" draws the whole box; "n" draws none.
            name: "bty_values",
            r: "plot(x, y, ann = FALSE)\n\
                legend('topleft', c('a', 'b'), pch = 1:2, bty = 'l')\n\
                legend('top', c('a', 'b'), pch = 1:2, bty = 'n')\n\
                l <- legend('topright', c('a', 'b'), pch = 1:2, bty = '7')",
            ours: |d| {
                d.plot().x(X).y(Y).draw();
                let keys = LegendOptions::default().pch([1, 2]);
                d.legend()
                    .pos("topleft")
                    .labels(["a", "b"])
                    .with(&keys)
                    .bty("l")
                    .draw();
                d.legend()
                    .pos("top")
                    .labels(["a", "b"])
                    .with(&keys)
                    .bty("n")
                    .draw();
                d.legend()
                    .pos("topright")
                    .labels(["a", "b"])
                    .with(&keys)
                    .bty("7")
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
                    .text_width(None)
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
                    .vadj(None)
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
    let cases = cases();
    let page = (PAGE_W, PAGE_H);
    // Each case prints its return value: box, then text positions.
    let codes: Vec<String> = cases
        .iter()
        .map(|c| {
            format!(
                "{}\ncat(unlist(l$rect[c('left', 'top', 'w', 'h')]), l$text$x, l$text$y, '\\n')",
                c.r
            )
        })
        .collect();
    let r_cases: Vec<_> = cases
        .iter()
        .zip(&codes)
        .map(|(c, r)| (c.name, r.as_str()))
        .collect();
    let prelude = format!("x <- {}\ny <- {}", r::r_vec(&X), r::r_vec(&Y));
    let r = r::run_r("legend_extras", page, &prelude, &r_cases);
    let r_values: Vec<&str> = r.stdout.lines().collect();
    assert_eq!(r_values.len(), cases.len());
    for ((c, r_pdf), r_value) in cases.iter().zip(&r.pdfs).zip(r_values) {
        let (ours_pdf, (layout, usr)) = r::ours(page, |dev| {
            let layout = (c.ours)(dev);
            (layout, dev.usr())
        });
        let ours = pdf::parse(&ours_pdf);
        let theirs = pdf::parse(r_pdf);
        let name = c.name;
        pdf::assert_same_pdf(name, ours, theirs, &pdf::Compare::default());
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
    }
}
