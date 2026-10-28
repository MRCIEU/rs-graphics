// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks `abline()`, `segments()`, `hist()` (incl. `add = TRUE`)
//! and `barplot()` on log axes, `par(lab)`, `plot(xaxt = "n")` and a horizontal
//! `boxplot()` with axes, against a real R `pdf()` device's uncompressed
//! output: every painted path and text item in drawing order (see
//! `common::pdf::assert_same`), except the origins of labels with a minus
//! sign (R's `/minus`). Skipped if `Rscript` isn't available.
//!
//! `hist()` gets `ylab = "n"`: R's `pdf()` never applies the last kerning
//! pair of each first character (`devPS.c`'s `KPend` is inclusive but
//! looped over exclusively), e.g. "F r" in "Frequency", and we deliberately
//! don't copy that.

use crate::common;
use crate::common::{pdf, r};

use rs_graphics::backend::pdf::PdfBackend;
use rs_graphics::color::{Color, ColorSpec};
use rs_graphics::device::Device;
use rs_graphics::highlevel::{BarplotOptions, BoxplotOptions, HistOptions, PlotOptions};
use rs_graphics::lty::LineType;
use rs_graphics::par::AxisType;
use rs_graphics::prelude::*;

const PAGE_W: f64 = 7.0;
const PAGE_H: f64 = 6.0;

type Dev = Device<PdfBackend>;

/// Data shared with R (`hx` there).
fn hist_data() -> Vec<f64> {
    (0..60)
        .map(|i| 100.0 + 10.0 * ((i as f64) * 0.37).sin() + 0.1 * i as f64)
        .collect()
}

struct Case {
    name: &'static str,
    r: &'static str,
    draw: fn(&mut Dev),
}

fn solid() -> LineType {
    LineType::Pattern(&[])
}

