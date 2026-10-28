// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks `pch` given as strings outside `legend()` against a real R
//! `pdf()` device's uncompressed output: each string's first character is
//! its symbol (`"bc"` draws `b`), `""` and `NA` draw none, non-ASCII
//! characters are drawn as text, in `points()`, `plot()` and `boxplot()`'s
//! `outpch`/`medpch`. Every painted path and text item in order (see
//! `common::pdf::assert_same`). Skipped if `Rscript` isn't available.

use crate::common;
use crate::common::{pdf, r};

use rs_graphics::prelude::*;

const PAGE: (f64, f64) = (7.0, 7.0);

struct Case {
    name: &'static str,
    r: &'static str,
    ours: fn(&mut Device<PdfBackend>),
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "points",
            r: "plot.new(); plot.window(c(0, 7), c(0, 2))\n\
                points(1:6, rep(1, 6), pch = c('a', 'bc', '\\u00e9', NA, '', '+'), cex = 2)",
            ours: |d| {
                d.plot_new();
                d.plot_window().xlim(0, 7).ylim(0, 2).draw();
                d.points()
                    .x(1..=6)
                    .y([1; 6])
                    .pch([
                        Some("a"),
                        Some("bc"),
                        Some("\u{e9}"),
                        None,
                        Some(""),
                        Some("+"),
                    ])
                    .cex(2)
                    .draw();
            },
        },
        Case {
            name: "plot",
            r: "plot(1:3, pch = 'abc', ylab = 'y')",
            ours: |d| {
                d.plot().x(1..=3).pch("abc").ylab("y").draw();
            },
        },
        Case {
            name: "boxplot",
            r: "boxplot(c(1:9, 30), outpch = 'x', medpch = 'm', medcex = 2)",
            ours: |d| {
                d.boxplot()
                    .x([1, 2, 3, 4, 5, 6, 7, 8, 9, 30])
                    .outpch("x")
                    .medpch("m")
                    .medcex(2)
                    .draw();
            },
        },
    ]
}

#[test]
fn pch_strings_match_r() {
    if !common::r_available() {
        return;
    }
    let cases = cases();
    let r_cases: Vec<_> = cases.iter().map(|c| (c.name, c.r)).collect();
    let r = r::run_r("pch_strings", PAGE, "", &r_cases);
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
