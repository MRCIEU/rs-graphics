// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks `NA` (`None`) in the high-level plots' label lists against
//! a real R `pdf()` device's uncompressed output: `barplot()`'s `names`
//! (no label under that bar) and `legend_text` (the key without a label),
//! `boxplot()`'s `names` (no label under that box) and `pie()`'s `labels`
//! (neither label nor tick line for that slice). Every painted path and
//! text item in order (see `common::pdf::assert_same`). Skipped if
//! `Rscript` isn't available.

use crate::common;
use crate::common::{pdf, r};

use rs_graphics::prelude::*;

const PAGE: (f64, f64) = (7.0, 7.0);

struct Case {
    name: &'static str,
    r: &'static str,
    ours: fn(&mut Device<AnyBackend>),
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "barplot",
            r: "barplot(c(3, 5, 2), names.arg = c('a', NA, 'c'), \
                legend.text = c('x', NA, 'z'))",
            ours: |d| {
                d.barplot()
                    .height([3, 5, 2])
                    .names([Some("a"), None, Some("c")])
                    .legend_text([Some("x"), None, Some("z")])
                    .draw();
            },
        },
        Case {
            name: "boxplot",
            r: "boxplot(list(1:5, 2:6, 3:7), names = c('p', NA, 'r'))",
            ours: |d| {
                d.boxplot()
                    .x([
                        vec![1, 2, 3, 4, 5],
                        vec![2, 3, 4, 5, 6],
                        vec![3, 4, 5, 6, 7],
                    ])
                    .names([Some("p"), None, Some("r")])
                    .draw();
            },
        },
        Case {
            name: "pie",
            r: "pie(c(1, 2, 3), labels = c('a', NA, 'c'))",
            ours: |d| {
                d.pie()
                    .x([1, 2, 3])
                    .labels([Some("a"), None, Some("c")])
                    .draw();
            },
        },
    ]
}

#[test]
fn label_na_matches_r() {
    if !common::r_available() {
        return;
    }
    let cases = cases();
    let r_cases: Vec<_> = cases.iter().map(|c| (c.name, c.r)).collect();
    let r = r::run_r("label_na", PAGE, "", &r_cases);
    for (c, r_pdf) in cases.iter().zip(&r.pdfs) {
        let (ours, ()) = r::ours(PAGE, c.ours);
        pdf::assert_same_pdf(
            c.name,
            pdf::parse(&ours),
            pdf::parse(r_pdf),
            &pdf::Compare::default(),
        );
    }
}
