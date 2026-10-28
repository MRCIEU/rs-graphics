// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks recorded [`Figure`]s with legends, drawn into multi-panel
//! pages with `add_figure`/`add_figure_at`, against the same R code on an
//! uncompressed R `pdf()`: `mfrow` with `add_figure_at` on a fresh device
//! (R's `plot.new(); par(mfg = )`), a figure's own `par()` not leaking into
//! the next, a shared-key panel in a `layout()`, and one figure added three
//! times to a 3x2 page (panels advance; `cex` 0.66). Compares every painted
//! path and text item in order (see `common::pdf::assert_same`), and each
//! `FigureResult::legends` entry with R's `legend()` return value.
//! Skipped if `Rscript` isn't available.

use crate::common;
use crate::common::{pdf, r};

use rs_graphics::prelude::*;

const PAGE_W: f64 = 8.0;
const PAGE_H: f64 = 6.0;

const DOSE: [f64; 6] = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
const RESPONSE: [f64; 6] = [1.8, 3.1, 3.9, 5.2, 5.8, 7.1];
const FITTED: [f64; 6] = [2.0, 3.0, 4.0, 5.0, 6.0, 7.0];

/// R definitions shared by every scenario. `key(...)` draws the legend
/// and prints its return value as `label left top w h text.x... text.y...`.
const R_PRELUDE: &str = r#"
dose <- 1:6
response <- c(1.8, 3.1, 3.9, 5.2, 5.8, 7.1)
fitted <- c(2, 3, 4, 5, 6, 7)
key <- function(label, ...) {
    l <- legend(..., legend = c("observed", "fitted"), pch = c(1, NA),
                lty = c(NA, 1), col = c("black", "red"))
    cat(label, unlist(l$rect[c("left", "top", "w", "h")]), l$text$x, l$text$y, "\n")
}
fit <- function(label, ...) {
    plot(dose, response, xlab = "x", ylab = "y", main = "Fit")
    lines(dose, fitted, col = "red")
    key(label, ...)
}
"#;

const LABELS: [&str; 2] = ["observed", "fitted"];

/// R's `key()` arguments: a point, then a line.
fn keys() -> LegendOptions {
    LegendOptions::default()
        .pch([Some(1), None])
        .lty([None, Some(LineType::SOLID)])
        .col(["black", "red"])
}

/// R's `fit()`: points, the fitted line and a legend at `pos`.
fn fit_figure(pos: LegendPos, opts: &LegendOptions) -> Figure {
    let mut fig = Figure::new();
    fig.plot()
        .x(DOSE)
        .y(RESPONSE)
        .xlab("x")
        .ylab("y")
        .main("Fit")
        .draw();
    fig.lines().x(DOSE).y(FITTED).col("red").draw();
    fig.legend().pos(pos).labels(LABELS).with(opts).draw();
    fig
}

/// A legend's layout and the `usr` of the panel it was drawn in.
type Drawn = (LegendLayout, (f64, f64, f64, f64));

struct Case {
    name: &'static str,
    /// R code after `R_PRELUDE`; each `key()` prints `name.k` with k from
    /// 0 in drawing order.
    r: &'static str,
    /// Draws the page; returns each legend's layout and the `usr` of the
    /// panel it was drawn in, in drawing order.
    draw: fn(&mut Device<PdfBackend>) -> Vec<Drawn>,
}

