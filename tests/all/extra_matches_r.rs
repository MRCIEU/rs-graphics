// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks the options that aren't in R (`rs_graphics::extra`)
//! against the R code each replaces, run in R: `label_srt` against
//! `axis(labels = FALSE)` plus `text(srt = , adj = c(1, 0.5), xpd = NA)` at
//! the labels' line (`mgp[1]` lines out), and `label_srt(90)` against
//! `las = 2`; `panel_bg` against `panel.first = rect(<par("usr")>, border =
//! NA)` in `plot()`, `hist()`, `barplot()` and `boxplot()`, on linear and
//! log axes. Every painted path and text item in order (see
//! `common::pdf::assert_same`), on a real R `pdf()` device's uncompressed
//! output. Also: the extras are the same in a `Figure` and through
//! `.with(&opts)`, and a bad colour is the call's error. Skipped if
//! `Rscript` isn't available.

use crate::common;
use crate::common::{pdf, r};

use rs_graphics::prelude::*;

const PAGE: (f64, f64) = (7.0, 7.0);

/// R helpers: `bg(col)` fills the plot region (data units on log axes);
/// `lab_y(n)`/`lab_x(n)`/`lab_y3(n)` are the user coordinate `n` margin
/// lines below/left of/above the plot region.
const PRELUDE: &str = "
bg <- function(col) {
  u <- par('usr')
  if (par('xlog')) u[1:2] <- 10^u[1:2]
  if (par('ylog')) u[3:4] <- 10^u[3:4]
  rect(u[1], u[3], u[2], u[4], col = col, border = NA)
}
lh <- function() par('csi') * par('mex')
lab_y <- function(n) { u <- par('usr'); u[3] - n * lh() / par('pin')[2] * (u[4] - u[3]) }
lab_y3 <- function(n) { u <- par('usr'); u[4] + n * lh() / par('pin')[2] * (u[4] - u[3]) }
lab_x <- function(n) { u <- par('usr'); u[1] - n * lh() / par('pin')[1] * (u[2] - u[1]) }
";

struct Case {
    name: &'static str,
    r: &'static str,
    ours: fn(&mut Device<AnyBackend>),
}

const NAMES: [&str; 3] = ["alpha", "beta", "gamma"];

fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "axis_side1",
            r: "plot.new(); plot.window(c(0, 4), c(0, 1))\n\
                axis(1, at = 1:3, labels = FALSE)\n\
                text(1:3, lab_y(1), c('alpha', 'beta', 'gamma'), srt = 45, \
                adj = c(1, 0.5), xpd = NA)",
            ours: |d| {
                d.plot_new();
                d.plot_window().xlim([0, 4]).ylim([0, 1]).draw();
                d.axis()
                    .side(1)
                    .at([1, 2, 3])
                    .labels(NAMES)
                    .label_srt(45)
                    .draw();
            },
        },
        Case {
            name: "axis_side2_mgp",
            r: "par(mgp = c(3, 2, 0)); plot.new(); plot.window(c(0, 1), c(0, 4))\n\
                axis(2, at = 1:3, labels = FALSE)\n\
                text(lab_x(2), 1:3, c('alpha', 'beta', 'gamma'), srt = 30, \
                adj = c(1, 0.5), xpd = NA)",
            ours: |d| {
                d.par().mgp([3, 2, 0]).draw();
                d.plot_new();
                d.plot_window().xlim([0, 1]).ylim([0, 4]).draw();
                d.axis()
                    .side(2)
                    .at([1, 2, 3])
                    .labels(NAMES)
                    .label_srt(30)
                    .draw();
            },
        },
        Case {
            // Sides 3 and 4 start the label at the tick.
            name: "axis_side3",
            r: "plot.new(); plot.window(c(0, 4), c(0, 1))\n\
                axis(3, at = 1:3, labels = FALSE)\n\
                text(1:3, lab_y3(1), c('alpha', 'beta', 'gamma'), srt = 45, \
                adj = c(0, 0.5), xpd = NA)",
            ours: |d| {
                d.plot_new();
                d.plot_window().xlim([0, 4]).ylim([0, 1]).draw();
                d.axis()
                    .side(3)
                    .at([1, 2, 3])
                    .labels(NAMES)
                    .label_srt(45)
                    .draw();
            },
        },
        Case {
            name: "srt_90_is_las_2",
            r: "plot.new(); plot.window(c(0, 4), c(0, 1))\n\
                axis(1, at = 1:3, labels = c('alpha', 'beta', 'gamma'), las = 2)",
            ours: |d| {
                d.plot_new();
                d.plot_window().xlim([0, 4]).ylim([0, 1]).draw();
                d.axis()
                    .side(1)
                    .at([1, 2, 3])
                    .labels(NAMES)
                    .label_srt(90)
                    .draw();
            },
        },
        Case {
            // Rotated labels are never dropped for overlapping: these would be.
            name: "no_gap_dropping",
            r: "plot.new(); plot.window(c(0, 4), c(0, 1))\n\
                axis(1, at = c(1, 1.1, 1.2), labels = FALSE)\n\
                text(c(1, 1.1, 1.2), lab_y(1), c('alpha', 'beta', 'gamma'), \
                srt = 20, adj = c(1, 0.5), xpd = NA)",
            ours: |d| {
                d.plot_new();
                d.plot_window().xlim([0, 4]).ylim([0, 1]).draw();
                d.axis()
                    .side(1)
                    .at([1.0, 1.1, 1.2])
                    .labels(NAMES)
                    .label_srt(20)
                    .draw();
            },
        },
        Case {
            name: "barplot_names_and_bg",
            r: "bp <- barplot(c(3, 5, 2), axisnames = FALSE, axes = FALSE, \
                panel.first = bg('grey90'))\n\
                text(bp, lab_y(1), c('alpha', 'beta', 'gamma'), srt = 45, \
                adj = c(1, 0.5), xpd = NA)\n\
                axis(2)",
            ours: |d| {
                d.barplot()
                    .height([3, 5, 2])
                    .names(NAMES)
                    .label_srt(45)
                    .panel_bg("grey90")
                    .draw();
            },
        },
        Case {
            name: "plot_bg",
            r: "plot(1:5, c(2, 4, 3, 5, 1), xlab = 'x', ylab = 'y', \
                panel.first = bg('grey90'))",
            ours: |d| {
                d.plot()
                    .x(1..=5)
                    .y([2, 4, 3, 5, 1])
                    .xlab("x")
                    .ylab("y")
                    .panel_bg("grey90")
                    .draw();
            },
        },
        Case {
            name: "plot_bg_log",
            r: "plot(c(1, 10, 100), c(2, 20, 200), log = 'xy', xlab = 'x', ylab = 'y', \
                panel.first = bg('lightblue'))",
            ours: |d| {
                d.plot()
                    .x([1, 10, 100])
                    .y([2, 20, 200])
                    .xlog(true)
                    .ylog(true)
                    .xlab("x")
                    .ylab("y")
                    .panel_bg("lightblue")
                    .draw();
            },
        },
        Case {
            name: "plot_x_labels",
            // Our `plot()` draws its title last, R's before the axes.
            r: "plot(1:3, c(2, 1, 3), type = 'n', axes = FALSE, ann = FALSE)\n\
                points(1:3, c(2, 1, 3))\n\
                at <- axTicks(1); axis(1, labels = FALSE)\n\
                text(at, lab_y(1), format(at), srt = 45, adj = c(1, 0.5), xpd = NA)\n\
                axis(2); box(); title(xlab = 'x', ylab = 'y')",
            ours: |d| {
                d.plot()
                    .x(1..=3)
                    .y([2, 1, 3])
                    .xlab("x")
                    .ylab("y")
                    .label_srt(45)
                    .draw();
            },
        },
        Case {
            name: "hist_bg",
            r: "x <- c(1, 2, 2, 3, 3, 3, 4)\n\
                hist(x, ylab = 'n', panel.first = bg('grey90'))",
            ours: |d| {
                d.hist()
                    .x([1, 2, 2, 3, 3, 3, 4])
                    .ylab("n")
                    .panel_bg("grey90")
                    .draw();
            },
        },
        Case {
            // R's `hist()` draws its bars after the axes, so the R code
            // draws the histogram by hand, in `plot.histogram()`'s order.
            name: "hist_x_labels",
            r: "x <- c(1, 2, 2, 3, 3, 3, 4); h <- hist(x, plot = FALSE)\n\
                plot.new(); plot.window(range(h$breaks), c(0, max(h$counts)))\n\
                title(main = 'Histogram of x', xlab = 'x', ylab = 'n')\n\
                at <- axTicks(1); axis(1, labels = FALSE)\n\
                text(at, lab_y(1), format(at), srt = 45, adj = c(1, 0.5), xpd = NA)\n\
                yt <- axTicks(2); axis(2, at = yt[yt %% 1 == 0])\n\
                rect(h$breaks[-length(h$breaks)], 0, h$breaks[-1], h$counts, \
                col = 'lightgray')",
            ours: |d| {
                d.hist()
                    .x([1, 2, 2, 3, 3, 3, 4])
                    .ylab("n")
                    .label_srt(45)
                    .draw();
            },
        },
        Case {
            name: "boxplot_names",
            r: "boxplot(list(1:5, 2:6), axes = FALSE)\n\
                axis(1, at = 1:2, labels = FALSE)\n\
                text(1:2, lab_y(1), c('first', 'second'), srt = 60, \
                adj = c(1, 0.5), xpd = NA)\n\
                axis(2); box()",
            ours: |d| {
                d.boxplot()
                    .x([vec![1, 2, 3, 4, 5], vec![2, 3, 4, 5, 6]])
                    .names(["first", "second"])
                    .label_srt(60)
                    .draw();
            },
        },
        Case {
            name: "boxplot_horizontal_bg",
            r: "boxplot(list(1:5, 2:9), horizontal = TRUE, \
                panel.first = function() bg('grey90'))",
            ours: |d| {
                d.boxplot()
                    .x([vec![1, 2, 3, 4, 5], vec![2, 3, 4, 5, 6, 7, 8, 9]])
                    .horizontal(true)
                    .panel_bg("grey90")
                    .draw();
            },
        },
    ]
}

