// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks `boxplot()`'s positioning and styling arguments against a
//! real R `pdf()` device's uncompressed output: `at` and `width`,
//! `outline = FALSE`, `bxp()`'s drawing parameters (`boxwex`, `staplewex`,
//! `outwex`, the general `lty`/`lwd` and the `box*`/`med*`/`whisk*`/
//! `staple*`/`out*` styles, recycled over boxes; outlier lines only with
//! one box, as R draws them several times over with more), `add = TRUE`
//! (with `ann`
//! off by default), log value and position axes, and `xlim`/`ylim` with
//! `horizontal`. Compares every painted path and text item in order (see
//! `common::pdf::assert_same`), except the origins of labels with a minus
//! sign (R's `pdf()` draws `-` as a wider `/minus`; see CLAUDE.md).
//! Skipped if `Rscript` isn't available.

use crate::common;
use crate::common::{pdf, r};

use rs_graphics::prelude::*;

const PAGE_W: f64 = 7.0;
const PAGE_H: f64 = 5.0;

/// Three groups; the first and last have outliers on both sides.
const G: [&[f64]; 3] = [
    &[-6.0, 1.0, 2.0, 2.5, 3.0, 3.5, 4.0, 5.0, 14.0],
    &[2.0, 3.0, 3.0, 4.0, 6.0, 7.5],
    &[0.5, 4.0, 4.2, 4.4, 5.0, 5.1, 5.6, 6.0, 11.0],
];
/// Positive groups, for log axes, with outliers.
const P: [&[f64]; 3] = [
    &[0.2, 3.0, 4.0, 5.0, 6.0, 7.0, 9.0, 80.0],
    &[10.0, 12.0, 15.0, 20.0, 30.0, 45.0],
    &[1.0, 2.0, 2.2, 2.5, 3.0, 3.1, 3.6, 40.0],
];

fn groups(g: &[&[f64]; 3]) -> Vec<Vec<f64>> {
    g.iter().map(|v| v.to_vec()).collect()
}

fn r_list(g: &[&[f64]; 3]) -> String {
    let groups: Vec<String> = g
        .iter()
        .map(|v| {
            let v: Vec<String> = v.iter().map(|x| x.to_string()).collect();
            format!("c({})", v.join(", "))
        })
        .collect();
    format!("list({})", groups.join(", "))
}

type Draw = fn(&mut Device<AnyBackend>);

