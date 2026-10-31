// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks `plot(type = "b")` and `plot(type = "c")` against a real R
//! `pdf()` device's uncompressed output: the gapped line segments between
//! points (each shortened by half a character height at both ends, short
//! ones left out, broken at `NA`), with `par(cex)` and a log axis. Compares
//! every painted path in order, the points too (none for `"c"`; see
//! `common::pdf::assert_same`). Skipped if `Rscript` isn't available.

use crate::common;
use crate::common::{pdf, r};

use rs_graphics::highlevel::{PlotOptions, PlotType};
use rs_graphics::par::ParOptions;
use rs_graphics::graphics::Graphics;

const PAGE_W: f64 = 6.0;
const PAGE_H: f64 = 5.0;

const X: [f64; 9] = [1.0, 2.0, 2.1, 4.0, 5.0, f64::NAN, 7.0, 8.0, 10.0];
const Y: [f64; 9] = [1.0, 3.0, 3.1, 2.0, 8.0, 5.0, 6.0, 30.0, 12.0];

struct Case {
    name: &'static str,
    /// `"b"` or `"c"` (the same lines, no points).
    type_: &'static str,
    cex: f64,
    ylog: bool,
}

const CASES: [Case; 3] = [
    Case {
        name: "plain",
        type_: "b",
        cex: 1.0,
        ylog: false,
    },
    Case {
        name: "cex_log",
        type_: "b",
        cex: 1.5,
        ylog: true,
    },
    Case {
        name: "type_c",
        type_: "c",
        cex: 1.0,
        ylog: false,
    },
];

fn r_code(c: &Case) -> String {
    let v = |a: &[f64]| {
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
    };
    format!(
        "par(cex = {cex})\n\
         plot({x}, {y}, type = '{t}', log = '{log}', axes = FALSE, ann = FALSE)",
        cex = c.cex,
        t = c.type_,
        x = v(&X),
        y = v(&Y),
        log = if c.ylog { "y" } else { "" },
    )
}

fn draw_ours(c: &Case) -> Vec<u8> {
    let par = ParOptions::default().cex(c.cex);
    let opts = PlotOptions::default()
        .type_(PlotType::try_from(c.type_).unwrap())
        .ylog(c.ylog)
        .axes(false);
    r::ours_with((PAGE_W, PAGE_H), par, |dev| {
        dev.plot().x(X).y(Y).with(&opts).draw();
    })
    .0
}

#[test]
fn type_b_matches_r() {
    if !common::r_available() {
        return;
    }
    let codes: Vec<String> = CASES.iter().map(r_code).collect();
    let r_cases: Vec<_> = CASES
        .iter()
        .zip(&codes)
        .map(|(c, r)| (c.name, r.as_str()))
        .collect();
    let r = r::run_r("type_b", (PAGE_W, PAGE_H), "", &r_cases);
    for (c, r_pdf) in CASES.iter().zip(&r.pdfs) {
        let theirs = pdf::parse(r_pdf);
        let points = theirs.pages[0].paths.iter().filter(|p| p.curved).count();
        assert_eq!(
            points == 0,
            c.type_ == "c",
            "{}: points only for type b",
            c.name
        );
        pdf::assert_same_pdf(
            c.name,
            pdf::parse(&draw_ours(c)),
            theirs,
            &pdf::Compare::default(),
        );
    }
}