#[test]
fn extras_match_the_r_code_they_replace() {
    if !common::r_available() {
        return;
    }
    let cases = cases();
    let r_cases: Vec<_> = cases.iter().map(|c| (c.name, c.r)).collect();
    let r = r::run_r("extra", PAGE, PRELUDE, &r_cases);
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

/// A scene using every extra.
fn scene<G: Graphics>(g: &mut G) {
    g.par().mfrow([2, 2]).draw();
    g.plot()
        .x(1..=5)
        .y([2, 4, 3, 5, 1])
        .panel_bg("grey90")
        .label_srt(30)
        .draw();
    g.hist()
        .x([1, 2, 2, 3, 3, 3, 4])
        .panel_bg("lightblue")
        .draw();
    g.barplot()
        .height([3, 5, 2])
        .names(NAMES)
        .label_srt(45)
        .panel_bg("grey80")
        .draw();
    g.boxplot()
        .x([vec![1, 2, 3, 4, 5], vec![2, 3, 4, 5, 6]])
        .names(["a", "b"])
        .label_srt(60)
        .panel_bg("grey95")
        .draw();
    g.axis().side(3).label_srt(20).draw();
}

#[test]
fn extras_are_the_same_in_a_figure() {
    let mut fig = Figure::new();
    scene(&mut fig);
    let mut recorded = r::device(PAGE);
    recorded.add_figure(&fig);
    let mut live = r::device(PAGE);
    scene(&mut live);
    assert_eq!(recorded.finish(), live.finish());
}

#[test]
fn extras_are_kept_by_with() {
    let opts = BarplotOptions::default()
        .names(NAMES)
        .label_srt(45)
        .panel_bg("grey90");
    let (with, ()) = r::ours(PAGE, |d| {
        d.barplot().height([3, 5, 2]).with(&opts).draw();
    });
    let (direct, ()) = r::ours(PAGE, |d| {
        d.barplot()
            .height([3, 5, 2])
            .names(NAMES)
            .label_srt(45)
            .panel_bg("grey90")
            .draw();
    });
    assert_eq!(with, direct);
    let axis = AxisOptions::default().label_srt(45);
    let (with, ()) = r::ours(PAGE, |d| {
        d.plot_new();
        d.axis().side(1).with(&axis).draw();
    });
    let (direct, ()) = r::ours(PAGE, |d| {
        d.plot_new();
        d.axis().side(1).label_srt(45).draw();
    });
    assert_eq!(with, direct);
}

#[test]
fn a_bad_panel_colour_is_the_calls_error() {
    let mut dev = r::device(PAGE);
    let e = dev
        .plot()
        .x(1..=3)
        .panel_bg("notacolour")
        .try_draw()
        .unwrap_err();
    assert_eq!(e.to_string(), "plot(): invalid color name 'notacolour'");
}
