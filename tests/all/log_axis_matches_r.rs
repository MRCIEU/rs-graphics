// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks log axes against a real R `pdf()` device's uncompressed
//! output: `plot(x, y, log = ...)` over ranges that hit each of R's log
//! tick rules (`GLPretty()` + `CreateAtVector()`: 1-2-5, 1-5, powers of
//! ten with decimation, a linear axis for small ranges, a reversed axis),
//! with non-positive values that must be dropped, plus explicit `at` in
//! data units. Compares every painted path and text item in order (see
//! `common::pdf::assert_same`), except the origins of labels with a minus
//! sign (R's `/minus`). Skipped if `Rscript` isn't available.

use crate::common;
use crate::common::{pdf, r};

use rs_graphics::backend::pdf::PdfBackend;
use rs_graphics::device::{AxisOptions, Device};
use rs_graphics::graphics::Graphics;
use rs_graphics::highlevel::PlotOptions;

const PAGE_W: f64 = 7.0;
const PAGE_H: f64 = 6.0;
const N: usize = 20;

struct Case {
    name: &'static str,
    x: (f64, f64),
    y: (f64, f64),
    xlog: bool,
    ylog: bool,
    xlim: Option<(f64, f64)>,
    /// Extra `axis(3, at = ...)` call, in data units.
    at3: Option<&'static [f64]>,
}

/// `N` values spaced evenly in log10 (or linearly) from `r.0` to `r.1`,
/// after a 0 and a -1 that a log axis must drop.
fn values(r: (f64, f64), log: bool) -> Vec<f64> {
    let mut v = vec![0.0, -1.0];
    for i in 0..N {
        let t = i as f64 / (N - 1) as f64;
        v.push(if log {
            10f64.powf(r.0.log10() + t * (r.1.log10() - r.0.log10()))
        } else {
            r.0 + t * (r.1 - r.0)
        });
    }
    v
}

fn r_vec(v: &[f64]) -> String {
    let s: Vec<String> = v.iter().map(|x| format!("{x:e}")).collect();
    format!("c({})", s.join(", "))
}

fn cases() -> Vec<Case> {
    let c = |name, x, y, xlog, ylog| Case {
        name,
        x,
        y,
        xlog,
        ylog,
        xlim: None,
        at3: None,
    };
    vec![
        // y: 2.4 decades -> 1, 2, 5; x: 6 decades -> powers of ten.
        c("one_two_five", (1.0, 1e6), (3.0, 700.0), true, true),
        // x: 3 decades -> 1, 5; y linear.
        c("one_five", (1.0, 1000.0), (0.0, 10.0), true, false),
        // 10 decades -> every other power of ten.
        c("decimated", (1e-5, 1e5), (2.0, 15.0), true, true),
        // 200 decades -> a large step, starting further in.
        c("huge", (1e-100, 1e100), (0.02, 0.9), true, true),
        // y: 2..15 is under a decade -> linear pretty() ticks.
        c("small", (0.5, 4.0), (2.0, 15.0), false, true),
        Case {
            xlim: Some((1e4, 1.0)),
            at3: Some(&[1.0, 3.0, 30.0, 300.0, 3000.0, 1e5]),
            ..c("reversed_at", (1.0, 1e4), (1.0, 100.0), true, false)
        },
    ]
}

fn r_code(c: &Case) -> String {
    let log = format!(
        "{}{}",
        if c.xlog { "x" } else { "" },
        if c.ylog { "y" } else { "" }
    );
    let xlim = c
        .xlim
        .map(|(a, b)| format!(", xlim = c({a:e}, {b:e})"))
        .unwrap_or_default();
    let at3 = c
        .at3
        .map(|a| format!("axis(3, at = {})\n", r_vec(a)))
        .unwrap_or_default();
    format!(
        "suppressWarnings(plot({x}, {y}, log = '{log}', xlab = 'x', ylab = 'y'{xlim}))\n{at3}",
        x = r_vec(&values(c.x, c.xlog)),
        y = r_vec(&values(c.y, c.ylog)),
    )
}

fn draw_ours(c: &Case, dev: &mut Device<PdfBackend>) {
    let mut opts = PlotOptions::default()
        .xlog(c.xlog)
        .ylog(c.ylog)
        .xlab("x")
        .ylab("y");
    if let Some((a, b)) = c.xlim {
        opts = opts.xlim(a, b);
    }
    dev.plot()
        .x(values(c.x, c.xlog))
        .y(values(c.y, c.ylog))
        .with(&opts)
        .draw();
    if let Some(at) = c.at3 {
        let o = AxisOptions::default().at(at.to_vec());
        dev.axis().side(3).with(&o).draw();
    }
}

#[test]
fn log_axes_match_r() {
    if !common::r_available() {
        return;
    }
    let cases = cases();
    let page = (PAGE_W, PAGE_H);
    let codes: Vec<String> = cases.iter().map(r_code).collect();
    let r_cases: Vec<_> = cases
        .iter()
        .zip(&codes)
        .map(|(c, r)| (c.name, r.as_str()))
        .collect();
    let r = r::run_r("log_axis", page, "", &r_cases);
    // R's `pdf()` draws `-` as the wider `/minus` glyph, which we
    // deliberately don't, so "1e-05" is laid out differently.
    let how = pdf::Compare {
        skip_origin: |t| t.string.contains('-'),
        ..Default::default()
    };
    for (c, r_pdf) in cases.iter().zip(&r.pdfs) {
        let (ours, ()) = r::ours(page, |dev| draw_ours(c, dev));
        pdf::assert_same_pdf(c.name, pdf::parse(&ours), pdf::parse(r_pdf), &how);
    }
}
