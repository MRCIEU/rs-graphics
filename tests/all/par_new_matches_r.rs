// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks `par(new = TRUE)` against a real R `pdf()` device's
//! uncompressed output: a second y axis overlaid on a plot, overlays in a
//! multi-figure page, `plot.new()` not resetting it, drawing and outer
//! `mtext()` (not) resetting it, and setting it before anything is
//! plotted. Compares every painted path and text item in order (see
//! `common::pdf::assert_same`). Skipped if `Rscript` isn't available.

use crate::common;
use crate::common::{pdf, r};


use rs_graphics::backend::pdf::PdfBackend;
use rs_graphics::color::{Color, ColorSpec};
use rs_graphics::device::{Device, MtextOptions};
use rs_graphics::highlevel::{PlotOptions, PlotType};
use rs_graphics::par::{Par, Sides};
use rs_graphics::prelude::*;

const PAGE_W: f64 = 8.0;
const PAGE_H: f64 = 6.0;

type Dev = Device<PdfBackend>;

fn xs() -> Vec<f64> {
    (1..=10).map(|i| i as f64).collect()
}

/// R: `pl(y, main, ...)` draws `plot(x, y, type = "l", ...)` with the
/// given title and labels "x"/"y" (or none, for `axes = FALSE`).
const R_PRELUDE: &str = "x <- 1:10\n\
    pl <- function(y, main = '', axes = TRUE, ...) {\n\
      lab <- if (axes) c('x', 'y') else c('', '')\n\
      plot(x, y, type = 'l', main = main, axes = axes, xlab = lab[1], ylab = lab[2], ...)\n\
    }\n";

/// Ours: as `pl()` above.
fn pl(dev: &mut Dev, y: &[f64], main: Option<&str>, axes: bool) {
    let opts = PlotOptions::default()
        .type_(PlotType::Lines)
        .main(main)
        .axes(axes)
        .xlab(axes.then_some("x"))
        .ylab(axes.then_some("y"));
    dev.plot().x(xs()).y(y).with(&opts).draw();
}

fn squares() -> Vec<f64> {
    xs().iter().map(|v| v * v).collect()
}

fn roots() -> Vec<f64> {
    xs().iter().map(|v| v.sqrt()).collect()
}

fn falling() -> Vec<f64> {
    xs().iter().map(|v| 11.0 - v).collect()
}

