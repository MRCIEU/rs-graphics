// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks `par()` colours given as palette numbers (resolved when
//! the call runs, 0 as the background, `NA` as transparent), `lend`/`ljoin`
//! given as R's integer codes (recycled above 2), and `pie()`'s `edges` as
//! a number (floored per slice, at least 2) against R's `pdf()` output.
//! Skipped if `Rscript` isn't available.

use crate::common;
use crate::common::{pdf, r};

use rs_graphics::prelude::*;

// `fg` first: R's `par(fg = )` sets `col` too (ours doesn't, by design).
const PARS_R: &str = "par(fg = 3, col = 2, col.axis = 4, col.lab = 5, col.main = 6, col.sub = 7)\n\
    plot(1:3, main = 'm', sub = 's')\n\
    lines(c(1, 3), c(3, 1), lwd = 10)\n\
    par(lend = 3, ljoin = 3)\n\
    lines(c(1, 2, 3), c(1, 3, 1), lwd = 12)\n\
    par(bg = 8); par(col = 0)\n\
    points(2, 2, pch = 19, cex = 4)\n\
    par(col = NA)\n\
    points(1, 1, pch = 19, cex = 4)\n\
    par(lend = 4, ljoin = 5)\n\
    lines(c(1, 2, 3), c(2, 2.5, 2), lwd = 12, col = 'grey40')\n";

const PIE_R: &str = "pie(c(1, 2, 3), edges = 7.9)\npie(c(1, 2), edges = -5)\n";

#[test]
fn par_colours_and_codes_match_r() {
    if !common::r_available() {
        return;
    }
    let page = (6.0, 6.0);
    let mut r_pdfs = r::run_r("par_colours", page, "", &[("pars", PARS_R), ("pie", PIE_R)]).pdfs;
    let (ours, ()) = r::ours(page, |dev| {
        dev.par()
            .col(2)
            .fg(3)
            .col_axis(4)
            .col_lab(5)
            .col_main(6)
            .col_sub(7)
            .draw();
        dev.plot()
            .x([1, 2, 3])
            .main("m")
            .sub("s")
            .ylab("1:3")
            .draw();
        dev.lines().x([1, 3]).y([3, 1]).lwd(10).draw();
        dev.par().lend(3).ljoin(3).draw();
        dev.lines().x([1, 2, 3]).y([1, 3, 1]).lwd(12).draw();
        dev.par().bg(8).draw();
        dev.par().col(0).draw();
        dev.points().x([2]).y([2]).pch(19).cex(4).draw();
        dev.par().col(None::<&str>).draw();
        dev.points().x([1]).y([1]).pch(19).cex(4).draw();
        dev.par().lend(4).ljoin(5).draw();
        dev.lines()
            .x([1, 2, 3])
            .y([2.0, 2.5, 2.0])
            .lwd(12)
            .col("grey40")
            .draw();
    });
    let (ours, r) = (pdf::parse(&ours), pdf::parse(&r_pdfs.remove(0)));
    pdf::assert_same_pdf("par_colours", ours, r, &pdf::Compare::default());

    let (ours, ()) = r::ours(page, |dev| {
        dev.pie().x([1, 2, 3]).edges(7.9).draw();
        dev.pie().x([1, 2]).edges(-5).draw();
    });
    let (ours, r) = (pdf::parse(&ours), pdf::parse(&r_pdfs.remove(0)));
    pdf::assert_same_pdf("pie_edges", ours, r, &pdf::Compare::default());
}
