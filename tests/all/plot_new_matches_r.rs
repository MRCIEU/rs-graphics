// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks the user coordinates `plot.new()` leaves without a
//! `plot.window()` call (R's `C_plot_new`: linear axes, `GScale(0, 1)` on
//! both, so -0.04..1.04 with `xaxs = "r"`), including after a log plot and
//! with `xaxs = yaxs = "i"`. Compares every painted path and text item in
//! order with an uncompressed R `pdf()` (see `common::pdf::assert_same`).
//! Skipped if `Rscript` isn't available.

use crate::common;
use crate::common::{pdf, r};


use rs_graphics::color::ColorSpec;
use rs_graphics::device::Device;
use rs_graphics::par::AxisStyle;
use rs_graphics::prelude::*;

const PAGE_W: f64 = 6.0;
const PAGE_H: f64 = 5.0;

struct Case {
    name: &'static str,
    r: &'static str,
    draw: fn(&mut Device<AnyBackend>),
}

fn axes(d: &mut Device<AnyBackend>, sides: &[i32]) {
    for &s in sides {
        d.axis().side(s).draw();
    }
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "default",
            r: "plot.new(); points(c(0, 0.5, 1), c(0, 0.5, 1)); axis(1); axis(2)",
            draw: |d| {
                d.plot_new();
                let v = [0.0, 0.5, 1.0];
                d.points()
                    .x(v)
                    .y(v)
                    .with(
                        &XyOptions::default()
                            .pch(vec![Some(1)])
                            .col(vec![ColorSpec::Index(1)]),
                    )
                    .draw();
                axes(d, &[1, 2]);
            },
        },
        Case {
            name: "after_log",
            r: "plot.new(); plot.window(c(1, 1000), c(1, 1000), log = 'xy'); axis(1); axis(2)\n\
                par(new = TRUE); plot.new(); axis(3); axis(4)",
            draw: |d| {
                d.plot_new();
                d.plot_window()
                    .xlim(1.0, 1000.0)
                    .ylim(1.0, 1000.0)
                    .with(&PlotWindowOptions::default().xlog(true).ylog(true))
                    .draw();
                axes(d, &[1, 2]);
                d.par().new(true).draw();
                d.plot_new();
                axes(d, &[3, 4]);
            },
        },
        Case {
            name: "xaxs_i",
            r: "par(xaxs = 'i', yaxs = 'i'); plot.new(); points(0.25, 0.75); axis(1); axis(2)",
            draw: |d| {
                d.par().xaxs(AxisStyle::Exact).draw();
                d.par().yaxs(AxisStyle::Exact).draw();
                d.plot_new();
                d.points()
                    .x([0.25])
                    .y([0.75])
                    .with(
                        &XyOptions::default()
                            .pch(vec![Some(1)])
                            .col(vec![ColorSpec::Index(1)]),
                    )
                    .draw();
                axes(d, &[1, 2]);
            },
        },
    ]
}

#[test]
fn plot_new_coordinates_match_r() {
    if !common::r_available() {
        return;
    }
    let cases = cases();
    let page = (PAGE_W, PAGE_H);
    let r_cases: Vec<_> = cases.iter().map(|c| (c.name, c.r)).collect();
    let r = r::run_r("plot_new", page, "", &r_cases);
    for (c, r_pdf) in cases.iter().zip(&r.pdfs) {
        let (ours, ()) = r::ours(page, c.draw);
        pdf::assert_same_pdf(
            c.name,
            pdf::parse(&ours),
            pdf::parse(r_pdf),
            &pdf::Compare::default(),
        );
    }
}
