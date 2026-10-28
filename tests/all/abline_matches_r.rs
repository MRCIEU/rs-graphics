// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks `abline()` with vectors of `h`/`v` and recycled `col`/
//! `lty`/`lwd` and `a`/`b` (several lines, and R's `coef =`), against a
//! real R `pdf()` device's uncompressed output: every painted path in
//! drawing order (see `common::pdf::assert_same`), on linear and log axes,
//! and dashes with square and butt line ends. Skipped if `Rscript` isn't
//! available.

use crate::common;
use crate::common::{pdf, r};

use rs_graphics::backend::LineEnd;
use rs_graphics::backend::pdf::PdfBackend;
use rs_graphics::color::{Color, ColorSpec};
use rs_graphics::device::{AblineOptions, Device};
use rs_graphics::lty::LineType;
use rs_graphics::par::Par;
use rs_graphics::prelude::*;

const PAGE_W: f64 = 6.0;
const PAGE_H: f64 = 5.0;

fn col(names: &[&str]) -> Vec<ColorSpec> {
    names
        .iter()
        .map(|n| ColorSpec::Color(Color::parse(n).unwrap()))
        .collect()
}

struct Case {
    name: &'static str,
    /// R code after `plot.new()`/`plot.window()`.
    r: &'static str,
    log: &'static str,
    lims: ((f64, f64), (f64, f64)),
    draw: fn(&mut Device<PdfBackend>),
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "vectors",
            r: "abline(h = c(2, 5, 8), v = c(3, 7), col = c('red', 'blue'), \
                       lty = 1:3, lwd = c(1, 2))",
            log: "",
            lims: ((0.0, 10.0), (0.0, 10.0)),
            draw: |d| {
                d.abline()
                    .with(
                        &AblineOptions::default()
                            .h(vec![2.0, 5.0, 8.0])
                            .v(vec![3.0, 7.0])
                            .col(col(&["red", "blue"]))
                            .lty(
                                (1..=3)
                                    .map(|c| Some(LineType::from_code(c).unwrap()))
                                    .collect::<Vec<_>>(),
                            )
                            .lwd(vec![Some(1.0), Some(2.0)]),
                    )
                    .draw();
            },
        },
        Case {
            name: "a_b_first",
            r: "abline(a = 1, b = 0.5, h = c(4, 6), v = 9, \
                       col = c('darkgreen', 'orange', 'purple'), lwd = 3)",
            log: "",
            lims: ((0.0, 10.0), (0.0, 10.0)),
            draw: |d| {
                d.abline()
                    .with(
                        &AblineOptions::default()
                            .a(vec![1.0])
                            .b(vec![0.5])
                            .h(vec![4.0, 6.0])
                            .v(vec![9.0])
                            .col(col(&["darkgreen", "orange", "purple"]))
                            .lwd(vec![Some(3.0)]),
                    )
                    .draw();
            },
        },
        Case {
            // R's `coef =` (here a fitted line) is `a`/`b`.
            name: "fitted_line",
            r: "x <- 1:10; y <- c(2.1, 2.9, 4.2, 4.8, 6.3, 6.9, 8.1, 9.2, 9.8, 11.4)\n\
                abline(coef = coef(lm(y ~ x)), col = 'red')\n\
                abline(coef = c(2, -0.3), lty = 2)",
            log: "",
            lims: ((0.0, 10.0), (0.0, 12.0)),
            draw: |d| {
                // Least squares, as `lm(y ~ x)`.
                let x: Vec<f64> = (1..=10).map(f64::from).collect();
                let y = [2.1, 2.9, 4.2, 4.8, 6.3, 6.9, 8.1, 9.2, 9.8, 11.4];
                let (mx, my) = (x.iter().sum::<f64>() / 10.0, y.iter().sum::<f64>() / 10.0);
                let sxy: f64 = x.iter().zip(&y).map(|(a, b)| (a - mx) * (b - my)).sum();
                let sxx: f64 = x.iter().map(|a| (a - mx) * (a - mx)).sum();
                let slope = sxy / sxx;
                let red = col(&["red"])[0];
                let fit = AblineOptions::default()
                    .a(vec![my - slope * mx])
                    .b(vec![slope])
                    .col(vec![red]);
                d.abline().with(&fit).draw();
                d.abline().a(2).b(-0.3).lty("dashed").draw();
            },
        },
        Case {
            name: "line_ends",
            r: "par(lend = 'square'); abline(h = c(2, 4), lty = c(2, 3), lwd = c(1, 3))\n\
                par(lend = 'butt'); abline(h = c(6, 8), lty = c('dotdash', '42'), lwd = 2)",
            log: "",
            lims: ((0.0, 10.0), (0.0, 10.0)),
            draw: |d| {
                d.par().lend(LineEnd::Square).draw();
                d.abline()
                    .with(
                        &AblineOptions::default()
                            .h(vec![2.0, 4.0])
                            .lty(vec![
                                Some(LineType::from_code(2).unwrap()),
                                Some(LineType::from_code(3).unwrap()),
                            ])
                            .lwd(vec![Some(1.0), Some(3.0)]),
                    )
                    .draw();
                d.par().lend(LineEnd::Butt).draw();
                d.abline()
                    .with(
                        &AblineOptions::default()
                            .h(vec![6.0, 8.0])
                            .lty(vec![
                                Some(LineType::from_code(4).unwrap()),
                                Some(LineType::parse("42").unwrap()),
                            ])
                            .lwd(vec![Some(2.0)]),
                    )
                    .draw();
            },
        },
        Case {
            name: "a_and_b",
            r: "abline(a = 1, b = 0.5, col = 'red')\n\
                abline(1, 0.8, h = 3)",
            log: "",
            lims: ((0.0, 10.0), (0.0, 10.0)),
            draw: |d| {
                d.abline().a(1).b(0.5).col("red").draw();
                d.abline().b(0.8).a(1).h(3).draw();
            },
        },
        Case {
            // Several `a`/`b` lines (not in R: drawn there as one call per
            // line), styles recycled over them and then `h`; a single `b`
            // pairs with each `a`.
            name: "several_a_b",
            r: "abline(a = 1, b = 0.2, lty = 1, col = 'red')\n\
                abline(a = 2, b = 0.3, lty = 2, col = 'blue')\n\
                abline(h = 3, lty = 3, col = 'red')\n\
                abline(h = 5, lty = 4, col = 'blue')\n\
                abline(a = 6, b = -0.5, lwd = 2)\n\
                abline(a = 8, b = -0.5, lwd = 2)",
            log: "",
            lims: ((0.0, 10.0), (0.0, 10.0)),
            draw: |d| {
                d.abline()
                    .a([1, 2])
                    .b([0.2, 0.3])
                    .h(&[3.0, 5.0][..])
                    .lty([1, 2, 3, 4])
                    .col(["red", "blue"])
                    .draw();
                d.abline().a([6, 8]).b(-0.5).lwd(2).draw();
            },
        },
        Case {
            name: "par_defaults_log",
            r: "par(col = 'grey40', lty = 2, lwd = 2)\n\
                abline(h = c(1, 10, 100), v = c(2, 20))\n\
                abline(a = 1, b = 1, untf = TRUE, h = 50, col = c('red', 'blue'))",
            log: "xy",
            lims: ((1.0, 100.0), (0.5, 500.0)),
            draw: |d| {
                d.par().col(Color::parse("grey40").unwrap()).draw();
                d.par().lty(LineType::from_code(2).unwrap()).draw();
                d.par().lwd(2.0).draw();
                d.abline()
                    .with(
                        &AblineOptions::default()
                            .h(vec![1.0, 10.0, 100.0])
                            .v(vec![2.0, 20.0]),
                    )
                    .draw();
                d.abline()
                    .with(
                        &AblineOptions::default()
                            .a(vec![1.0])
                            .b(vec![1.0])
                            .untf(true)
                            .h(vec![50.0])
                            .col(col(&["red", "blue"])),
                    )
                    .draw();
            },
        },
    ]
}