struct Case {
    name: &'static str,
    /// R code; `G` and `P` are the groups above.
    r: &'static str,
    ours: Draw,
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "at_width",
            r: "boxplot(G, at = c(1, 2.5, 6), width = c(1, 2, 0.5), names = c('a', 'b', 'c'))",
            ours: |d| {
                d.boxplot()
                    .x(groups(&G))
                    .at([1.0, 2.5, 6.0])
                    .width([1.0, 2.0, 0.5])
                    .names(["a", "b", "c"])
                    .draw();
            },
        },
        Case {
            name: "outline_false",
            r: "boxplot(G, outline = FALSE, axes = FALSE)",
            ours: |d| {
                d.boxplot().x(groups(&G)).outline(false).axes(false).draw();
            },
        },
        Case {
            name: "pars",
            r: "boxplot(G, boxwex = 0.4, staplewex = 1, lwd = 2, lty = 1, \
                medcol = 'red', medlwd = 5, medpch = 19, medcex = 1.5, \
                whiskcol = 'blue', staplelty = 3, boxlty = 2, \
                boxcol = c('darkgreen', 'orange'), outpch = 21, outbg = 'yellow', \
                outcex = 2, outcol = 'purple', \
                col = c('pink', 'lightblue'), axes = FALSE)",
            ours: |d| {
                d.boxplot()
                    .x(groups(&G))
                    .boxwex(0.4)
                    .staplewex(1.0)
                    .lwd(2.0)
                    .lty(LineType::SOLID)
                    .medcol("red")
                    .medlwd(5.0)
                    .medpch(19)
                    .medcex(1.5)
                    .whiskcol("blue")
                    .staplelty(LineType::DOTTED)
                    .boxlty(LineType::DASHED)
                    .boxcol(["darkgreen", "orange"])
                    .outpch(21)
                    .outbg("yellow")
                    .outcex(2.0)
                    .outcol("purple")
                    .col(["pink", "lightblue"])
                    .axes(false)
                    .draw();
            },
        },
        Case {
            // One box: with several, R draws each outlier line once per box
            // (`rep(x - wid * outwex, nout)` uses the whole `outwex` vector),
            // all on top of each other; we draw it once.
            name: "outlier_lines",
            r: "boxplot(G[1], outlty = 1, outwex = 1.5, outcol = 'purple', outlwd = 2, \
                axes = FALSE)",
            ours: |d| {
                d.boxplot()
                    .x(&groups(&G)[..1])
                    .outlty(LineType::SOLID)
                    .outwex(1.5)
                    .outcol("purple")
                    .outlwd(2.0)
                    .axes(false)
                    .draw();
            },
        },
        Case {
            name: "add",
            r: "plot(c(0, 10), c(0, 16), type = 'n', ann = FALSE)\n\
                boxplot(G, add = TRUE, at = c(2, 5, 8), col = 'white', main = 'hidden')",
            ours: |d| {
                d.plot()
                    .x([0.0, 10.0])
                    .y([0.0, 16.0])
                    .type_(PlotType::None)
                    .draw();
                d.boxplot()
                    .x(groups(&G))
                    .add(true)
                    .at([2.0, 5.0, 8.0])
                    .col("white")
                    .main("hidden")
                    .draw();
            },
        },
        Case {
            name: "log_values",
            r: "boxplot(P, log = 'y', main = 'log')",
            ours: |d| {
                d.boxplot().x(groups(&P)).ylog(true).main("log").draw();
            },
        },
        Case {
            name: "log_positions_horizontal",
            r: "boxplot(P, horizontal = TRUE, log = 'y', at = c(1, 3, 10))",
            ours: |d| {
                d.boxplot()
                    .x(groups(&P))
                    .horizontal(true)
                    .ylog(true)
                    .at([1.0, 3.0, 10.0])
                    .draw();
            },
        },
        Case {
            name: "limits_horizontal",
            r: "boxplot(P, horizontal = TRUE, ylim = c(0, 100), xlim = c(0, 5), notch = TRUE)",
            ours: |d| {
                d.boxplot()
                    .x(groups(&P))
                    .horizontal(true)
                    .ylim([0.0, 100.0])
                    .xlim([0.0, 5.0])
                    .notch(true)
                    .draw();
            },
        },
    ]
}

// -------------------------------------------------------------------
// A minimal PDF content-stream interpreter (as in
// `boxplot_draw_matches_r.rs`), plus text origins.
// -------------------------------------------------------------------

#[test]
fn boxplot_extras_match_r() {
    if !common::r_available() {
        return;
    }
    let cases = cases();
    let page = (PAGE_W, PAGE_H);
    let codes: Vec<String> = cases
        .iter()
        .map(|c| format!("suppressWarnings({{\n{}\n}})", c.r))
        .collect();
    let r_cases: Vec<_> = cases
        .iter()
        .zip(&codes)
        .map(|(c, r)| (c.name, r.as_str()))
        .collect();
    let prelude = format!("G <- {}\nP <- {}", r_list(&G), r_list(&P));
    let r = r::run_r("bxp_extras", page, &prelude, &r_cases);
    let how = pdf::Compare {
        skip_origin: |t| t.string.contains('-'),
        ..Default::default()
    };
    for (c, r_pdf) in cases.iter().zip(&r.pdfs) {
        let (ours_pdf, ()) = r::ours(page, c.ours);
        pdf::assert_same_pdf(c.name, pdf::parse(&ours_pdf), pdf::parse(r_pdf), &how);
    }
}