fn red() -> ColorSpec {
    ColorSpec::Color(Color::parse("red").unwrap())
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "segments_log",
            r: "plot.new(); plot.window(c(1, 100), c(1, 100), log = 'xy')\n\
                segments(c(1, 0, 10, -5, 2, NA, 3), c(1, 10, 50, 5, 3, 5, 2), \
                         c(10, 50, 100, 20, 0, 9, 80), c(5, 20, 0.5, 30, 30, 9, 0), col = 'red')",
            draw: |d| {
                d.plot_new();
                d.plot_window()
                    .xlim(1.0, 100.0)
                    .ylim(1.0, 100.0)
                    .with(&PlotWindowOptions {
                        xlog: true,
                        ylog: true,
                        ..PlotWindowOptions::default()
                    })
                    .draw();
                let nan = f64::NAN;
                d.segments()
                    .x0([1.0, 0.0, 10.0, -5.0, 2.0, nan, 3.0])
                    .y0([1.0, 10.0, 50.0, 5.0, 3.0, 5.0, 2.0])
                    .x1([10.0, 50.0, 100.0, 20.0, 0.0, 9.0, 80.0])
                    .y1([5.0, 20.0, 0.5, 30.0, 30.0, 9.0, 0.0])
                    .with(&SegmentsOptions {
                        col: vec![red()],
                        lty: vec![Some(solid())],
                        lwd: vec![Some(1.0)],
                        ..SegmentsOptions::default()
                    })
                    .draw();
            },
        },
        Case {
            name: "abline_loglog",
            r: "plot(c(1, 1000), c(1, 1e4), log = 'xy', xlab = 'x', ylab = 'y')\n\
                abline(h = 10); abline(v = 100, col = 'red'); abline(0, 1)\n\
                abline(a = 5, b = 2, untf = TRUE, lty = 2)\n\
                abline(h = -1); abline(a = -50, b = 3, untf = TRUE, col = 'red')",
            draw: |d| {
                let opts = PlotOptions {
                    xlog: true,
                    ylog: true,
                    xlab: Some("x".into()),
                    ylab: Some("y".into()),
                    ..PlotOptions::default()
                };
                d.plot().x([1.0, 1000.0]).y([1.0, 1e4]).with(&opts).draw();
                let black = ColorSpec::Index(1);
                d.abline()
                    .with(&AblineOptions {
                        h: vec![10.0],
                        col: vec![black],
                        lwd: vec![Some(1.0)],
                        lty: vec![Some(solid())],
                        ..AblineOptions::default()
                    })
                    .draw();
                d.abline()
                    .with(&AblineOptions {
                        v: vec![100.0],
                        col: vec![red()],
                        lwd: vec![Some(1.0)],
                        lty: vec![Some(solid())],
                        ..AblineOptions::default()
                    })
                    .draw();
                d.abline()
                    .with(&AblineOptions {
                        a: vec![0.0],
                        b: vec![1.0],
                        col: vec![black],
                        lwd: vec![Some(1.0)],
                        lty: vec![Some(solid())],
                        ..AblineOptions::default()
                    })
                    .draw();
                d.abline()
                    .with(&AblineOptions {
                        a: vec![5.0],
                        b: vec![2.0],
                        untf: true,
                        col: vec![black],
                        lwd: vec![Some(1.0)],
                        lty: vec![Some(LineType::from_code(2).unwrap())],
                        ..AblineOptions::default()
                    })
                    .draw();
                d.abline()
                    .with(&AblineOptions {
                        h: vec![-1.0],
                        col: vec![black],
                        lwd: vec![Some(1.0)],
                        lty: vec![Some(solid())],
                        ..AblineOptions::default()
                    })
                    .draw();
                d.abline()
                    .with(&AblineOptions {
                        a: vec![-50.0],
                        b: vec![3.0],
                        untf: true,
                        col: vec![red()],
                        lwd: vec![Some(1.0)],
                        lty: vec![Some(solid())],
                        ..AblineOptions::default()
                    })
                    .draw();
            },
        },
        Case {
            name: "abline_logx_xpd",
            r: "plot(c(1, 1000), c(0, 10), log = 'x', xlab = 'x', ylab = 'y')\n\
                abline(1, 2); abline(a = 1, b = 0.01, untf = TRUE)\n\
                par(xpd = TRUE); abline(h = 5, v = 10, col = 'red')",
            draw: |d| {
                let opts = PlotOptions {
                    xlog: true,
                    xlab: Some("x".into()),
                    ylab: Some("y".into()),
                    ..PlotOptions::default()
                };
                d.plot().x([1.0, 1000.0]).y([0.0, 10.0]).with(&opts).draw();
                let black = ColorSpec::Index(1);
                d.abline()
                    .with(&AblineOptions {
                        a: vec![1.0],
                        b: vec![2.0],
                        col: vec![black],
                        lwd: vec![Some(1.0)],
                        lty: vec![Some(solid())],
                        ..AblineOptions::default()
                    })
                    .draw();
                d.abline()
                    .with(&AblineOptions {
                        a: vec![1.0],
                        b: vec![0.01],
                        untf: true,
                        col: vec![black],
                        lwd: vec![Some(1.0)],
                        lty: vec![Some(solid())],
                        ..AblineOptions::default()
                    })
                    .draw();
                d.par().xpd(Xpd::Figure).draw();
                d.abline()
                    .with(&AblineOptions {
                        h: vec![5.0],
                        v: vec![10.0],
                        col: vec![red()],
                        lwd: vec![Some(1.0)],
                        lty: vec![Some(solid())],
                        ..AblineOptions::default()
                    })
                    .draw();
            },
        },
        Case {
            name: "hist_logx",
            r: "hist(hx, log = 'x', ylab = 'n')",
            draw: |d| {
                let o = HistOptions { xlog: true, main: Some("Histogram of hx".into()), xlab: Some("hx".into()), ylab: Some("n".into()), ..Default::default() };
                d.hist().x(hist_data()).with(&o).draw();
            },
        },
        Case {
            name: "hist_logy",
            r: "suppressWarnings(hist(hx, log = 'y', ylab = 'n'))",
            draw: |d| {
                let o = HistOptions { ylog: true, main: Some("Histogram of hx".into()), xlab: Some("hx".into()), ylab: Some("n".into()), ..Default::default() };
                d.hist().x(hist_data()).with(&o).draw();
            },
        },
        Case {
            name: "hist_add",
            r: "hist(hx, log = 'x', ylab = 'n')\n\
                hist(hx * 1.05, add = TRUE, col = 'pink', border = 'red')",
            draw: |d| {
                let o = HistOptions { xlog: true, main: Some("Histogram of hx".into()), xlab: Some("hx".into()), ylab: Some("n".into()), ..Default::default() };
                d.hist().x(hist_data()).with(&o).draw();
                let more: Vec<f64> = hist_data().iter().map(|v| v * 1.05).collect();
                let o = HistOptions {
                    add: true,
                    col: vec![ColorSpec::Color(Color::parse("pink").unwrap())],
                    border: vec![red()],
                    ..Default::default()
                };
                d.hist().x(&more).with(&o).draw();
            },
        },
        Case {
            name: "barplot_logy",
            r: "barplot(c(3, 40, 700, 12), names.arg = c('a', 'b', 'c', 'd'), log = 'y')",
            draw: |d| {
                let o = BarplotOptions {
                    names: Some(["a", "b", "c", "d"].map(Label::from).to_vec()),
                    ylog: true,
                    ..Default::default()
                };
                d.barplot().height([3.0, 40.0, 700.0, 12.0]).with(&o).draw();
            },
        },
        Case {
            name: "barplot_stacked_logy",
            // No legend: on a log axis R misplaces it (log10 `usr` values
            // used as data), which we deliberately don't copy.
            r: "barplot(cbind(c(2, 5), c(30, 60), c(400, 100)), log = 'y')",
            draw: |d| {
                let o = BarplotOptions {
                    ylog: true,
                    ..Default::default()
                };
                d.barplot()
                    .height(&[vec![2.0, 5.0], vec![30.0, 60.0], vec![400.0, 100.0]])
                    .with(&o)
                    .draw();
            },
        },
        Case {
            name: "barplot_horiz_beside_logx",
            r: "barplot(cbind(c(2, 5), c(30, 60)), beside = TRUE, horiz = TRUE, \
                        log = 'x', xlim = c(1, 100))",
            draw: |d| {
                let o = BarplotOptions {
                    beside: true,
                    horiz: true,
                    xlog: true,
                    xlim: Some((1.0, 100.0)),
                    ..Default::default()
                };
                d.barplot()
                    .height(&[vec![2.0, 5.0], vec![30.0, 60.0]])
                    .with(&o)
                    .draw();
            },
        },
        Case {
            name: "plot_xaxt",
            r: "plot(1:10, xaxt = 'n', xlab = 'x', ylab = 'y')\n\
                axis(1, at = c(2, 5, 8), labels = c('a', 'b', 'c'))",
            draw: |d| {
                let x: Vec<f64> = (1..=10).map(f64::from).collect();
                let o = PlotOptions {
                    xaxt: Some(AxisType::None),
                    xlab: Some("x".into()),
                    ylab: Some("y".into()),
                    ..Default::default()
                };
                d.plot().x(&x).y(&x).with(&o).draw();
                let fg = ColorSpec::Index(1);
                let labels = ["a", "b", "c"].map(String::from).to_vec();
                d.axis()
                    .side(1)
                    .with(&AxisOptions {
                        at: Some(vec![2.0, 5.0, 8.0]),
                        labels: Some(labels).map(|v| {
                            v.into_iter()
                                .map(|t| Some(rs_graphics::plotmath::Label::from(t)))
                                .collect()
                        }),
                        col: Some(fg),
                        col_axis: Some(fg),
                        cex_axis: Some(1.0),
                        font: Some(FontFace::Plain),
                        las: Some(0),
                        ..AxisOptions::default()
                    })
                    .draw();
            },
        },
        Case {
            name: "boxplot_horizontal",
            r: "boxplot(list(c(1, 2, 2.5, 3, 7), c(2, 3, 3.5, 4, 4.2, 5)), horizontal = TRUE, \
                        names = c('a', 'b'), main = 'H', xlab = 'value')",
            draw: |d| {
                let o = BoxplotOptions {
                    horizontal: true,
                    names: Some(vec!["a".into(), "b".into()]),
                    main: Some("H".into()),
                    xlab: Some("value".into()),
                    ..Default::default()
                };
                d.boxplot()
                    .x(&[
                        vec![1.0, 2.0, 2.5, 3.0, 7.0],
                        vec![2.0, 3.0, 3.5, 4.0, 4.2, 5.0],
                    ])
                    .with(&o)
                    .draw();
            },
        },
        Case {
            name: "lab",
            r: "par(lab = c(10, 3, 7)); plot(c(0, 1), c(0, 1), xlab = 'x', ylab = 'y')\n\
                par(new = TRUE, lab = c(2, 12, 7)); \
                plot(c(1, 1e6), c(1, 50), log = 'xy', axes = FALSE, ann = FALSE); \
                axis(3); axis(4)",
            draw: |d| {
                d.par().lab([10, 3, 7]).draw();
                let o = PlotOptions { xlab: Some("x".into()), ylab: Some("y".into()), ..Default::default() };
                d.plot().x([0.0, 1.0]).y([0.0, 1.0]).with(&o).draw();
                d.par().new(true).draw();
                d.par().lab([2, 12, 7]).draw();
                let o = PlotOptions { xlog: true, ylog: true, axes: false, ..Default::default() };
                d.plot().x([1.0, 1e6]).y([1.0, 50.0]).with(&o).draw();
                let fg = ColorSpec::Index(1);
                d.axis()
                    .side(3)
                    .with(&AxisOptions {
                        col: Some(fg),
                        col_axis: Some(fg),
                        cex_axis: Some(1.0),
                        font: Some(FontFace::Plain),
                        las: Some(0),
                        ..AxisOptions::default()
                    })
                    .draw();
                d.axis()
                    .side(4)
                    .with(&AxisOptions {
                        col: Some(fg),
                        col_axis: Some(fg),
                        cex_axis: Some(1.0),
                        font: Some(FontFace::Plain),
                        las: Some(0),
                        ..AxisOptions::default()
                    })
                    .draw();
            },
        },
    ]
}

#[test]
fn log_plots_match_r() {
    if !common::r_available() {
        return;
    }
    let cases = cases();
    let page = (PAGE_W, PAGE_H);
    let hx: Vec<String> = hist_data().iter().map(|v| format!("{v:e}")).collect();
    let r_cases: Vec<_> = cases.iter().map(|c| (c.name, c.r)).collect();
    let r = r::run_r(
        "log_plots",
        page,
        &format!("hx <- c({})", hx.join(", ")),
        &r_cases,
    );
    let how = pdf::Compare {
        // R writes widths to 2 decimals.
        lwd: 0.0051,
        // R's `pdf()` draws `-` as the wider `/minus` glyph, which we
        // deliberately don't.
        skip_origin: |t| t.string.contains('-'),
        ..Default::default()
    };
    for (c, r_pdf) in cases.iter().zip(&r.pdfs) {
        let (ours_pdf, ()) = r::ours(page, c.draw);
        pdf::assert_same_pdf(c.name, pdf::parse(&ours_pdf), pdf::parse(r_pdf), &how);
    }
}