#[test]
fn abline_matches_r() {
    if !common::r_available() {
        return;
    }
    let cases = cases();
    let page = (PAGE_W, PAGE_H);
    let codes: Vec<String> = cases
        .iter()
        .map(|c| {
            let ((x0, x1), (y0, y1)) = c.lims;
            format!(
                "plot.new(); plot.window(c({x0}, {x1}), c({y0}, {y1}), log = '{}')\n{}",
                c.log, c.r
            )
        })
        .collect();
    let r_cases: Vec<_> = cases
        .iter()
        .zip(&codes)
        .map(|(c, r)| (c.name, r.as_str()))
        .collect();
    let r = r::run_r("abline", page, "", &r_cases);
    // R writes widths to 2 decimals.
    let how = pdf::Compare {
        lwd: 0.0051,
        ..Default::default()
    };
    for (c, r_pdf) in cases.iter().zip(&r.pdfs) {
        let (ours_pdf, ()) = r::ours(page, |dev| {
            dev.plot_new();
            let (xl, yl) = c.lims;
            dev.plot_window()
                .xlim(xl.0, xl.1)
                .ylim(yl.0, yl.1)
                .xlog(c.log.contains('x'))
                .ylog(c.log.contains('y'))
                .draw();
            (c.draw)(dev);
        });
        pdf::assert_same_pdf(c.name, pdf::parse(&ours_pdf), pdf::parse(r_pdf), &how);
    }
}

fn window() -> Device<PdfBackend> {
    let mut dev = Device::new(PdfBackend::new(PAGE_W, PAGE_H), Par::default());
    dev.plot_new();
    dev.plot_window().xlim(0.0, 1.0).ylim(0.0, 1.0).draw();
    dev
}

#[test]
#[should_panic(expected = "invalid a=, b= specification")]
fn abline_a_needs_b() {
    window().abline().a(1).draw();
}

/// R draws nothing for a lone `b` (`C_abline` only looks at `b` when `a`
/// is given); we treat it as the same mistake as a lone `a`.
#[test]
#[should_panic(expected = "invalid a=, b= specification")]
fn abline_b_needs_a() {
    window().abline().b(2).h(0.5).draw();
}

#[test]
fn abline_a_and_b_record_in_a_figure() {
    let mut fig = Figure::new();
    fig.plot_new();
    fig.abline().a(0.2).b(0.5).draw();
    let (mut a, mut b) = (window(), window());
    a.add_figure(&fig);
    b.plot_new();
    b.abline().a([0.2]).b([0.5]).draw();
    assert!(a.finish() == b.finish());
}

#[test]
#[should_panic(expected = "'a' and 'b' must be finite")]
fn abline_ab_must_be_finite() {
    window().abline().a(f64::NAN).b(1).draw();
}

#[test]
#[should_panic(expected = "'a' and 'b' must be finite")]
fn abline_every_a_b_must_be_finite() {
    window().abline().a([1.0, f64::NAN]).b(1).draw();
}

#[test]
#[should_panic(expected = "'a' and 'b' lengths differ")]
fn abline_a_b_lengths_must_match() {
    window().abline().a([1, 2, 3]).b([1, 2]).draw();
}
