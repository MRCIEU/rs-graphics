// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks `plot()`'s axis tick labels, axis titles (`xlab`/`ylab`)
//! and points against a real R `pdf()` device's (uncompressed) output, for
//! each `las` setting: every painted path and text item in order (see
//! `common::pdf::assert_same`). Skipped if `Rscript` isn't available.
//!
//! `main` is included: our PDF backend replicates R's pdf-device font-size
//! rounding (`floor(cex * ps + 0.5)`, so `cex.main = 1.2` is 14pt, not 14.4pt).

use crate::common;
use crate::common::{pdf, r};

use rs_graphics::graphics::Graphics;

#[test]
fn axis_and_label_text_positions_match_r() {
    if !common::r_available() {
        return;
    }

    let x: Vec<f64> = (1..=25).map(|i| i as f64).collect();
    let y: Vec<f64> = x.iter().map(|v| (v * 0.35).sin() * 8.0 + v * 0.6).collect();
    let page = (6.0, 4.0);
    let names: Vec<String> = (0..=3).map(|las| format!("las{las}")).collect();
    let codes: Vec<String> = (0..=3)
        .map(|las| {
            format!(
                "par(las = {las}); x <- 1:25; y <- sin(x * 0.35) * 8 + x * 0.6; \
                 plot(x, y, pch = 19, main = 'Title', xlab = 'x', ylab = 'y')"
            )
        })
        .collect();
    let r_cases: Vec<_> = names
        .iter()
        .zip(&codes)
        .map(|(n, c)| (n.as_str(), c.as_str()))
        .collect();
    let r = r::run_r("axis", page, "", &r_cases);
    for (las, r_pdf) in (0..=3).zip(&r.pdfs) {
        let (ours, ()) = r::ours(page, |dev| {
            dev.par().las(las).draw();
            dev.plot()
                .x(&x)
                .y(&y)
                .pch(19)
                .main("Title")
                .xlab("x")
                .ylab("y")
                .draw();
        });
        let r = pdf::parse(r_pdf);
        assert!(
            r.pages[0].texts.len() >= 10,
            "las={las}: too few text items from R"
        );
        pdf::assert_same_pdf(
            &format!("las={las}"),
            pdf::parse(&ours),
            r,
            &pdf::Compare::default(),
        );
    }
}
