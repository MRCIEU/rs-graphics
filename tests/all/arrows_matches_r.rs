// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks `arrows()` against a real R `pdf()` device's uncompressed
//! output: every shaft and head in order (see `common::pdf::assert_same`),
//! for heads at the start, end and both ends, `angle = 90` error-bar caps,
//! steep/diagonal shafts, a skipped `NA` segment and a zero-length arrow
//! (drawn without heads).
//! Skipped if `Rscript` isn't available.

use crate::common;
use crate::common::{pdf, r};

use rs_graphics::prelude::*;

/// `(x0, y0, x1, y1, length, angle, code)` per `arrows()` call.
type Call = (&'static [f64], &'static [f64], &'static [f64], &'static [f64], f64, f64, u8);

const CALLS: &[Call] = &[
    // R's defaults.
    (&[1.0], &[1.0], &[4.0], &[2.0], 0.25, 30.0, 2),
    (&[2.0, 6.0], &[8.0, 9.0], &[2.0, 9.0], &[5.0, 9.0], 0.1, 20.0, 1),
    // Horizontal and vertical error bars, with an NA in the middle.
    (&[5.0, 6.0, 7.0], &[3.0, f64::NAN, 4.0], &[8.0, 6.5, 9.5], &[3.0, 6.0, 4.0], 0.05, 90.0, 3),
    (&[3.0], &[1.0], &[3.0], &[6.0], 0.05, 90.0, 3),
    // Steep diagonal, both ends, wide angle.
    (&[9.0], &[1.0], &[8.0], &[7.0], 0.2, 60.0, 3),
    // Zero length: shaft only.
    (&[5.0], &[5.0], &[5.0], &[5.0], 0.25, 30.0, 2),
];

fn r_vec(v: &[f64]) -> String {
    let items: Vec<String> = v
        .iter()
        .map(|x| if x.is_nan() { "NA".to_string() } else { x.to_string() })
        .collect();
    format!("c({})", items.join(", "))
}

#[test]
fn arrows_match_r() {
    if !common::r_available() {
        return;
    }
    let page = (7.0, 5.0);
    let mut script = String::from("plot.new(); plot.window(c(0, 10), c(0, 10))\n");
    for (x0, y0, x1, y1, length, angle, code) in CALLS {
        script.push_str(&format!(
            "suppressWarnings(arrows({}, {}, {}, {}, length = {length}, angle = {angle}, code = {code}))\n",
            r_vec(x0),
            r_vec(y0),
            r_vec(x1),
            r_vec(y1)
        ));
    }
    let r_pdf = r::run_r("arrows", page, "", &[("all", &script)])
        .pdfs
        .remove(0);
    let (ours, ()) = r::ours(page, |dev| {
        dev.plot_new();
        dev.plot_window().xlim(0.0, 10.0).ylim(0.0, 10.0).draw();
        for &(x0, y0, x1, y1, length, angle, code) in CALLS {
            dev.arrows()
                .x0(x0)
                .y0(y0)
                .x1(x1)
                .y1(y1)
                .length(length)
                .angle(angle)
                .code(code)
                .draw();
        }
    });
    let (ours, r) = (pdf::parse(&ours), pdf::parse(&r_pdf));
    assert!(r.pages[0].paths.len() > 10);
    pdf::assert_same_pdf("arrows", ours, r, &pdf::Compare::default());
}
