// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks colour 0 against R, where it is the background colour
//! (`par("bg")`, `RGBpar3`): with `pdf(bg = )`, with the default
//! transparent `pdf()` background (nothing drawn), and after `par(bg = )`.
//! Compares every painted path (the page background included) and text
//! item in order, with their colours, to an uncompressed R `pdf()` (see
//! `common::pdf::assert_same`). Skipped if `Rscript` isn't available.

use crate::common;
use crate::common::{pdf, r};


use rs_graphics::backend::pdf::PdfBackend;
use rs_graphics::color::{Color, ColorSpec};
use rs_graphics::device::Device;
use rs_graphics::lty::LineType;
use rs_graphics::par::Par;
use rs_graphics::prelude::*;

const PAGE_W: f64 = 5.0;
const PAGE_H: f64 = 5.0;

struct Case {
    name: &'static str,
    /// `pdf(bg = )`, if any.
    page_bg: Option<&'static str>,
    /// `par(bg = )` after `plot.new()`, if any.
    par_bg: Option<&'static str>,
}

const CASES: &[Case] = &[
    Case { name: "pdf_bg", page_bg: Some("yellow"), par_bg: None },
    Case { name: "transparent", page_bg: None, par_bg: None },
    Case { name: "par_bg", page_bg: None, par_bg: Some("lightblue") },
];

const DRAW_R: &str = "plot.window(c(0, 1), c(0, 1))\n\
    points(0.5, 0.5, col = 0, pch = 19)\n\
    rect(0.1, 0.1, 0.3, 0.3, col = 0, border = 2)\n\
    text(0.7, 0.7, 'z', col = 0)\n\
    segments(0, 1, 1, 0, col = 0, lwd = 3)";

fn draw_ours(c: &Case) -> Vec<u8> {
    let mut backend = PdfBackend::new(PAGE_W, PAGE_H).with_compression(false);
    if let Some(bg) = c.page_bg {
        backend = backend.with_background(Color::parse(bg).unwrap());
    }
    let mut d = Device::new(backend, Par::default());
    d.plot_new();
    if let Some(bg) = c.par_bg {
        d.par().bg(Color::parse(bg).unwrap()).draw();
    }
    d.plot_window().xlim(0.0, 1.0).ylim(0.0, 1.0).draw();
    let zero = ColorSpec::Index(0);
    d.points()
        .x([0.5])
        .y([0.5])
        .with(&XyOptions {
            pch: vec![Some(19)],
            col: vec![zero],
            ..XyOptions::default()
        })
        .draw();
    let solid = LineType::from_code(1).unwrap();
    d.rect()
        .xleft([0.1])
        .ybottom([0.1])
        .xright([0.3])
        .ytop([0.3])
        .with(&RectOptions {
            col: vec![zero],
            border: vec![ColorSpec::Index(2)],
            lwd: vec![Some(1.0)],
            lty: vec![Some(solid)],
            ..RectOptions::default()
        })
        .draw();
    d.text()
        .x([0.7])
        .y([0.7])
        .labels(["z"])
        .with(&TextOptions {
            adj: Some(0.5),
            col: vec![zero],
            cex: vec![Some(1.0)],
            font: vec![Some(FontFace::Plain)],
            ..TextOptions::default()
        })
        .draw();
    let line = SegmentsOptions {
        col: vec![zero],
        lty: vec![Some(solid)],
        lwd: vec![Some(3.0)],
        ..SegmentsOptions::default()
    };
    d.segments()
        .x0([0.0])
        .y0([1.0])
        .x1([1.0])
        .y1([0.0])
        .with(&line)
        .draw();
    d.into_backend().finish()
}

#[test]
fn colour_zero_matches_r() {
    if !common::r_available() {
        return;
    }
    let setups: Vec<(String, String)> = CASES
        .iter()
        .map(|c| {
            let bg = c
                .page_bg
                .map(|b| format!(", bg = '{b}'"))
                .unwrap_or_default();
            let par_bg = c
                .par_bg
                .map(|b| format!("par(bg = '{b}')\n"))
                .unwrap_or_default();
            (bg, format!("plot.new()\n{par_bg}{DRAW_R}"))
        })
        .collect();
    let r_cases: Vec<_> = CASES
        .iter()
        .zip(&setups)
        .map(|(c, (bg, code))| (c.name, (PAGE_W, PAGE_H), bg.as_str(), code.as_str()))
        .collect();
    let r = r::run_r_args("colour_zero", "", &r_cases);
    // The page background counts here.
    let how = pdf::Compare {
        col: 0.002,
        drop_path: |_, _| false,
        ..Default::default()
    };
    for (c, r_pdf) in CASES.iter().zip(&r.pdfs) {
        let theirs = pdf::parse(r_pdf);
        assert!(
            !theirs.pages[0].paths.is_empty(),
            "{}: nothing from R",
            c.name
        );
        pdf::assert_same_pdf(c.name, pdf::parse(&draw_ours(c)), theirs, &how);
    }
}
