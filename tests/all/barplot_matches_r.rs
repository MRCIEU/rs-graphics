// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks `barplot()` (vector, stacked / `beside` matrix, `horiz`,
//! `width`/`space`, `legend.text`) against a real R `pdf()` device's
//! uncompressed output: every painted path and text item in order (see
//! `common::pdf::assert_same`), except the origins of labels with a minus
//! sign (R's `/minus`). Skipped if `Rscript` isn't available.

use crate::common;
use crate::common::{pdf, r};

use rs_graphics::backend::AnyBackend;
use rs_graphics::color::{Color, ColorSpec};
use rs_graphics::device::Device;
use rs_graphics::highlevel::BarplotOptions;
use rs_graphics::plotmath::Label;
use rs_graphics::graphics::Graphics;

fn names(n: &[&str]) -> Vec<Label> {
    n.iter().map(|&s| s.into()).collect()
}

fn red_green_blue() -> Vec<ColorSpec> {
    ["red", "green", "blue"]
        .iter()
        .map(|c| ColorSpec::Color(Color::try_from(*c).unwrap()))
        .collect()
}

/// The 3 x 4 matrix used by the matrix scenarios, as columns.
fn mat() -> Vec<Vec<f64>> {
    vec![
        vec![3.0, 5.0, 2.0],
        vec![4.0, 1.0, 6.0],
        vec![2.0, 2.0, 7.0],
        vec![5.0, 3.0, 1.0],
    ]
}

const R_MAT: &str = "m <- matrix(c(3,5,2, 4,1,6, 2,2,7, 5,3,1), nrow = 3)";

struct Case {
    name: &'static str,
    r: &'static str,
    run: fn(&mut Device<AnyBackend>),
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "vector",
            r: r#"barplot(c(23, 41, 12, -5), names.arg = c("A","B","C","D"), main = "Vec", ylab = "v")"#,
            run: |d| {
                let o = BarplotOptions::default()
                    .names(names(&["A", "B", "C", "D"]))
                    .main("Vec")
                    .ylab("v");
                d.barplot().height([23.0, 41.0, 12.0, -5.0]).with(&o).draw();
            },
        },
        Case {
            name: "stacked_legend",
            r: r#"barplot(m, names.arg = c("a","b","c","d"), legend.text = c("r1","r2","r3"))"#,
            run: |d| {
                let o = BarplotOptions::default()
                    .names(names(&["a", "b", "c", "d"]))
                    .legend_text(names(&["r1", "r2", "r3"]));
                d.barplot().height(mat()).with(&o).draw();
            },
        },
        Case {
            name: "beside_col",
            r: r#"barplot(m, beside = TRUE, names.arg = c("a","b","c","d"), col = c("red","green","blue"))"#,
            run: |d| {
                let o = BarplotOptions::default()
                    .names(names(&["a", "b", "c", "d"]))
                    .beside(true)
                    .col(red_green_blue());
                d.barplot().height(mat()).with(&o).draw();
            },
        },
        Case {
            name: "horiz_stacked",
            r: r#"barplot(m, horiz = TRUE, names.arg = c("a","b","c","d"))"#,
            run: |d| {
                let o = BarplotOptions::default()
                    .names(names(&["a", "b", "c", "d"]))
                    .horiz(true);
                d.barplot().height(mat()).with(&o).draw();
            },
        },
        Case {
            name: "horiz_beside_space_width_legend",
            r: r#"barplot(m, horiz = TRUE, beside = TRUE, space = c(0.1, 0.5), width = c(1, 2, 1), legend.text = c("r1","r2","r3"))"#,
            run: |d| {
                let o = BarplotOptions::default()
                    .horiz(true)
                    .beside(true)
                    .space(vec![0.1, 0.5])
                    .width(vec![1.0, 2.0, 1.0])
                    .legend_text(names(&["r1", "r2", "r3"]));
                d.barplot().height(mat()).with(&o).draw();
            },
        },
        Case {
            name: "stacked_negative_ylim",
            r: r#"barplot(m - 3, ylim = c(-8, 12))"#,
            run: |d| {
                let cols: Vec<Vec<f64>> = mat()
                    .iter()
                    .map(|c| c.iter().map(|v| v - 3.0).collect())
                    .collect();
                let o = BarplotOptions::default().ylim([-8.0, 12.0]);
                d.barplot().height(&cols).with(&o).draw();
            },
        },
    ]
}

#[test]
fn barplot_matches_r() {
    if !common::r_available() {
        return;
    }
    let cases = cases();
    let page = (6.0, 4.0);
    let r_cases: Vec<_> = cases.iter().map(|c| (c.name, c.r)).collect();
    let r = r::run_r("barplot", page, R_MAT, &r_cases);
    // R's pdf device draws `-` as the wider `minus` glyph (see `devPS.c`,
    // `/Differences [ 45/minus ]`), which we deliberately don't replicate.
    let how = pdf::Compare {
        skip_origin: |t| t.string.contains('-'),
        ..Default::default()
    };
    for (c, r_pdf) in cases.iter().zip(&r.pdfs) {
        let (ours, ()) = r::ours(page, c.run);
        pdf::assert_same_pdf(c.name, pdf::parse(&ours), pdf::parse(r_pdf), &how);
    }
}
