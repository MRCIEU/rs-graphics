// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks `barplot()`'s `offset` (vector, `beside` and stacked; the
//! stacked case with `ylim` given, since R's default limits there use a
//! mis-recycled `offset`), `orderH` (with tied heights), `add = TRUE`
//! (titles off by default, and `ann = TRUE`) and `args.legend` (keyword
//! position with `bty`/`inset`/`title`; `(x, y)` with `ncol`/`xjust`/`cex`)
//! against a real R `pdf()` device's uncompressed output. Compares every
//! painted path and text item in order (see `common::pdf::assert_same`),
//! except the origins of labels with a minus sign (R's `pdf()` draws `-`
//! as a wider `/minus`; see CLAUDE.md). Skipped if `Rscript` isn't
//! available.

use crate::common;
use crate::common::{pdf, r};

use rs_graphics::prelude::*;

const PAGE_W: f64 = 7.0;
const PAGE_H: f64 = 5.0;

/// A 3 x 4 matrix as its columns; the second column has tied rows.
const M: [[f64; 3]; 4] = [
    [1.0, 4.0, 2.0],
    [3.0, 1.0, 3.0],
    [2.0, 2.0, 5.0],
    [4.0, 3.0, 1.0],
];

fn mat() -> Vec<Vec<f64>> {
    M.iter().map(|c| c.to_vec()).collect()
}

fn r_matrix() -> String {
    let cols: Vec<String> = M
        .iter()
        .map(|c| {
            let v: Vec<String> = c.iter().map(|x| x.to_string()).collect();
            format!("c({})", v.join(", "))
        })
        .collect();
    format!("cbind({})", cols.join(", "))
}

const ROWS: [&str; 3] = ["r1", "r2", "r3"];

type Draw = fn(&mut Device<AnyBackend>);

struct Case {
    name: &'static str,
    /// R code; `m` is the matrix above.
    r: &'static str,
    ours: Draw,
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "offset_vector",
            r: "barplot(c(3, 1, 2, 4), offset = c(1, 5, 2), names.arg = c('a', 'b', 'c', 'd'))",
            ours: |d| {
                d.barplot()
                    .height([3.0, 1.0, 2.0, 4.0])
                    .offset([1.0, 5.0, 2.0])
                    .names(["a", "b", "c", "d"])
                    .draw();
            },
        },
        Case {
            name: "offset_beside",
            r: "barplot(m, beside = TRUE, offset = c(1, 2, 0), legend.text = c('r1', 'r2', 'r3'))",
            ours: |d| {
                d.barplot()
                    .height(mat())
                    .beside(true)
                    .offset([1.0, 2.0, 0.0])
                    .legend_text(ROWS)
                    .draw();
            },
        },
        Case {
            name: "offset_stacked",
            r: "barplot(m, offset = c(0, 10, 0, 5), ylim = c(0, 25))",
            ours: |d| {
                d.barplot()
                    .height(mat())
                    .offset([0.0, 10.0, 0.0, 5.0])
                    .ylim(0.0, 25.0)
                    .draw();
            },
        },
        Case {
            name: "order_decr",
            r: "barplot(m, orderH = 'decr', col = c('red', 'green', 'blue'), \
                legend.text = c('r1', 'r2', 'r3'))",
            ours: |d| {
                d.barplot()
                    .height(mat())
                    .order_h(OrderH::Decr)
                    .col(["red", "green", "blue"])
                    .legend_text(ROWS)
                    .draw();
            },
        },
        Case {
            name: "order_incr_horiz",
            r: "barplot(m, orderH = 'incr', horiz = TRUE, col = c('red', 'green', 'blue'))",
            ours: |d| {
                d.barplot()
                    .height(mat())
                    .order_h(OrderH::Incr)
                    .horiz(true)
                    .col(["red", "green", "blue"])
                    .draw();
            },
        },
        Case {
            name: "add",
            r: "plot(c(0, 6), c(0, 12), type = 'n', ann = FALSE)\n\
                barplot(c(3, 8, 5), add = TRUE, space = 1, col = 'white', main = 'hidden', \
                names.arg = c('x', 'y', 'z'))",
            ours: |d| {
                d.plot()
                    .x([0.0, 6.0])
                    .y([0.0, 12.0])
                    .type_(PlotType::None)
                    .draw();
                d.barplot()
                    .height([3.0, 8.0, 5.0])
                    .add(true)
                    .space([1.0])
                    .col("white")
                    .main("hidden")
                    .names(["x", "y", "z"])
                    .draw();
            },
        },
        Case {
            name: "add_ann",
            r: "plot(c(0, 6), c(0, 12), type = 'n', ann = FALSE)\n\
                barplot(c(3, 8, 5), add = TRUE, ann = TRUE, axes = FALSE, main = 'title', \
                xlab = 'x label')",
            ours: |d| {
                d.plot()
                    .x([0.0, 6.0])
                    .y([0.0, 12.0])
                    .type_(PlotType::None)
                    .draw();
                d.barplot()
                    .height([3.0, 8.0, 5.0])
                    .add(true)
                    .ann(true)
                    .axes(false)
                    .main("title")
                    .xlab("x label")
                    .draw();
            },
        },
        Case {
            name: "args_legend_keyword",
            r: "barplot(m, legend.text = c('r1', 'r2', 'r3'), \
                args.legend = list(x = 'topleft', bty = 'n', inset = 0.05, title = 'Groups'))",
            ours: |d| {
                d.barplot()
                    .height(mat())
                    .legend_text(ROWS)
                    .legend_pos("topleft")
                    .args_legend(
                        LegendOptions::default()
                            .bty(Bty::N)
                            .inset((0.05, 0.05))
                            .title("Groups"),
                    )
                    .draw();
            },
        },
        Case {
            name: "args_legend_xy",
            r: "barplot(m, beside = TRUE, legend.text = c('r1', 'r2', 'r3'), ylim = c(0, 8), \
                args.legend = list(x = 2, y = 7.5, ncol = 3, xjust = 0, cex = 0.8))",
            ours: |d| {
                d.barplot()
                    .height(mat())
                    .beside(true)
                    .legend_text(ROWS)
                    .ylim(0.0, 8.0)
                    .legend_pos((2.0, 7.5))
                    .args_legend(LegendOptions::default().ncol(3usize).xjust(0.0).cex(0.8))
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
fn barplot_extras_match_r() {
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
    let r = r::run_r(
        "bar_extras",
        page,
        &format!("m <- {}", r_matrix()),
        &r_cases,
    );
    let how = pdf::Compare {
        skip_origin: |t| t.string.contains('-'),
        ..Default::default()
    };
    for (c, r_pdf) in cases.iter().zip(&r.pdfs) {
        let (ours_pdf, ()) = r::ours(page, c.ours);
        pdf::assert_same_pdf(c.name, pdf::parse(&ours_pdf), pdf::parse(r_pdf), &how);
    }
}
