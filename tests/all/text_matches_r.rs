// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks `text()` placement against a real R `pdf()` device's
//! uncompressed output: single `adj` (vertical adjustment `NA`, i.e. ink
//! centring on the glyph bounding boxes), two-element `adj` (baseline
//! offset by `adj[2] * strheight("M")`), rotation, `cex` and `font`: every
//! text item (see `common::pdf::assert_same`). Skipped if `Rscript` isn't
//! available.

use crate::common;
use crate::common::{pdf, r};

use rs_graphics::prelude::*;

/// `(label, hadj, vadj, srt, cex, font)`; `vadj = None` is a single `adj`.
const CASES: &[(&str, f64, Option<f64>, f64, f64, i32)] = &[
    ("Centre", 0.5, None, 0.0, 1.0, 1),
    ("left gyp", 0.0, None, 0.0, 1.0, 1),
    ("right 12", 1.0, None, 0.0, 1.5, 2),
    ("xxx", 0.5, None, 0.0, 1.0, 1),
    ("rotated", 0.5, None, 90.0, 1.0, 1),
    ("slanted Qj", 0.0, None, 30.0, 0.8, 3),
    ("baseline", 0.0, Some(0.0), 0.0, 1.0, 1),
    ("middle M", 0.5, Some(0.5), 0.0, 1.0, 1),
    ("top", 1.0, Some(1.0), 0.0, 1.2, 2),
    ("two adj srt", 0.3, Some(0.7), 45.0, 1.0, 1),
];

#[test]
fn text_placement_matches_r() {
    if !common::r_available() {
        return;
    }
    let page = (6.0, 4.0);
    let mut script =
        String::from("par(mar = c(0, 0, 0, 0))\nplot.new(); plot.window(c(0, 10), c(0, 11))\n");
    for (i, (label, h, v, srt, cex, font)) in CASES.iter().enumerate() {
        let adj = match v {
            Some(v) => format!("c({h}, {v})"),
            None => h.to_string(),
        };
        script.push_str(&format!(
            "text(5, {y}, '{label}', adj = {adj}, srt = {srt}, cex = {cex}, font = {font})\n",
            y = i + 1
        ));
    }
    let r_pdf = r::run_r("text", page, "", &[("all", &script)])
        .pdfs
        .remove(0);
    let (ours, ()) = r::ours(page, |dev| {
        dev.par().mar([0, 0, 0, 0]).draw();
        dev.plot_new();
        dev.plot_window().xlim([0, 10]).ylim([0, 11]).draw();
        for (i, &(label, h, v, srt, cex, font)) in CASES.iter().enumerate() {
            let text = dev
                .text()
                .x(5)
                .y((i + 1) as f64)
                .labels(label)
                .adj(h)
                .srt(srt)
                .cex(cex)
                .font(font);
            match v {
                Some(v) => text.vadj(v).draw(),
                None => text.draw(),
            }
        }
    });
    let (ours, r) = (pdf::parse(&ours), pdf::parse(&r_pdf));
    assert_eq!(r.pages[0].texts.len(), CASES.len());
    pdf::assert_same_pdf("text", ours, r, &pdf::Compare::default());
}
