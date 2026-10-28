// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks `type = "h"`, `"s"` and `"S"` (in `plot()`, `lines()` and
//! `points()`) against a real R `pdf()` device's uncompressed output:
//! `"h"` lines from y = 0 (also when 0 is below the plot), from the bottom
//! of the plot on a log y axis, with colours recycled per line and `NA`
//! colours, `NA` values and values <= 0 on a log axis skipped; steps in
//! both directions, broken at `NA`, on linear and log axes, with `lty` and
//! `lwd`. Compares every painted path in order (see
//! `common::pdf::assert_same`). Skipped if `Rscript` isn't available.

use crate::common;
use crate::common::{pdf, r};

use rs_graphics::prelude::*;

const PAGE_W: f64 = 6.0;
const PAGE_H: f64 = 5.0;

const NA: f64 = f64::NAN;
const X: [f64; 8] = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
const Y: [f64; 8] = [3.0, -2.0, 5.0, NA, 8.0, 6.0, 9.0, 4.0];
/// For log y axes: a 0 (skipped there) and a wide range.
const Y_LOG: [f64; 8] = [3.0, 0.0, 5.0, NA, 8.0, 60.0, 900.0, 4.0];

type Draw = fn(&mut Device<PdfBackend>);

struct Case {
    name: &'static str,
    /// R calls after `pdf()`, with `axes = FALSE, ann = FALSE` on `plot()`.
    r: &'static str,
    ours: Draw,
}

/// `plot(type = type_, axes = FALSE)`, limits from the data as in R.
fn plot(type_: PlotType) -> PlotOptions {
    PlotOptions::default().type_(type_).axes(false)
}

const CASES: [Case; 6] = [
    Case {
        name: "h_colours",
        r: "plot(x, y, type = 'h', col = c('red', NA, 'blue'), lwd = 3, axes = FALSE, ann = FALSE)",
        ours: |dev| {
            let opts = plot(PlotType::Histogram);
            dev.plot()
                .x(X)
                .y(Y)
                .with(&opts)
                .col([
                    ColorSpec::try_from("red").unwrap(),
                    ColorSpec::None,
                    ColorSpec::try_from("blue").unwrap(),
                ])
                .lwd(3.0)
                .draw();
        },
    },
    Case {
        name: "h_zero_below_plot",
        r: "plot(x, y + 10, type = 'h', axes = FALSE, ann = FALSE)",
        ours: |dev| {
            let y: Vec<f64> = Y.iter().map(|v| v + 10.0).collect();
            let opts = plot(PlotType::Histogram);
            dev.plot().x(X).y(&y).with(&opts).draw();
        },
    },
    Case {
        name: "h_log",
        r: "plot(x, ylog, type = 'h', log = 'y', axes = FALSE, ann = FALSE)",
        ours: |dev| {
            let opts = plot(PlotType::Histogram).ylog(true);
            dev.plot().x(X).y(Y_LOG).with(&opts).draw();
        },
    },
    Case {
        name: "s_dashed",
        r: "plot(x, y, type = 's', lty = 2, col = 'darkgreen', axes = FALSE, ann = FALSE)",
        ours: |dev| {
            let opts = plot(PlotType::Steps);
            dev.plot()
                .x(X)
                .y(Y)
                .with(&opts)
                .lty(LineType::DASHED)
                .col("darkgreen")
                .draw();
        },
    },
    Case {
        name: "S_log_x",
        r: "plot(x, y, type = 'S', log = 'x', lwd = 2, axes = FALSE, ann = FALSE)",
        ours: |dev| {
            let opts = plot(PlotType::StepsVerticalFirst).xlog(true);
            dev.plot().x(X).y(Y).with(&opts).lwd(2.0).draw();
        },
    },
    Case {
        name: "lines_points",
        r: "plot(x, ylog, type = 'n', log = 'y', axes = FALSE, ann = FALSE)\n\
            lines(x, ylog, type = 's', col = 'blue')\n\
            points(x, ylog, type = 'h', col = c('red', 'grey'))\n\
            lines(x, ylog, type = 'S', lwd = 2)",
        ours: |dev| {
            let opts = plot(PlotType::None).ylog(true);
            dev.plot().x(X).y(Y_LOG).with(&opts).draw();
            dev.lines()
                .x(X)
                .y(Y_LOG)
                .type_(PlotType::Steps)
                .col("blue")
                .draw();
            dev.points()
                .x(X)
                .y(Y_LOG)
                .type_(PlotType::Histogram)
                .col(["red", "grey"])
                .draw();
            dev.lines()
                .x(X)
                .y(Y_LOG)
                .type_(PlotType::StepsVerticalFirst)
                .lwd(2.0)
                .draw();
        },
    },
];

fn r_vector(a: &[f64]) -> String {
    let s: Vec<String> = a
        .iter()
        .map(|x| {
            if x.is_nan() {
                "NA".into()
            } else {
                x.to_string()
            }
        })
        .collect();
    format!("c({})", s.join(", "))
}

#[test]
fn plot_types_h_s_match_r() {
    if !common::r_available() {
        return;
    }
    let page = (PAGE_W, PAGE_H);
    let prelude = format!(
        "x <- {}\ny <- {}\nylog <- {}",
        r_vector(&X),
        r_vector(&Y),
        r_vector(&Y_LOG)
    );
    let r_cases: Vec<_> = CASES.iter().map(|c| (c.name, c.r)).collect();
    let r = r::run_r("type_h_s", page, &prelude, &r_cases);
    let how = pdf::Compare {
        col: 0.002,
        // R writes widths to 2 decimals.
        lwd: 0.0051,
        ..Default::default()
    };
    for (c, r_pdf) in CASES.iter().zip(&r.pdfs) {
        let theirs = pdf::parse(r_pdf);
        assert!(
            !theirs.pages[0].paths.is_empty(),
            "{}: nothing stroked by R",
            c.name
        );
        let (ours, ()) = r::ours(page, c.ours);
        pdf::assert_same_pdf(c.name, pdf::parse(&ours), theirs, &how);
    }
}
