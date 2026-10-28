// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks R's inline `lend`/`ljoin`/`lmitre` on every primitive
//! (`lines()`, `points()`, `segments()`, `arrows()`, `rect()`, `polygon()`,
//! `box()`, `abline()`) against a real R `pdf()` device's uncompressed
//! output: every painted path in order with its line cap, join, mitre
//! limit and width (see `common::pdf::assert_same`), so also that they
//! don't outlast the call. Skipped if `Rscript` isn't available.

use crate::common;
use crate::common::{pdf, r};

use rs_graphics::prelude::*;

const R_CALLS: &str = "plot.new(); plot.window(c(0, 10), c(0, 10))\n\
    lines(1:3, c(1, 5, 2), lwd = 8, lend = 'butt', ljoin = 'mitre', lmitre = 3)\n\
    points(5, 5, pch = 0, cex = 3, lwd = 4, ljoin = 'bevel')\n\
    segments(1, 8, 4, 9, lwd = 6, ljoin = 'mitre', lend = 'square')\n\
    arrows(6, 1, 9, 3, lwd = 3, ljoin = 'mitre', lmitre = 2, lend = 'butt')\n\
    rect(6, 6, 8, 8, lwd = 5, ljoin = 'bevel', lty = 2, lend = 'square')\n\
    polygon(c(1, 2, 3), c(6, 7, 6), lwd = 5, ljoin = 'mitre', lmitre = 1.5)\n\
    box(lwd = 4, ljoin = 'bevel', lend = 'butt')\n\
    abline(h = 4, lwd = 5, lend = 'butt')\n\
    lines(c(7, 8, 9), c(9, 5, 9), lwd = 6)\n";

fn ours(d: &mut Device<PdfBackend>) {
    d.plot_new();
    d.plot_window().xlim(0, 10).ylim(0, 10).draw();
    d.lines()
        .x([1, 2, 3])
        .y([1, 5, 2])
        .lwd(8)
        .lend("butt")
        .ljoin("mitre")
        .lmitre(3)
        .draw();
    d.points()
        .x([5])
        .y([5])
        .pch(0)
        .cex(3)
        .lwd(4)
        .ljoin("bevel")
        .draw();
    d.segments()
        .x0([1])
        .y0([8])
        .x1([4])
        .y1([9])
        .lwd(6)
        .ljoin("mitre")
        .lend("square")
        .draw();
    d.arrows()
        .x0([6])
        .y0([1])
        .x1([9])
        .y1([3])
        .lwd(3)
        .ljoin("mitre")
        .lmitre(2)
        .lend("butt")
        .draw();
    d.rect()
        .xleft([6])
        .ybottom([6])
        .xright([8])
        .ytop([8])
        .lwd(5)
        .ljoin("bevel")
        .lty("dashed")
        .lend("square")
        .draw();
    d.polygon()
        .x([1, 2, 3])
        .y([6, 7, 6])
        .lwd(5)
        .ljoin("mitre")
        .lmitre(1.5)
        .draw();
    d.box_().lwd(4).ljoin("bevel").lend("butt").draw();
    d.abline().h(4).lwd(5).lend("butt").draw();
    // Back to `par()`'s values.
    d.lines().x([7, 8, 9]).y([9, 5, 9]).lwd(6).draw();
}

#[test]
fn inline_line_pars_match_r() {
    if !common::r_available() {
        return;
    }
    let page = (6.0, 6.0);
    let r_pdf = r::run_r("line_pars", page, "", &[("all", R_CALLS)])
        .pdfs
        .remove(0);
    let (ours, ()) = r::ours(page, ours);
    // R writes widths to 2 decimals.
    let how = pdf::Compare {
        lwd: 0.0051,
        ..Default::default()
    };
    pdf::assert_same_pdf("line_pars", pdf::parse(&ours), pdf::parse(&r_pdf), &how);
}

#[test]
#[should_panic(expected = "invalid value specified for graphical parameter \"lmitre\"")]
fn lmitre_below_one_panics_like_r() {
    let mut dev = Device::new(PdfBackend::new(6, 6), Par::default());
    dev.plot_new();
    dev.lines().x([1, 2]).y([1, 2]).lmitre(0.5).draw();
}
