// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks `pie()` against a real R `pdf()` device's uncompressed
//! output: every slice polygon, label tick line and text item in order
//! (see `common::pdf::assert_same`), for square, wide and tall pages,
//! `clockwise`/`init.angle`/`radius`, and `par(xaxs = "i")` (which
//! exercises `plot.window(asp = 1)`'s own range widening). Skipped if
//! `Rscript` isn't available.

use crate::common;
use crate::common::{pdf, r};

use rs_graphics::graphics::Graphics;
use rs_graphics::highlevel::PieOptions;
use rs_graphics::par::{AxisStyle, Par};

struct Case {
    name: &'static str,
    width: f64,
    height: f64,
    /// Extra `pie()` arguments in R.
    r_args: &'static str,
    exact_axes: bool,
    clockwise: bool,
    /// `None`: R's default (90 when clockwise, else 0).
    init_angle: Option<f64>,
    radius: f64,
}

fn cases() -> Vec<Case> {
    let base = Case {
        name: "",
        width: 6.0,
        height: 6.0,
        r_args: "",
        exact_axes: false,
        clockwise: false,
        init_angle: None,
        radius: 0.8,
    };
    vec![
        Case {
            name: "square",
            ..base
        },
        Case {
            name: "wide",
            width: 7.0,
            height: 4.0,
            ..base
        },
        Case {
            name: "tall",
            width: 4.0,
            height: 7.0,
            ..base
        },
        Case {
            name: "clockwise_radius",
            r_args: ", clockwise = TRUE, radius = 1",
            clockwise: true,
            radius: 1.0,
            ..base
        },
        Case {
            name: "init_angle_exact_axes",
            width: 6.5,
            height: 5.0,
            r_args: ", init.angle = 45",
            exact_axes: true,
            init_angle: Some(45.0),
            ..base
        },
    ]
}

const VALUES: [f64; 5] = [18.0, 25.0, 12.0, 30.0, 15.0];
const LABELS: [&str; 5] = ["Alpha", "Beta", "Gamma", "Delta", "Epsilon"];

#[test]
fn pie_matches_r() {
    if !common::r_available() {
        return;
    }
    let cases = cases();
    let codes: Vec<String> = cases
        .iter()
        .map(|c| {
            format!(
                "par(xaxs = '{ax}', yaxs = '{ax}')\n\
                 pie(c(18, 25, 12, 30, 15), labels = c('Alpha', 'Beta', 'Gamma', 'Delta', 'Epsilon'), \
                 main = 'pie()'{})",
                c.r_args,
                ax = if c.exact_axes { "i" } else { "r" },
            )
        })
        .collect();
    let r_cases: Vec<_> = cases
        .iter()
        .zip(&codes)
        .map(|(c, r)| (c.name, (c.width, c.height), r.as_str()))
        .collect();
    let r = r::run_r_sized("pie", "", &r_cases);
    for (c, r_pdf) in cases.iter().zip(&r.pdfs) {
        let mut par = Par::default();
        if c.exact_axes {
            par.xaxs = AxisStyle::Exact;
            par.yaxs = AxisStyle::Exact;
        }
        let opts = PieOptions {
            labels: Some(LABELS.iter().map(|&s| s.into()).collect()),
            main: Some("pie()".into()),
            clockwise: c.clockwise,
            init_angle: c.init_angle,
            radius: Some(c.radius),
            ..PieOptions::default()
        };
        let (ours, ()) = r::ours_with((c.width, c.height), par, |dev| {
            dev.pie().x(VALUES).with(&opts).draw();
        });
        pdf::assert_same_pdf(
            c.name,
            pdf::parse(&ours),
            pdf::parse(r_pdf),
            &pdf::Compare::default(),
        );
    }
}