struct Case {
    name: &'static str,
    r: &'static str,
    setup: fn(&mut Par),
    draw: fn(&mut Dev),
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "second_axis",
            r: "par(mar = c(5, 4, 4, 5)); pl(x^2, 'Two scales')\n\
                par(new = TRUE); pl(sqrt(x), axes = FALSE, col = 'red')\n\
                axis(4); mtext('sqrt', side = 4, line = 3)",
            setup: |p| p.mar = Sides::new(5.0, 4.0, 4.0, 5.0),
            draw: |d| {
                pl(d, &squares(), Some("Two scales"), true);
                d.par().new(true).draw();
                let opts = PlotOptions::default()
                    .type_(PlotType::Lines)
                    .axes(false)
                    .col(vec![ColorSpec::Color(Color::try_from("red").unwrap())]);
                d.plot().x(xs()).y(roots()).with(&opts).draw();
                d.axis().side(4).draw();
                let o = MtextOptions::default().line(vec![3.0]);
                d.mtext().text("sqrt").side(4).with(&o).draw();
            },
        },
        Case {
            name: "mfrow_overlay",
            r: "par(mfrow = c(2, 2)); pl(x^2, 'A')\n\
                par(new = TRUE); pl(rev(x)^2, axes = FALSE)\n\
                pl(x, 'B'); pl(sqrt(x), 'C')",
            setup: |_| {},
            draw: |d| {
                d.par().mfrow(2, 2).draw();
                pl(d, &squares(), Some("A"), true);
                d.par().new(true).draw();
                let rev: Vec<f64> = squares().into_iter().rev().collect();
                pl(d, &rev, None, false);
                pl(d, &xs(), Some("B"), true);
                pl(d, &roots(), Some("C"), true);
            },
        },
        Case {
            name: "plot_new_keeps_it",
            r: "par(mfrow = c(1, 2)); pl(x^2, 'A')\n\
                par(new = TRUE); plot.new(); plot.new()\n\
                plot.window(c(0, 20), c(0, 1)); axis(3)\n\
                pl(x, 'B')",
            setup: |_| {},
            draw: |d| {
                d.par().mfrow(1, 2).draw();
                pl(d, &squares(), Some("A"), true);
                d.par().new(true).draw();
                d.plot_new();
                d.plot_new();
                d.plot_window().xlim(0.0, 20.0).ylim(0.0, 1.0).draw();
                d.axis().side(3).draw();
                pl(d, &xs(), Some("B"), true);
            },
        },
        Case {
            name: "drawing_resets_it",
            r: "par(mfrow = c(1, 2)); pl(x^2, 'A')\n\
                par(new = TRUE); points(5, 50, pch = 19)\n\
                pl(falling(), 'B')",
            setup: |_| {},
            draw: |d| {
                d.par().mfrow(1, 2).draw();
                pl(d, &squares(), Some("A"), true);
                d.par().new(true).draw();
                let black = ColorSpec::Index(1);
                d.points()
                    .x([5.0])
                    .y([50.0])
                    .with(&XyOptions::default().pch(vec![Some(19)]).col(vec![black]))
                    .draw();
                assert!(!d.is_new());
                pl(d, &falling(), Some("B"), true);
            },
        },
        Case {
            name: "outer_mtext_keeps_it",
            r: "par(mfrow = c(1, 2), oma = c(0, 0, 2, 0)); pl(x^2, 'A')\n\
                par(new = TRUE); mtext('Outer', side = 3, outer = TRUE)\n\
                pl(falling(), axes = FALSE); pl(x, 'B')",
            setup: |p| p.oma = Sides::new(0.0, 0.0, 2.0, 0.0),
            draw: |d| {
                d.par().mfrow(1, 2).draw();
                pl(d, &squares(), Some("A"), true);
                d.par().new(true).draw();
                let o = MtextOptions::default().outer(true);
                d.mtext().text("Outer").side(3).with(&o).draw();
                assert!(d.is_new());
                pl(d, &falling(), None, false);
                pl(d, &xs(), Some("B"), true);
            },
        },
        Case {
            name: "ignored_before_plotting",
            r: "par(mfrow = c(1, 2)); suppressWarnings(par(new = TRUE))\n\
                pl(x^2, 'A'); pl(x, 'B')",
            setup: |_| {},
            draw: |d| {
                d.par().mfrow(1, 2).draw();
                d.par().new(true).draw();
                assert!(!d.is_new());
                pl(d, &squares(), Some("A"), true);
                pl(d, &xs(), Some("B"), true);
            },
        },
    ]
}

/// `(x, y)` of every text matrix, and the vertices of every stroked path
/// with at least 5 points (the data lines), each sorted.
#[test]
fn par_new_matches_r() {
    if !common::r_available() {
        return;
    }
    let cases = cases();
    let page = (PAGE_W, PAGE_H);
    let codes: Vec<String> = cases
        .iter()
        .map(|c| format!("falling <- function() 11 - x\n{}", c.r))
        .collect();
    let r_cases: Vec<_> = cases
        .iter()
        .zip(&codes)
        .map(|(c, r)| (c.name, r.as_str()))
        .collect();
    let r = r::run_r("par_new", page, R_PRELUDE, &r_cases);
    for (c, r_pdf) in cases.iter().zip(&r.pdfs) {
        let mut par = Par::default();
        (c.setup)(&mut par);
        let (ours, ()) = r::ours_with(page, par, c.draw);
        pdf::assert_same_pdf(
            c.name,
            pdf::parse(&ours),
            pdf::parse(r_pdf),
            &pdf::Compare::default(),
        );
    }
}
