// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks how `boxplot()` draws (as `bxp()` does) against a real R
//! `pdf()` device's uncompressed output: every painted path in drawing
//! order, with its vertices (so line direction, which sets where dashes
//! fall, counts) and style (see `common::pdf::assert_same`). Covers `notch`, `varwidth` and `horizontal`. Axes are off, so only the
//! boxes themselves are compared. Skipped if `Rscript` isn't available.

use crate::common;
use crate::common::{pdf, r};

use rs_graphics::backend::AnyBackend;
use rs_graphics::color::{Color, ColorSpec};
use rs_graphics::device::Device;
use rs_graphics::graphics::Graphics;
use rs_graphics::highlevel::BoxplotOptions;

const PAGE_W: f64 = 6.0;
const PAGE_H: f64 = 4.0;

/// Three groups; the first and last have outliers on both sides.
fn groups() -> Vec<Vec<f64>> {
    vec![
        vec![-6.0, 1.0, 2.0, 2.5, 3.0, 3.5, 4.0, 5.0, 14.0],
        vec![2.0, 3.0, 3.0, 4.0, 6.0, 7.5],
        vec![0.5, 4.0, 4.2, 4.4, 5.0, 5.1, 5.6, 6.0, 11.0],
    ]
}

struct Case {
    name: &'static str,
    /// R colour names for `col` and `border`.
    col: &'static str,
    border: &'static str,
    lwd: f64,
    notch: bool,
    varwidth: bool,
    horizontal: bool,
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "defaults",
            col: "lightgray",
            border: "black",
            lwd: 1.0,
            notch: false,
            varwidth: false,
            horizontal: false,
        },
        Case {
            name: "coloured_lwd2",
            col: "lightblue",
            border: "darkred",
            lwd: 2.0,
            notch: false,
            varwidth: false,
            horizontal: false,
        },
        Case {
            name: "notch",
            col: "lightgray",
            border: "black",
            lwd: 1.0,
            notch: true,
            varwidth: false,
            horizontal: false,
        },
        Case {
            name: "varwidth",
            col: "lightgray",
            border: "black",
            lwd: 1.0,
            notch: false,
            varwidth: true,
            horizontal: false,
        },
        Case {
            name: "horizontal",
            col: "lightgray",
            border: "black",
            lwd: 1.0,
            notch: false,
            varwidth: false,
            horizontal: true,
        },
        Case {
            name: "all_three",
            col: "lightblue",
            border: "darkred",
            lwd: 2.0,
            notch: true,
            varwidth: true,
            horizontal: true,
        },
    ]
}

fn r_code(c: &Case) -> String {
    let groups: Vec<String> = groups()
        .iter()
        .map(|g| {
            let v: Vec<String> = g.iter().map(|x| x.to_string()).collect();
            format!("c({})", v.join(", "))
        })
        .collect();
    format!(
        "par(lwd = {lwd})\n\
         suppressWarnings(boxplot(list({groups}), col = '{col}', border = '{border}', \
                 axes = FALSE, notch = {notch}, varwidth = {varwidth}, horizontal = {horizontal}))",
        lwd = c.lwd,
        groups = groups.join(", "),
        col = c.col,
        border = c.border,
        notch = if c.notch { "TRUE" } else { "FALSE" },
        varwidth = if c.varwidth { "TRUE" } else { "FALSE" },
        horizontal = if c.horizontal { "TRUE" } else { "FALSE" },
    )
}

fn draw_ours(c: &Case, dev: &mut Device<AnyBackend>) {
    dev.par().lwd(c.lwd).draw();
    let opts = BoxplotOptions::default()
        .col(vec![ColorSpec::Color(Color::try_from(c.col).unwrap())])
        .border(vec![ColorSpec::Color(Color::try_from(c.border).unwrap())])
        .axes(false)
        .notch(c.notch)
        .varwidth(c.varwidth)
        .horizontal(c.horizontal);
    dev.boxplot().x(groups()).with(&opts).draw();
}

// -------------------------------------------------------------------
// A minimal PDF content-stream interpreter (see `pch_matches_r.rs`), keeping
// paths in drawing order with their vertices and dash pattern.
// -------------------------------------------------------------------

#[test]
fn boxplot_drawing_matches_r() {
    if !common::r_available() {
        return;
    }
    let cases = cases();
    let page = (PAGE_W, PAGE_H);
    let codes: Vec<String> = cases.iter().map(r_code).collect();
    let r_cases: Vec<_> = cases
        .iter()
        .zip(&codes)
        .map(|(c, r)| (c.name, r.as_str()))
        .collect();
    let r = r::run_r("bxp", page, "", &r_cases);
    let how = pdf::Compare::default();
    for (c, r_pdf) in cases.iter().zip(&r.pdfs) {
        let (ours_pdf, ()) = r::ours(page, |dev| draw_ours(c, dev));
        pdf::assert_same_pdf(c.name, pdf::parse(&ours_pdf), pdf::parse(r_pdf), &how);
    }
}
