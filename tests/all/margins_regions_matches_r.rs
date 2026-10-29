// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks the margin and region parameters in other units against
//! a real R `pdf()` device's uncompressed output: `mai` (also with `cex`
//! changed, and `mar` resetting it), `omi` and `omd` (with outer
//! `mtext()`, and `omi` with `mfrow`), `pin` (and `mar` resetting it),
//! `pty` resetting `plt`, `fin`, and `par(oma = )` starting a new page
//! ("force eject"). Compares every page's painted paths and text items in
//! order (see `common::pdf::assert_same`). Skipped if `Rscript` isn't
//! available.

use crate::common;
use crate::common::{pdf, r};

use rs_graphics::prelude::*;

const PAGE_W: f64 = 8.0;
const PAGE_H: f64 = 6.0;

const X: [f64; 5] = [1.0, 2.0, 3.0, 4.0, 5.0];
const Y: [f64; 5] = [2.0, 4.0, 3.0, 5.0, 1.0];

type Draw = fn(&mut Device<AnyBackend>);

struct Case {
    name: &'static str,
    /// R code; `x` and `y` are the vectors above.
    r: &'static str,
    ours: Draw,
}

fn titled(d: &mut Device<AnyBackend>) {
    d.plot()
        .x(X)
        .y(Y)
        .main("title")
        .xlab("xlab")
        .ylab("ylab")
        .draw();
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "mai",
            r: "par(mai = c(1, 1.2, 0.6, 0.3)); plot(x, y, main = 'title', xlab = 'xlab', ylab = 'ylab')",
            ours: |d| {
                d.par().mai([1.0, 1.2, 0.6, 0.3]).draw();
                titled(d);
            },
        },
        Case {
            name: "mai_with_cex",
            r: "par(mai = c(1, 1, 0.8, 0.4), cex = 0.8)\n\
                plot(x, y, main = 'title', xlab = 'xlab', ylab = 'ylab')",
            ours: |d| {
                d.par().mai([1.0, 1.0, 0.8, 0.4]).cex(0.8).draw();
                titled(d);
            },
        },
        Case {
            name: "mar_after_mai",
            r: "par(mai = c(2, 2, 2, 2)); par(mar = c(4, 4, 2, 1)); plot(x, y, ann = FALSE)",
            ours: |d| {
                d.par().mai([2, 2, 2, 2]).draw();
                d.par().mar([4, 4, 2, 1]).draw();
                d.plot().x(X).y(Y).draw();
            },
        },
        Case {
            name: "omi_outer_mtext",
            r: "par(omi = c(0.5, 0.6, 0.8, 0.2)); plot(x, y, ann = FALSE)\n\
                mtext('title', side = 3, outer = TRUE, line = 1)",
            ours: |d| {
                d.par().omi([0.5, 0.6, 0.8, 0.2]).draw();
                d.plot().x(X).y(Y).draw();
                d.mtext().text("title").side(3).outer(true).line(1).draw();
            },
        },
        Case {
            name: "omd_outer_mtext",
            r: "par(omd = c(0.1, 0.9, 0.05, 0.9)); plot(x, y, ann = FALSE)\n\
                mtext('title', outer = TRUE, line = 0.5)",
            ours: |d| {
                d.par().omd([0.1, 0.9, 0.05, 0.9]).draw();
                d.plot().x(X).y(Y).draw();
                d.mtext().text("title").side(3).outer(true).line(0.5).draw();
            },
        },
        Case {
            name: "mfrow_omi",
            r: "par(mfrow = c(2, 2), omi = c(0, 0, 0.5, 0))\n\
                for (i in 1:4) plot(x, y, ann = FALSE)\n\
                mtext('title', outer = TRUE, line = 1)",
            ours: |d| {
                d.par().mfrow(2, 2).omi([0.0, 0.0, 0.5, 0.0]).draw();
                for _ in 0..4 {
                    d.plot().x(X).y(Y).draw();
                }
                d.mtext().text("title").side(3).outer(true).line(1).draw();
            },
        },
        Case {
            name: "pin",
            r: "par(pin = c(4, 3)); plot(x, y, main = 'title', xlab = 'xlab', ylab = 'ylab')",
            ours: |d| {
                d.par().pin([4, 3]).draw();
                titled(d);
            },
        },
        Case {
            name: "mar_after_pin",
            r: "par(pin = c(3, 2)); par(mar = c(5, 4, 4, 2)); plot(x, y, ann = FALSE)",
            ours: |d| {
                d.par().pin([3, 2]).draw();
                d.par().mar([5, 4, 4, 2]).draw();
                d.plot().x(X).y(Y).draw();
            },
        },
        Case {
            name: "pty_resets_plt",
            r: "par(plt = c(0.2, 0.8, 0.3, 0.7)); par(pty = 's'); plot(x, y, ann = FALSE)",
            ours: |d| {
                d.par().plt([0.2, 0.8, 0.3, 0.7]).draw();
                d.par().pty("s").draw();
                d.plot().x(X).y(Y).draw();
            },
        },
        Case {
            name: "fin",
            r: "par(fin = c(5, 4)); plot(x, y, main = 'title', xlab = 'xlab', ylab = 'ylab')",
            ours: |d| {
                d.par().fin([5, 4]).draw();
                titled(d);
            },
        },
        Case {
            name: "oma_ejects",
            r: "par(mfrow = c(1, 2)); plot(x, y, ann = FALSE)\n\
                par(oma = c(0, 0, 2, 0)); plot(x, y, ann = FALSE)",
            ours: |d| {
                d.par().mfrow(1, 2).draw();
                d.plot().x(X).y(Y).draw();
                d.par().oma([0, 0, 2, 0]).draw();
                d.plot().x(X).y(Y).draw();
            },
        },
    ]
}

// -------------------------------------------------------------------
// A minimal PDF content-stream interpreter: painted paths, and text with
// font, size and fill colour.
// -------------------------------------------------------------------

#[test]
fn margins_and_regions_match_r() {
    if !common::r_available() {
        return;
    }
    let cases = cases();
    let page = (PAGE_W, PAGE_H);
    let r_cases: Vec<_> = cases.iter().map(|c| (c.name, c.r)).collect();
    let prelude = format!("x <- {}\ny <- {}", r::r_vec(&X), r::r_vec(&Y));
    let r = r::run_r("regions_units", page, &prelude, &r_cases);
    for (c, r_pdf) in cases.iter().zip(&r.pdfs) {
        let (ours_pdf, ()) = r::ours(page, c.ours);
        let ours = pdf::parse(&ours_pdf);
        let theirs = pdf::parse(r_pdf);
        pdf::assert_same_pdf(c.name, ours, theirs, &pdf::Compare::default());
    }
}
