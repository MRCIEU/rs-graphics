// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks semi-transparent colours on PDF against a real R `pdf()`
//! device's uncompressed output: every painted path and text item in
//! order (see `common::pdf::assert_same`), colours *with alpha*, which R
//! and we both set through `ExtGState` resources and `gs`, across clipping
//! (`q`/`Q`) and pages. Skipped if `Rscript` isn't available.

use crate::common;
use crate::common::{pdf, r};

use rs_graphics::prelude::*;

const PAGE_W: f64 = 6.0;
const PAGE_H: f64 = 5.0;

struct Case {
    name: &'static str,
    r: &'static str,
    ours: fn(&mut Device<AnyBackend>),
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

#[test]
fn semi_transparent_colours_match_r() {
    if !common::r_available() {
        return;
    }
    let cases = cases();
    let page = (PAGE_W, PAGE_H);
    let r_cases: Vec<_> = cases.iter().map(|c| (c.name, c.r)).collect();
    let prelude = String::new();
    let r = r::run_r("alpha", page, &prelude, &r_cases);
    for (c, r_pdf) in cases.iter().zip(&r.pdfs) {
        let (ours_pdf, ()) = r::ours(page, c.ours);
        let ours = pdf::parse(&ours_pdf);
        let theirs = pdf::parse(r_pdf);
        pdf::assert_same_pdf(c.name, ours, theirs, &pdf::Compare::default());
    }
}