fn layouts(r: FigureResult, usr: (f64, f64, f64, f64)) -> Vec<Drawn> {
    r.legends.into_iter().map(|l| (l, usr)).collect()
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "mfrow_at",
            r: r#"par(mfrow = c(2, 2))
plot.new(); par(mfg = c(1, 2))
fit("mfrow_at.0", "bottomright", title = "Key")
par(mfg = c(2, 1)); op <- par(las = 1)
fit("mfrow_at.1", 1.5, 6.8)
par(op)
par(mfg = c(2, 2)); plot(dose, fitted, type = "l", main = "After")"#,
            draw: |d| {
                d.par().mfrow(2, 2).draw();
                let a = fit_figure(
                    LegendPos::Keyword(LegendKeyword::BottomRight),
                    &keys().title("Key"),
                );
                let mut out = layouts(d.add_figure_at(1, 2, &a), d.usr());
                // las = 1 is recorded in the figure, so it must not leak
                // into the next panel.
                let mut b = Figure::new();
                b.par().las(1).draw();
                b.plot()
                    .x(DOSE)
                    .y(RESPONSE)
                    .xlab("x")
                    .ylab("y")
                    .main("Fit")
                    .draw();
                b.lines().x(DOSE).y(FITTED).col("red").draw();
                b.legend()
                    .pos(LegendPos::Xy(1.5, 6.8))
                    .labels(LABELS)
                    .with(&keys())
                    .draw();
                out.extend(layouts(d.add_figure_at(2, 1, &b), d.usr()));
                d.par().mfg(2, 2).draw();
                d.plot()
                    .x(DOSE)
                    .y(FITTED)
                    .type_(PlotType::Lines)
                    .xlab("dose")
                    .ylab("fitted")
                    .main("After")
                    .draw();
                out
            },
        },
        Case {
            name: "layout_key",
            r: r#"layout(matrix(1:3, 1), widths = c(2, 2, 1))
plot(dose, response, xlab = "x", ylab = "y", main = "A")
plot(dose, fitted, type = "l", xlab = "x", ylab = "y", main = "B", col = "red")
par(mar = c(0, 0, 0, 0)); plot.new()
key("layout_key.0", "center")"#,
            draw: |d| {
                let rel = LayoutSize::Relative;
                d.layout()
                    .mat(&[[1, 2, 3]])
                    .widths([rel(2.0), rel(2.0), rel(1.0)])
                    .draw();
                let mut a = Figure::new();
                a.plot()
                    .x(DOSE)
                    .y(RESPONSE)
                    .xlab("x")
                    .ylab("y")
                    .main("A")
                    .draw();
                let mut b = Figure::new();
                b.plot()
                    .x(DOSE)
                    .y(FITTED)
                    .type_(PlotType::Lines)
                    .xlab("x")
                    .ylab("y")
                    .main("B")
                    .col("red")
                    .draw();
                let mut key = Figure::new();
                key.par().mar([0.0, 0.0, 0.0, 0.0]).draw();
                key.plot_new();
                key.legend()
                    .pos(LegendPos::Keyword(LegendKeyword::Center))
                    .labels(LABELS)
                    .with(&keys())
                    .draw();
                d.add_figure(&a);
                d.add_figure(&b);
                let r = d.add_figure(&key);
                layouts(r, d.usr())
            },
        },
        Case {
            name: "mfrow_3x2",
            r: r#"par(mfrow = c(3, 2))
fit("mfrow_3x2.0", "topleft", inset = 0.05)
fit("mfrow_3x2.1", "topleft", inset = 0.05)
fit("mfrow_3x2.2", "topleft", inset = 0.05)"#,
            draw: |d| {
                d.par().mfrow(3, 2).draw();
                let fig = fit_figure(
                    LegendPos::Keyword(LegendKeyword::TopLeft),
                    &keys().inset((0.05, 0.05)),
                );
                let mut out = Vec::new();
                for _ in 0..3 {
                    out.extend(layouts(d.add_figure(&fig), d.usr()));
                }
                out
            },
        },
    ]
}

#[test]
fn figure_legends_in_panels_match_r() {
    if !common::r_available() {
        return;
    }
    let cases = cases();
    let page = (PAGE_W, PAGE_H);
    let r_cases: Vec<_> = cases.iter().map(|c| (c.name, c.r)).collect();
    let run = r::run_r("figure_legends", page, R_PRELUDE, &r_cases);
    let r_stdout = run.stdout;

    for (c, r_pdf_bytes) in cases.iter().zip(&run.pdfs) {
        let (ours, legends) = r::ours(page, c.draw);
        let theirs = pdf::parse(r_pdf_bytes);
        let texts: usize = theirs.pages.iter().map(|p| p.texts.len()).sum();
        assert!(texts > 20, "{}: R drew only {texts} text items", c.name);
        pdf::assert_same_pdf(c.name, pdf::parse(&ours), theirs, &pdf::Compare::default());

        // Each legend's return value, as R's legend() returns it.
        for (k, (layout, usr)) in legends.iter().enumerate() {
            let label = format!("{}.{k}", c.name);
            let line = r_stdout
                .lines()
                .find(|l| l.split_whitespace().next() == Some(label.as_str()))
                .unwrap_or_else(|| panic!("{label}: no return value printed by R"));
            let r: Vec<f64> = line
                .split_whitespace()
                .skip(1)
                .map(|v| v.parse().unwrap())
                .collect();
            let (tx, ty) = (
                (usr.1 - usr.0).abs() / 5000.0,
                (usr.3 - usr.2).abs() / 5000.0,
            );
            let mut flat = vec![
                (layout.left, tx),
                (layout.top, ty),
                (layout.w, tx),
                (layout.h, ty),
            ];
            flat.extend(layout.text_x.iter().map(|&v| (v, tx)));
            flat.extend(layout.text_y.iter().map(|&v| (v, ty)));
            assert_eq!(r.len(), flat.len(), "{label}: number of returned values");
            for (i, (&(a, tol), &b)) in flat.iter().zip(&r).enumerate() {
                assert!(
                    (a - b).abs() < tol,
                    "{label}: returned value {i} differs: ours={a} r={b}"
                );
            }
        }
    }
}
