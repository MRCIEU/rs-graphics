// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks sizes R takes from the device's starting point size
//! (`pdf(pointsize = )`, R's `cra`) rather than the current `par(ps)`:
//! margin lines, plotting symbols, `type = "b"` gaps and `legend()`
//! spacing, after `par(ps)` is changed. Compares every painted path and
//! text item in order with an uncompressed R `pdf()` (see
//! `common::pdf::assert_same`). Skipped if `Rscript` isn't available.

use crate::common;
use crate::common::{pdf, r};

use rs_graphics::backend::pdf::PdfBackend;
use rs_graphics::color::ColorSpec;
use rs_graphics::device::{Device, LegendKeyword, LegendPos};
use rs_graphics::highlevel::{PlotOptions, PlotType};
use rs_graphics::lty::LineType;
use rs_graphics::par::Par;
use rs_graphics::graphics::Graphics;

const PAGE_W: f64 = 7.0;
const PAGE_H: f64 = 6.0;

/// `(device pointsize, par(ps) set afterwards)`.
const CASES: &[(f64, f64)] = &[(12.0, 20.0), (9.0, 14.0), (14.0, 8.0)];

fn r_code(ps: f64) -> String {
    format!(
        "par(ps = {ps})\n\
         plot(c(1, 2, 3, 4), c(1, 3, 2, 4), type = 'b', xlab = 'x', ylab = 'y')\n\
         legend('topleft', legend = c('a', 'bb'), pch = 1, lty = 1)"
    )
}

fn draw_ours(start: f64, ps: f64) -> Vec<u8> {
    let par = Par {
        ps: start,
        ..Par::default()
    };
    let mut dev = Device::new(PdfBackend::new(PAGE_W, PAGE_H).with_compression(false), par);
    dev.par().ps(ps).draw();
    let opts = PlotOptions {
        type_: Some(PlotType::Both),
        xlab: Some("x".into()),
        ylab: Some("y".into()),
        ..PlotOptions::default()
    };
    dev.plot()
        .x([1.0, 2.0, 3.0, 4.0])
        .y([1.0, 3.0, 2.0, 4.0])
        .with(&opts)
        .draw();
    let black = ColorSpec::Index(1);
    dev.legend()
        .pos(LegendPos::Keyword(LegendKeyword::TopLeft))
        .labels(["a", "bb"])
        .col(black)
        .pch(1)
        .lty(LineType::from_code(1).unwrap())
        .draw();
    dev.into_backend().finish()
}

#[test]
fn start_ps_sizes_match_r() {
    if !common::r_available() {
        return;
    }
    let names: Vec<String> = (0..CASES.len()).map(|i| i.to_string()).collect();
    let codes: Vec<(String, String)> = CASES
        .iter()
        .map(|&(start, ps)| (format!(", pointsize = {start}"), r_code(ps)))
        .collect();
    let r_cases: Vec<_> = names
        .iter()
        .zip(&codes)
        .map(|(n, (args, code))| (n.as_str(), (PAGE_W, PAGE_H), args.as_str(), code.as_str()))
        .collect();
    let r = r::run_r_args("start_ps", "", &r_cases);
    for (&(start, ps), r_pdf) in CASES.iter().zip(&r.pdfs) {
        let name = format!("pointsize {start}, ps {ps}");
        pdf::assert_same_pdf(
            &name,
            pdf::parse(&draw_ours(start, ps)),
            pdf::parse(r_pdf),
            &pdf::Compare::default(),
        );
    }
}
