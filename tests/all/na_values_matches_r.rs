// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks `None` elements (R's `NA`) in the recycled style vectors
//! against a real R `pdf()` device's uncompressed output: every painted
//! path and text item in order (see `common::pdf::assert_same`).
//!
//! Where R defines `NA` (a skipped point, segment, line or symbol, or the
//! default text size and font), the R code passes `NA`. Where R errors
//! (`lty = NA` outside `legend()`, legend `cex`) or leaves it undefined
//! (`lwd = NA` for points), `None` still skips the element (or, for text
//! sizes, fonts and legend spacing, means the default), and the R code
//! draws that instead: the same call without the element, or with the
//! default. Skipped if `Rscript` isn't available.

use crate::common;
use crate::common::{pdf, r};

use rs_graphics::prelude::*;

const PAGE_W: f64 = 7.0;
const PAGE_H: f64 = 5.0;

const X: [f64; 5] = [1.0, 2.0, 3.0, 4.0, 5.0];
const Y: [f64; 5] = [2.0, 4.0, 3.0, 5.0, 1.0];

struct Case {
    name: &'static str,
    /// R code; `x` and `y` are the vectors above.
    r: &'static str,
    ours: fn(&mut Device<AnyBackend>),
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "points",
            r: "plot(x, y, ann = FALSE, pch = c(1, NA, 3, 19, 2), cex = c(1, 1, NA, 2, 1), \
                col = c(1, 2, 3, NA, 4))",
            ours: |d| {
                d.plot()
                    .x(X)
                    .y(Y)
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
                d.plot().x(X).y(Y).type_("n").draw();
                d.points()
                    .x(X)
                    .y(Y)
                    .lwd([Some(2), None, Some(3), Some(1), Some(2)])
                    .draw();
            },
        },
        Case {
            // R uses `par(lwd)` for `lines(lwd = NA)`.
            name: "lines_lwd",
            r: "plot(x, y, ann = FALSE, type = 'n'); par(lwd = 2); lines(x, y, lwd = NA)",
            ours: |d| {
                d.plot().x(X).y(Y).type_("n").draw();
                d.par().lwd(2).draw();
                d.lines().x(X).y(Y).lwd([None::<f64>]).draw();
            },
        },
        Case {
            name: "segments_arrows",
            r: "plot(x, y, ann = FALSE, type = 'n')\n\
                segments(x, y, x + 0.5, y + 0.5, lwd = c(2, NA, 1, 1, 3), col = c(1, 2, NA, 4, 5))\n\
                arrows(x, y - 0.5, x + 0.3, y - 0.2, lwd = c(NA, 2))",
            ours: |d| {
                d.plot().x(X).y(Y).type_("n").draw();
                let (x1, y1) = (X.map(|v| v + 0.5), Y.map(|v| v + 0.5));
                d.segments()
                    .x0(X)
                    .y0(Y)
                    .x1(x1)
                    .y1(y1)
                    .lwd([Some(2), None, Some(1), Some(1), Some(3)])
                    .col([Some(1), Some(2), None, Some(4), Some(5)])
                    .draw();
                let (y0, x2, y2) = (Y.map(|v| v - 0.5), X.map(|v| v + 0.3), Y.map(|v| v - 0.2));
                d.arrows()
                    .x0(X)
                    .y0(y0)
                    .x1(x2)
                    .y1(y2)
                    .lwd([None, Some(2)])
                    .draw();
            },
        },
        Case {
            // R errors for `lty = NA`.
            name: "segments_lty",
            r: "plot(x, y, ann = FALSE, type = 'n'); segments(x[-4], y[-4], x[-4] + 0.5, y[-4], lty = c(1, 2, 3, 5))",
            ours: |d| {
                d.plot().x(X).y(Y).type_("n").draw();
                d.segments()
                    .x0(X)
                    .y0(Y)
                    .x1(X.map(|v| v + 0.5))
                    .y1(Y)
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
                d.plot().x(X).y(Y).type_("n").draw();
                d.rect()
                    .xleft(X)
                    .ybottom(Y)
                    .xright(X.map(|v| v + 0.4))
                    .ytop(Y.map(|v| v + 0.3))
                    .col("grey")
                    .lwd([Some(1), None, Some(2), Some(1), Some(1)])
                    .draw();
                d.rect()
                    .xleft([1.0, 2.0, 3.0])
                    .ybottom([0.5])
                    .xright([1.4, 2.4, 3.4])
                    .ytop([0.8])
                    .lty([Some(1), None, Some(3)])
                    .draw();
                d.rect()
                    .xleft([2.0])
                    .ybottom([0.5])
                    .xright([2.4])
                    .ytop([0.8])
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
                d.plot().x(X).y(Y).type_("n").draw();
                let nan = f64::NAN;
                d.polygon()
                    .x([1.0, 2.0, 2.0, nan, 3.0, 4.0, 4.0, nan, 4.0, 5.0, 5.0])
                    .y([1.0, 1.0, 2.0, nan, 1.0, 1.0, 2.0, nan, 1.0, 1.0, 2.0])
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
                d.plot().x(X).y(Y).type_("n").draw();
                d.text()
                    .x(X)
                    .y(Y)
                    .labels(["a", "b", "c", "d", "e"])
                    .cex([Some(1), None, Some(2)])
                    .font([Some(2), Some(3), None])
                    .draw();
                d.mtext()
                    .text(["m", "n"])
                    .side(3)
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
                d.plot().x(X).y(Y).type_("n").draw();
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
                d.boxplot()
                    .x(&g[..])
                    .medpch([Some(19), None, Some(1)])
                    .medlwd([Some(3), None, Some(2)])
                    .outpch([None, Some(19)])
                    .outcex([Some(1), Some(2)])
                    .draw();
                let g2 = [
                    (1..=10).map(f64::from).collect::<Vec<_>>(),
                    (2..=12).map(f64::from).collect(),
                ];
                d.boxplot()
                    .x(&g2[..])
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
                d.hist()
                    .x([1.0, 2.0, 2.0, 3.0, 3.0, 3.0, 4.0, 4.0])
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
                d.plot().x(X).y(Y).type_("n").draw();
                d.legend()
                    .pos("topleft")
                    .labels(["a", "mm", "n"])
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
                d.plot().x(X).y(Y).type_("n").draw();
                d.legend()
                    .pos("bottomright")
                    .labels(["a", "mm", "n"])
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

#[test]
fn na_values_match_r() {
    if !common::r_available() {
        return;
    }
    let cases = cases();
    let page = (PAGE_W, PAGE_H);
    let r_cases: Vec<_> = cases.iter().map(|c| (c.name, c.r)).collect();
    let prelude = format!("x <- {}\ny <- {}", r::r_vec(&X), r::r_vec(&Y));
    let r = r::run_r("na_values", page, &prelude, &r_cases);
    for (c, r_pdf) in cases.iter().zip(&r.pdfs) {
        let (ours_pdf, ()) = r::ours(page, c.ours);
        let ours = pdf::parse(&ours_pdf);
        let theirs = pdf::parse(r_pdf);
        pdf::assert_same_pdf(c.name, ours, theirs, &pdf::Compare::default());
    }
}
