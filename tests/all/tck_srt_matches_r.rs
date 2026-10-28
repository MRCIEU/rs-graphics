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
//! real R `pdf()` device's uncompressed output. Compares every painted
//! path and text item in order, text rotation included (see
//! `common::pdf::assert_same`). Skipped if `Rscript` isn't available.

use crate::common;
use crate::common::{pdf, r};

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
                d.plot().x(X).y(Y).draw();
            },
        },
        Case {
            name: "par_tck_grid",
            r: "par(tck = 1); plot(x, y, ann = FALSE)",
            ours: |d| {
                d.par().tck(1).draw();
                d.plot().x(X).y(Y).draw();
            },
        },
        Case {
            name: "inline_tck_overrides_par",
            r: "par(tck = -0.05); plot(x, y, ann = FALSE, axes = FALSE)\n\
                axis(1); axis(2, tck = 0.03); box()",
            ours: |d| {
                d.par().tck(-0.05).draw();
                d.plot().x(X).y(Y).axes(false).draw();
                d.axis().side(1).draw();
                d.axis().side(2).tck(0.03).draw();
                d.box_().draw();
            },
        },
        Case {
            name: "par_tck_na_resets",
            r: "par(tck = 0.1); par(tck = NA); plot(x, y, ann = FALSE)",
            ours: |d| {
                d.par().tck(0.1).draw();
                d.par().tck(None).draw();
                d.plot().x(X).y(Y).draw();
            },
        },
        Case {
            name: "par_srt_text",
            r: "par(srt = 45); plot(x, y, ann = FALSE, type = 'n')\n\
                text(x, y, labels = c('a', 'b', 'c', 'd', 'e'))\n\
                text(3, 3, 'f', srt = -30)",
            ours: |d| {
                d.par().srt(45).draw();
                d.plot().x(X).y(Y).type_("n").draw();
                d.text().x(X).y(Y).labels(["a", "b", "c", "d", "e"]).draw();
                d.text().x([3]).y([3]).labels(["f"]).srt(-30).draw();
            },
        },
        Case {
            name: "par_srt_pie",
            r: "par(srt = 30); pie(c(1, 2, 3), labels = c('a', 'b', 'c'))",
            ours: |d| {
                d.par().srt(30).draw();
                d.pie().x([1, 2, 3]).labels(["a", "b", "c"]).draw();
            },
        },
        Case {
            name: "pie_inline_srt",
            r: "par(srt = 30); pie(c(1, 2, 3), labels = c('a', 'b', 'c'), srt = -45)",
            ours: |d| {
                d.par().srt(30).draw();
                d.pie().x([1, 2, 3]).labels(["a", "b", "c"]).srt(-45).draw();
            },
        },
        Case {
            name: "par_srt_legend",
            r: "plot(x, y, ann = FALSE); par(srt = 20)\n\
                legend('topleft', legend = c('abc', 'sum'), pch = 1:2, title = 'title')",
            ours: |d| {
                d.plot().x(X).y(Y).draw();
                d.par().srt(20).draw();
                d.legend()
                    .pos("topleft")
                    .labels(["abc", "sum"])
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

#[test]
fn tck_and_srt_match_r() {
    if !common::r_available() {
        return;
    }
    let cases = cases();
    let page = (PAGE_W, PAGE_H);
    let r_cases: Vec<_> = cases.iter().map(|c| (c.name, c.r)).collect();
    let prelude = format!("x <- {}\ny <- {}", r::r_vec(&X), r::r_vec(&Y));
    let r = r::run_r("tck_srt", page, &prelude, &r_cases);
    for (c, r_pdf) in cases.iter().zip(&r.pdfs) {
        let (ours_pdf, ()) = r::ours(page, c.ours);
        let ours = pdf::parse(&ours_pdf);
        let theirs = pdf::parse(r_pdf);
        pdf::assert_same_pdf(c.name, ours, theirs, &pdf::Compare::default());
    }
}
