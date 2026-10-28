// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks shading lines (`density`/`angle`) in `polygon()`,
//! `rect()`, `barplot()` and `legend()` against a real R `pdf()` device's
//! uncompressed output: every painted path and text item in order (see
//! `common::pdf::assert_same`). Covers both of `polygon.fullhatch()`'s
//! branches and their boundaries, winding inside a self-intersecting
//! polygon (and `fillOddEven`), `NA`/0/negative densities, reversed axes and a log axis
//! (no shading, as R).
//!
//! An all-`NA` `density` is no shading. R checks this with
//! `is.numeric(density)`, so a logical `NA` (what `density = NA` gives)
//! shades all the same, putting a border in the fill colour; the R code
//! uses `NA_real_`, which R treats as intended.
//!
//! Not compared: R's `rect()` with a vector `lwd` and shading (R recycles
//! it over each rectangle's shading lines instead of per rectangle), and
//! `barplot()` legends with fewer `density` values than labels (R
//! reverses them before recycling). With `orderH`, R keeps `density` and
//! `angle` in segment order while the colours follow the rows; ours
//! follow the rows too (see `barplot_order_h`). Skipped if `Rscript` isn't available.

use crate::common;
use crate::common::{pdf, r};

use rs_graphics::prelude::*;

const PAGE_W: f64 = 7.0;
const PAGE_H: f64 = 5.0;

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
    let cols: Vec<String> = M.iter().map(|c| r::r_vec(c)).collect();
    format!("cbind({})", cols.join(", "))
}

const NAN: f64 = f64::NAN;

struct Case {
    name: &'static str,
    /// R code; `m` is the matrix above.
    r: &'static str,
    ours: fn(&mut Device<PdfBackend>),
}

macro_rules! window {
    () => {
        "plot.new(); plot.window(c(0, 10), c(0, 10))\n"
    };
}

fn window(d: &mut Device<PdfBackend>, xlim: (f64, f64), ylim: (f64, f64)) {
    d.plot_new();
    d.plot_window()
        .xlim(xlim.0, xlim.1)
        .ylim(ylim.0, ylim.1)
        .draw();
}

/// Five polygons (NA-separated): a triangle, a square, a thin slanted
/// quadrilateral, a pentagon and a triangle.
const PX: [f64; 23] = [
    0.5, 3.0, 1.5, NAN, 4.0, 6.0, 6.0, 4.0, NAN, 7.0, 9.5, 8.5, 6.5, NAN, 1.0, 3.0, 3.5, 2.0, 0.5,
    NAN, 5.0, 9.0, 6.0,
];
const PY: [f64; 23] = [
    6.0, 6.5, 9.5, NAN, 6.0, 6.0, 9.0, 9.0, NAN, 6.0, 7.0, 9.5, 8.0, NAN, 0.5, 0.5, 2.5, 4.5, 2.5,
    NAN, 1.0, 1.5, 4.5,
];

fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "angles",
            r: concat!(
                window!(),
                "px <- c(0.5, 3, 1.5, NA, 4, 6, 6, 4, NA, 7, 9.5, 8.5, 6.5, NA, 1, 3, 3.5, 2, 0.5, NA, 5, 9, 6)\n",
                "py <- c(6, 6.5, 9.5, NA, 6, 6, 9, 9, NA, 6, 7, 9.5, 8, NA, 0.5, 0.5, 2.5, 4.5, 2.5, NA, 1, 1.5, 4.5)\n",
                "polygon(px, py, density = 8, angle = c(0, 30, 45, 100, 160), col = c('red', NA, 'blue'), lty = c(1, 2), lwd = 1.5)\n",
                "polygon(px, py - 0.3, density = c(5, 12), angle = c(90, 135, -30, 44.9, 135.1), border = 'grey')"
            ),
            ours: |d| {
                window(d, (0.0, 10.0), (0.0, 10.0));
                d.polygon()
                    .x(PX)
                    .y(PY)
                    .density(8)
                    .angle([0, 30, 45, 100, 160])
                    .col([Some("red"), None, Some("blue")])
                    .lty([1, 2])
                    .lwd(1.5)
                    .draw();
                d.polygon()
                    .x(PX)
                    .y(PY.map(|v| v - 0.3))
                    .density([5, 12])
                    .angle([90.0, 135.0, -30.0, 44.9, 135.1])
                    .border("grey")
                    .draw();
            },
        },
        Case {
            // A pentagram: its centre has winding number 2, so it is shaded.
            name: "star",
            r: concat!(
                window!(),
                "a <- pi / 2 + (0:4) * 4 * pi / 5\n",
                "polygon(5 + 4 * cos(a), 5 + 4 * sin(a), density = 12, angle = 20, border = 'darkgreen')"
            ),
            ours: |d| {
                window(d, (0.0, 10.0), (0.0, 10.0));
                let a: Vec<f64> = (0..5)
                    .map(|k| {
                        std::f64::consts::PI / 2.0 + k as f64 * 4.0 * std::f64::consts::PI / 5.0
                    })
                    .collect();
                let x: Vec<f64> = a.iter().map(|a| 5.0 + 4.0 * a.cos()).collect();
                let y: Vec<f64> = a.iter().map(|a| 5.0 + 4.0 * a.sin()).collect();
                d.polygon()
                    .x(&x)
                    .y(&y)
                    .density(12)
                    .angle(20)
                    .border("darkgreen")
                    .draw();
            },
        },
        Case {
            // With `fillOddEven`, the pentagram's centre (encircled twice)
            // isn't shaded.
            name: "star_odd_even",
            r: concat!(
                window!(),
                "a <- pi / 2 + (0:4) * 4 * pi / 5\n",
                "polygon(5 + 4 * cos(a), 5 + 4 * sin(a), density = 12, angle = 20, fillOddEven = TRUE)"
            ),
            ours: |d| {
                window(d, (0.0, 10.0), (0.0, 10.0));
                let a: Vec<f64> = (0..5)
                    .map(|k| {
                        std::f64::consts::PI / 2.0 + k as f64 * 4.0 * std::f64::consts::PI / 5.0
                    })
                    .collect();
                let x: Vec<f64> = a.iter().map(|a| 5.0 + 4.0 * a.cos()).collect();
                let y: Vec<f64> = a.iter().map(|a| 5.0 + 4.0 * a.sin()).collect();
                d.polygon()
                    .x(&x)
                    .y(&y)
                    .density(12)
                    .angle(20)
                    .fill_odd_even(true)
                    .draw();
            },
        },
        Case {
            // `NA` and negative densities fill, 0 draws only the border.
            name: "density_mix",
            r: concat!(
                window!(),
                "polygon(c(1, 3, 3, 1, NA, 4, 6, 6, 4, NA, 7, 9, 9, 7, NA, 1, 3, 3, 1),\n",
                "        c(1, 1, 3, 3, NA, 1, 1, 3, 3, NA, 1, 1, 3, 3, NA, 5, 5, 8, 8),\n",
                "        density = c(NA, 0, 15, -1), col = c('pink', 'green', 'blue', 'orange'), border = c('black', 'red'))"
            ),
            ours: |d| {
                window(d, (0.0, 10.0), (0.0, 10.0));
                d.polygon()
                    .x([
                        1.0, 3.0, 3.0, 1.0, NAN, 4.0, 6.0, 6.0, 4.0, NAN, 7.0, 9.0, 9.0, 7.0, NAN,
                        1.0, 3.0, 3.0, 1.0,
                    ])
                    .y([
                        1.0, 1.0, 3.0, 3.0, NAN, 1.0, 1.0, 3.0, 3.0, NAN, 1.0, 1.0, 3.0, 3.0, NAN,
                        5.0, 5.0, 8.0, 8.0,
                    ])
                    .density([None, Some(0.0), Some(15.0), Some(-1.0)])
                    .col(["pink", "green", "blue", "orange"])
                    .border(["black", "red"])
                    .draw();
            },
        },
        Case {
            // The border defaults to the shading colour; an all-`NA`
            // density is an ordinary rectangle.
            name: "rect",
            r: concat!(
                window!(),
                "rect(c(1, 6, 7), 1, c(3, 4, 9), c(4, 5, 3), density = c(5, 10, 20), angle = c(45, -30, 90),\n",
                "     col = c(NA, 'red'), lty = c(1, 2, 3), lwd = 2)\n",
                "rect(1, 6, 3, 8, density = NA_real_, col = 'grey')\n",
                "rect(4, 6, 6, 8, density = 0, col = 'purple')\n",
                "rect(7, 6, 9, 8, density = 7, border = 'black')"
            ),
            ours: |d| {
                window(d, (0.0, 10.0), (0.0, 10.0));
                d.rect()
                    .xleft([1, 6, 7])
                    .ybottom([1])
                    .xright([3, 4, 9])
                    .ytop([4, 5, 3])
                    .density([5, 10, 20])
                    .angle([45, -30, 90])
                    .col([None, Some("red")])
                    .lty([1, 2, 3])
                    .lwd(2)
                    .draw();
                d.rect()
                    .xleft([1])
                    .ybottom([6])
                    .xright([3])
                    .ytop([8])
                    .density([None::<f64>])
                    .col("grey")
                    .draw();
                d.rect()
                    .xleft([4])
                    .ybottom([6])
                    .xright([6])
                    .ytop([8])
                    .density(0)
                    .col("purple")
                    .draw();
                d.rect()
                    .xleft([7])
                    .ybottom([6])
                    .xright([9])
                    .ytop([8])
                    .density(7)
                    .border("black")
                    .draw();
            },
        },
        Case {
            name: "reversed_x",
            r: concat!(
                "plot.new(); plot.window(c(10, 0), c(0, 10))\n",
                "polygon(c(1, 6, 4), c(1, 2, 8), density = 10, angle = 30)\n",
                "rect(6, 5, 9, 9, density = 6, angle = 120, col = 'blue')"
            ),
            ours: |d| {
                window(d, (10.0, 0.0), (0.0, 10.0));
                d.polygon()
                    .x([1, 6, 4])
                    .y([1, 2, 8])
                    .density(10)
                    .angle(30)
                    .draw();
                d.rect()
                    .xleft([6])
                    .ybottom([5])
                    .xright([9])
                    .ytop([9])
                    .density(6)
                    .angle(120)
                    .col("blue")
                    .draw();
            },
        },
        Case {
            name: "reversed_y",
            r: concat!(
                "plot.new(); plot.window(c(0, 5), c(10, 0))\n",
                "polygon(c(0.5, 4, 3, 1), c(1, 2, 9, 7), density = 9, angle = 60, col = 'red')"
            ),
            ours: |d| {
                window(d, (0.0, 5.0), (10.0, 0.0));
                d.polygon()
                    .x([0.5, 4.0, 3.0, 1.0])
                    .y([1, 2, 9, 7])
                    .density(9)
                    .angle(60)
                    .col("red")
                    .draw();
            },
        },
        Case {
            // R doesn't shade on a log axis (it warns); fills and borders
            // are drawn.
            name: "log",
            r: concat!(
                "plot.new(); plot.window(c(1, 100), c(0, 10), log = 'x')\n",
                "suppressWarnings(polygon(c(2, 50, 50, 2, NA, 2, 50, 50, 2), c(1, 1, 4, 4, NA, 6, 6, 9, 9),\n",
                "        density = c(10, NA), col = c('red', 'blue')))"
            ),
            ours: |d| {
                d.plot_new();
                d.plot_window().xlim(1, 100).ylim(0, 10).xlog(true).draw();
                d.polygon()
                    .x([2.0, 50.0, 50.0, 2.0, NAN, 2.0, 50.0, 50.0, 2.0])
                    .y([1.0, 1.0, 4.0, 4.0, NAN, 6.0, 6.0, 9.0, 9.0])
                    .density([Some(10), None])
                    .col(["red", "blue"])
                    .draw();
            },
        },
        Case {
            name: "barplot_beside",
            r: "barplot(m, beside = TRUE, density = c(10, 20, NA), angle = c(45, 135, 0), \
                col = c('red', 'blue', 'green'), legend.text = c('r1', 'r2', 'r3'))",
            ours: |d| {
                d.barplot()
                    .height(mat())
                    .beside(true)
                    .density([Some(10), Some(20), None])
                    .angle([45, 135, 0])
                    .col(["red", "blue", "green"])
                    .legend_text(["r1", "r2", "r3"])
                    .draw();
            },
        },
        Case {
            name: "barplot_stacked",
            r: "barplot(m, density = c(8, 16, 24), angle = 60, legend.text = c('r1', 'r2', 'r3'))",
            ours: |d| {
                d.barplot()
                    .height(mat())
                    .density([8, 16, 24])
                    .angle(60)
                    .legend_text(["r1", "r2", "r3"])
                    .draw();
            },
        },
        Case {
            // With `orderH`, a segment's shading follows its row, as its
            // colour does. R keeps `density` in segment order instead; all
            // columns here sort the same way (rows 2, 3, 1), so R gets the
            // rows' densities in that order.
            name: "barplot_order_h",
            r: "barplot(cbind(c(1, 3, 2), c(2, 5, 4), c(1, 4, 3)), order = 'decr', \
                density = c(16, 24, 8), col = c('red', 'green', 'blue'), angle = c(30, 60, 120))",
            ours: |d| {
                d.barplot()
                    .height(vec![
                        vec![1.0, 3.0, 2.0],
                        vec![2.0, 5.0, 4.0],
                        vec![1.0, 4.0, 3.0],
                    ])
                    .order_h("decr")
                    .density([8, 16, 24])
                    .col(["red", "green", "blue"])
                    .angle([120, 30, 60])
                    .draw();
            },
        },
        Case {
            name: "barplot_horiz",
            r: "barplot(c(3, 1, 2, 4), horiz = TRUE, density = c(10, NA), angle = c(30, 0), col = c('red', 'blue'))",
            ours: |d| {
                d.barplot()
                    .height([3, 1, 2, 4])
                    .horiz(true)
                    .density([Some(10), None])
                    .angle([30, 0])
                    .col(["red", "blue"])
                    .draw();
            },
        },
        Case {
            // `density` alone draws boxes, shaded in `par(fg)`.
            name: "legend",
            r: concat!(
                window!(),
                "legend('topleft', legend = c('a', 'b', 'c'), density = c(10, NA, 20), angle = c(45, 0, 135),\n",
                "       fill = c('red', 'blue', 'green'))\n",
                "legend('bottomright', legend = c('d', 'e'), density = 15, cex = 2)"
            ),
            ours: |d| {
                window(d, (0.0, 10.0), (0.0, 10.0));
                d.legend()
                    .pos("topleft")
                    .labels(["a", "b", "c"])
                    .density([Some(10), None, Some(20)])
                    .angle([45, 0, 135])
                    .fill(["red", "blue", "green"])
                    .draw();
                d.legend()
                    .pos("bottomright")
                    .labels(["d", "e"])
                    .density(15)
                    .cex(2)
                    .draw();
            },
        },
    ]
}

#[test]
fn hatching_matches_r() {
    if !common::r_available() {
        return;
    }
    let cases = cases();
    let page = (PAGE_W, PAGE_H);
    let r_cases: Vec<_> = cases.iter().map(|c| (c.name, c.r)).collect();
    let prelude = format!("m <- {}", r_matrix());
    let r = r::run_r("hatch", page, &prelude, &r_cases);
    for (c, r_pdf) in cases.iter().zip(&r.pdfs) {
        let (ours, ()) = r::ours(page, c.ours);
        let ours = pdf::parse(&ours);
        let theirs = pdf::parse(r_pdf);
        pdf::assert_same_pdf(c.name, ours, theirs, &pdf::Compare::default());
    }
}
