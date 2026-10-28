// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks `xpd = NA` (clip to the whole device) against a real R
//! `pdf()` device's uncompressed output, on a two-panel page where the
//! figure and the device differ: `par(xpd = NA)` for lines, points and
//! text reaching into the other panel, `abline()` spanning the device,
//! `axis()` ticks outside the figure, inline `xpd = NA` on `text()`,
//! `legend()` and `abline()` (also `TRUE`, `FALSE` over `par(xpd)` and
//! `untf` on a log axis), and `TRUE` next to `NA`. Compares every painted
//! path and text item in order, with the clip rectangle in effect for
//! each (see `common::pdf::assert_same`). Skipped if `Rscript` isn't
//! available.

use crate::common;
use crate::common::{pdf, r};

use rs_graphics::prelude::*;

const PAGE_W: f64 = 8.0;
const PAGE_H: f64 = 5.0;

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
            name: "par_xpd_na_primitives",
            r: "par(mfrow = c(1, 2)); plot(x, y, ann = FALSE); par(xpd = NA)\n\
                lines(c(1, 14), c(1, 6)); points(c(9, 12), c(3, 4), pch = 19)\n\
                text(10, 2, 'abc')",
            ours: |d| {
                d.par().mfrow(1, 2).draw();
                d.plot().x(X).y(Y).draw();
                d.par().xpd(Xpd::Device).draw();
                d.lines().x([1, 14]).y([1, 6]).draw();
                d.points().x([9, 12]).y([3, 4]).pch(19).draw();
                d.text().x([10]).y([2]).labels(["abc"]).draw();
            },
        },
        Case {
            name: "par_xpd_na_abline_axis",
            r: "par(mfrow = c(1, 2)); plot(x, y, ann = FALSE); par(xpd = NA)\n\
                abline(h = 3, v = 2, col = 'red')\n\
                axis(3, at = c(-4, 0, 3, 8, 12))",
            ours: |d| {
                d.par().mfrow(1, 2).draw();
                d.plot().x(X).y(Y).draw();
                d.par().xpd(Xpd::Device).draw();
                d.abline().h(3).v(2).col("red").draw();
                d.axis().side(3).at([-4, 0, 3, 8, 12]).draw();
            },
        },
        Case {
            name: "inline_xpd_na",
            r: "par(mfrow = c(1, 2)); plot(x, y, ann = FALSE)\n\
                text(11, 3, 'abc', xpd = NA); text(11, 4, 'sum', xpd = TRUE)\n\
                legend(8, 5, legend = c('abc', 'sum'), pch = 1:2, xpd = NA)",
            ours: |d| {
                d.par().mfrow(1, 2).draw();
                d.plot().x(X).y(Y).draw();
                d.text()
                    .x([11])
                    .y([3])
                    .labels(["abc"])
                    .xpd(Xpd::Device)
                    .draw();
                d.text().x([11]).y([4]).labels(["sum"]).xpd(true).draw();
                d.legend()
                    .pos((8, 5))
                    .labels(["abc", "sum"])
                    .pch([1, 2])
                    .xpd(Xpd::Device)
                    .draw();
            },
        },
        Case {
            name: "xpd_true_vs_na_abline",
            r: "par(mfrow = c(1, 2), xpd = TRUE); plot(x, y, ann = FALSE)\n\
                abline(h = 2, col = 'blue'); par(xpd = NA); abline(h = 4, col = 'red')",
            ours: |d| {
                d.par().mfrow(1, 2).xpd(true).draw();
                d.plot().x(X).y(Y).draw();
                d.abline().h(2).col("blue").draw();
                d.par().xpd(Xpd::Device).draw();
                d.abline().h(4).col("red").draw();
            },
        },
        Case {
            name: "barplot_xpd",
            r: "par(mfrow = c(1, 2))\n\
                barplot(c(3, 5, 2), ylim = c(0, 4), xpd = FALSE)\n\
                barplot(c(3, 5, 2), ylim = c(0, 4))",
            ours: |d| {
                d.par().mfrow(1, 2).draw();
                d.barplot().height([3, 5, 2]).ylim(0, 4).xpd(false).draw();
                d.barplot().height([3, 5, 2]).ylim(0, 4).draw();
            },
        },
        Case {
            name: "inline_xpd_abline",
            r: "par(mfrow = c(1, 2)); plot(x, y, ann = FALSE)\n\
                abline(h = 2, xpd = TRUE, col = 'blue'); abline(v = 3, xpd = NA, col = 'red')\n\
                abline(a = 1, b = 0.5, xpd = NA); abline(h = 4)\n\
                par(xpd = TRUE); abline(h = 1.5, xpd = FALSE, col = 'grey'); par(xpd = FALSE)\n\
                plot(x, y, log = 'x', ann = FALSE)\n\
                abline(a = 1, b = 0.5, untf = TRUE, xpd = NA, col = 'green')",
            ours: |d| {
                d.par().mfrow(1, 2).draw();
                d.plot().x(X).y(Y).draw();
                d.abline().h(2).xpd(true).col("blue").draw();
                d.abline().v(3).xpd(Xpd::Device).col("red").draw();
                d.abline().a(1).b(0.5).xpd(Xpd::Device).draw();
                // Inline `xpd` doesn't change `par(xpd)`.
                d.abline().h(4).draw();
                d.par().xpd(true).draw();
                d.abline().h(1.5).xpd(false).col("grey").draw();
                d.par().xpd(false).draw();
                d.plot().x(X).y(Y).xlog(true).draw();
                d.abline()
                    .a(1)
                    .b(0.5)
                    .untf(true)
                    .xpd(Xpd::Device)
                    .col("green")
                    .draw();
            },
        },
    ]
}

// -------------------------------------------------------------------
// A minimal PDF content-stream interpreter: painted paths, and text with
// font, size and fill colour.
// -------------------------------------------------------------------

#[test]
fn xpd_na_matches_r() {
    if !common::r_available() {
        return;
    }
    let cases = cases();
    let page = (PAGE_W, PAGE_H);
    let r_cases: Vec<_> = cases.iter().map(|c| (c.name, c.r)).collect();
    let prelude = format!("x <- {}\ny <- {}", r::r_vec(&X), r::r_vec(&Y));
    let r = r::run_r("xpd_na", page, &prelude, &r_cases);
    for (c, r_pdf) in cases.iter().zip(&r.pdfs) {
        let (ours_pdf, ()) = r::ours(page, c.ours);
        let ours = pdf::parse(&ours_pdf);
        let theirs = pdf::parse(r_pdf);
        pdf::assert_same_pdf(c.name, ours, theirs, &pdf::Compare::default());
    }
}
