// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks multi-figure output (`mfrow`/`mfcol`, `oma`, `mfg`,
//! `layout()` with spanning figures / `lcm()` / `respect`, and `par(cex)`)
//! against a real R `pdf()` device's uncompressed output: every painted
//! path and text item in order (see `common::pdf::assert_same`). Skipped if
//! `Rscript` isn't available.

use crate::common;
use crate::common::{pdf, r};

use rs_graphics::backend::pdf::PdfBackend;
use rs_graphics::device::Device;
use rs_graphics::highlevel::{BarplotOptions, PlotOptions, PlotType};
use rs_graphics::backend::svg::SvgBackend;
use rs_graphics::layout::{LayoutRespect, LayoutSize};
use rs_graphics::par::{Par, Sides};
use rs_graphics::plotmath::Label;
use rs_graphics::graphics::Graphics;

const PAGE_W: f64 = 8.0;
const PAGE_H: f64 = 6.0;

/// R: `bp(h, main)`.
const R_PRELUDE: &str = "bp <- function(h, main) barplot(h, names.arg = LETTERS[seq_along(h)], main = main, ylab = 'v')\n";

fn bp(d: &mut Device<PdfBackend>, h: &[f64], main: &str) {
    let o = BarplotOptions::default()
        .names(
            (0..h.len())
                .map(|i| Label::from(((b'A' + i as u8) as char).to_string()))
                .collect::<Vec<_>>(),
        )
        .main(main)
        .ylab("v");
    d.barplot().height(h).with(&o).draw();
}

struct Case {
    name: &'static str,
    r: &'static str,
    run: fn(&mut Device<PdfBackend>),
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "mfrow_2x2",
            r: "par(mfrow = c(2, 2))\n\
                bp(c(3, 5, 2), 'one'); bp(c(4, 1, 6, 2), 'two')\n\
                bp(c(2, 2, 7), 'three'); bp(c(5, 3), 'four')",
            run: |d| {
                d.par().mfrow(2, 2).draw();
                bp(d, &[3.0, 5.0, 2.0], "one");
                bp(d, &[4.0, 1.0, 6.0, 2.0], "two");
                bp(d, &[2.0, 2.0, 7.0], "three");
                bp(d, &[5.0, 3.0], "four");
            },
        },
        Case {
            name: "mfrow_3x2",
            r: "par(mfrow = c(3, 2))\n\
                for (k in 1:6) bp(c(k, 7 - k, 3), paste('fig', k))",
            run: |d| {
                d.par().mfrow(3, 2).draw();
                for k in 1..=6 {
                    bp(d, &[k as f64, 7.0 - k as f64, 3.0], &format!("fig {k}"));
                }
            },
        },
        Case {
            name: "mfcol_2x3_partial",
            r: "par(mfcol = c(2, 3))\n\
                bp(c(1, 4, 2), 'a'); bp(c(3, 3, 1), 'b'); bp(c(5, 2, 4), 'c')",
            run: |d| {
                d.par().mfcol(2, 3).draw();
                bp(d, &[1.0, 4.0, 2.0], "a");
                bp(d, &[3.0, 3.0, 1.0], "b");
                bp(d, &[5.0, 2.0, 4.0], "c");
            },
        },
        Case {
            name: "mfrow_2x1_oma",
            r: "par(mfrow = c(2, 1), oma = c(1, 2, 3, 1))\n\
                bp(c(3, 5, 2), 'top'); bp(c(4, 1, 6), 'bottom')",
            run: |d| {
                d.par().oma(Sides::new(1.0, 2.0, 3.0, 1.0)).draw();
                d.par().mfrow(2, 1).draw();
                bp(d, &[3.0, 5.0, 2.0], "top");
                bp(d, &[4.0, 1.0, 6.0], "bottom");
            },
        },
        Case {
            name: "mfg",
            r: "par(mfrow = c(2, 2))\n\
                bp(c(3, 5, 2), 'first'); par(mfg = c(2, 2)); bp(c(4, 1, 6), 'last')",
            run: |d| {
                d.par().mfrow(2, 2).draw();
                bp(d, &[3.0, 5.0, 2.0], "first");
                d.par().mfg(2, 2).draw();
                bp(d, &[4.0, 1.0, 6.0], "last");
            },
        },
        Case {
            name: "layout_span_heights",
            r: "layout(matrix(c(1, 1, 2, 3), 2, byrow = TRUE), heights = c(1, 2))\n\
                bp(c(3, 5, 2), 'wide'); bp(c(4, 1), 'left'); bp(c(2, 6, 1), 'right')",
            run: |d| {
                d.layout()
                    .mat(&[[1, 1], [2, 3]])
                    .heights([LayoutSize::Relative(1.0), LayoutSize::Relative(2.0)])
                    .draw();
                bp(d, &[3.0, 5.0, 2.0], "wide");
                bp(d, &[4.0, 1.0], "left");
                bp(d, &[2.0, 6.0, 1.0], "right");
            },
        },
        Case {
            name: "layout_cm_widths",
            r: "layout(matrix(1:2, 1), widths = c(lcm(4), 1))\n\
                bp(c(3, 5), 'narrow'); bp(c(4, 1, 6), 'wide')",
            run: |d| {
                d.layout()
                    .mat(&[[1, 2]])
                    .widths([LayoutSize::Cm(4.0), LayoutSize::Relative(1.0)])
                    .draw();
                bp(d, &[3.0, 5.0], "narrow");
                bp(d, &[4.0, 1.0, 6.0], "wide");
            },
        },
        Case {
            name: "layout_mixed_cm_heights",
            r: "layout(matrix(c(1, 2, 3, 3), 2, byrow = TRUE), widths = c(1, 2), heights = c(lcm(5), 1))\n\
                bp(c(3, 5), 'a'); bp(c(4, 1, 6), 'b'); bp(c(2, 2, 7), 'c')",
            run: |d| {
                d.layout()
                    .mat(&[[1, 2], [3, 3]])
                    .widths([LayoutSize::Relative(1.0), LayoutSize::Relative(2.0)])
                    .heights([LayoutSize::Cm(5.0), LayoutSize::Relative(1.0)])
                    .draw();
                bp(d, &[3.0, 5.0], "a");
                bp(d, &[4.0, 1.0, 6.0], "b");
                bp(d, &[2.0, 2.0, 7.0], "c");
            },
        },
        Case {
            name: "layout_respect",
            r: "layout(matrix(1:4, 2, byrow = TRUE), respect = TRUE)\n\
                for (k in 1:4) bp(c(k, 2), paste('r', k))",
            run: |d| {
                d.layout().mat(&[[1, 2], [3, 4]]).respect(true).draw();
                for k in 1..=4 {
                    bp(d, &[k as f64, 2.0], &format!("r {k}"));
                }
            },
        },
        Case {
            name: "layout_respect_matrix",
            r: "layout(matrix(1:2, 1), widths = c(1, 2), respect = matrix(c(TRUE, FALSE), 1))\n\
                bp(c(3, 5), 'sq'); bp(c(4, 1, 6), 'free')",
            run: |d| {
                d.layout()
                    .mat(&[[1, 2]])
                    .widths([LayoutSize::Relative(1.0), LayoutSize::Relative(2.0)])
                    .respect(&[[true, false]])
                    .draw();
                bp(d, &[3.0, 5.0], "sq");
                bp(d, &[4.0, 1.0, 6.0], "free");
            },
        },
        Case {
            name: "par_cex_half",
            r: "par(cex = 0.5)\nbp(c(3, 5, 2), 'small')",
            run: |d| {
                d.par().cex(0.5).draw();
                bp(d, &[3.0, 5.0, 2.0], "small");
            },
        },
        Case {
            name: "mfrow_2x2_plot",
            r: "par(mfrow = c(2, 2))\n\
                for (k in 1:4) plot(1:10, (1:10)^2, type = 'n', main = paste('p', k), xlab = 'x', ylab = 'y')",
            run: |d| {
                d.par().mfrow(2, 2).draw();
                let x: Vec<f64> = (1..=10).map(f64::from).collect();
                let y: Vec<f64> = x.iter().map(|v| v * v).collect();
                for k in 1..=4 {
                    let title = format!("p {k}");
                    let o = PlotOptions::default()
                        .type_(PlotType::None)
                        .main(title.as_str())
                        .xlab("x")
                        .ylab("y");
                    d.plot().x(&x).y(&y).with(&o).draw();
                }
            },
        },
    ]
}

#[test]
fn layout_matches_r() {
    if !common::r_available() {
        return;
    }
    let cases = cases();
    let page = (PAGE_W, PAGE_H);
    let r_cases: Vec<_> = cases.iter().map(|c| (c.name, c.r)).collect();
    let r = r::run_r("layout", page, R_PRELUDE, &r_cases);
    for (c, r_pdf) in cases.iter().zip(&r.pdfs) {
        let (ours, ()) = r::ours(page, c.run);
        pdf::assert_same_pdf(
            c.name,
            pdf::parse(&ours),
            pdf::parse(r_pdf),
            &pdf::Compare::default(),
        );
    }
}

fn svg_device() -> Device<SvgBackend> {
    Device::new(SvgBackend::new(8.0, 6.0), Par::default())
}

/// `.draw()` returns the number of figures, as R's `layout()` does
/// (`layout(matrix(c(1, 1, 2, 3), 2, byrow = TRUE))` gives 3; fractions
/// are truncated, so `c(1.7, 2.2)` is `c(1, 2)` and gives 2).
#[test]
fn layout_returns_the_number_of_figures() {
    let mut d = svg_device();
    assert_eq!(d.layout().mat(&[[1, 1], [2, 3]]).draw(), 3);
    assert_eq!(d.layout().mat(&[[1.7, 2.2]]).draw(), 2);
    assert_eq!(d.layout().mat(&[[1, 0], [2, 2]]).draw(), 2);
}

#[test]
#[should_panic(expected = "argument \"mat\" is missing, with no default")]
fn layout_without_mat_panics() {
    svg_device().layout().widths([1, 2]).draw();
}

#[test]
#[should_panic(expected = "layout matrix must contain at least one reference")]
fn layout_skipping_a_figure_panics_like_r() {
    svg_device().layout().mat(&[[1, 3]]).draw();
}

#[test]
#[should_panic(expected = "'respect' must be logical or matrix with same dimension as 'mat'")]
fn layout_respect_of_another_shape_panics_like_r() {
    svg_device()
        .layout()
        .mat(&[[1, 2]])
        .respect(&[[true], [false]])
        .draw();
}

#[test]
#[should_panic(expected = "layout matrix values must be non-negative numbers")]
fn layout_with_a_negative_value_panics() {
    svg_device().layout().mat(&[[-1, 1]]).draw();
}

/// `respect` takes R's two forms: a logical or a grid of cells.
#[test]
fn layout_respect_takes_a_bool_or_cells() {
    assert_eq!(LayoutRespect::from(true), LayoutRespect::All);
    assert_eq!(
        LayoutRespect::from(&[[true, false]]),
        LayoutRespect::Cells(vec![vec![true, false]])
    );
}
