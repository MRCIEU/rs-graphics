// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks `par(xaxp/yaxp)` against R: the values `plot.window()`
//! sets (linear, log with each tick-rule code, reversed, tiny ranges,
//! `par(lab)`), and the ticks `axis()` draws when they are changed
//! afterwards, overridden inline, or passed to `plot()` (every painted path
//! and text item in order vs. an uncompressed R `pdf()`, see
//! `common::pdf::assert_same`). Skipped if `Rscript` isn't available.

use crate::common;
use crate::common::{pdf, r};

use std::process::Command;


use rs_graphics::device::{AxisOptions, Device};
use rs_graphics::highlevel::PlotOptions;
use rs_graphics::prelude::*;

const PAGE_W: f64 = 7.0;
const PAGE_H: f64 = 6.0;

/// Runs `script` from its own file (the tests run in parallel).
fn rscript(name: &str, script: &str) -> String {
    let path = std::env::temp_dir().join(format!("rs_graphics_axp_{name}.R"));
    std::fs::write(&path, script).unwrap();
    let out = Command::new("Rscript").arg(&path).output().expect("run Rscript");
    assert!(
        out.status.success(),
        "Rscript failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// `(xlim, ylim, log, lab)`.
type Window = ((f64, f64), (f64, f64), &'static str, [i32; 2]);

/// Windows whose `xaxp`/`yaxp` are compared.
const WINDOWS: &[Window] = &[
    ((0.0, 10.0), (-3.0, 7.5), "", [5, 5]),
    ((10.0, 0.0), (0.001, 0.0013), "", [5, 5]),
    ((1.0, 1e6), (3.0, 700.0), "xy", [5, 5]),
    ((1.0, 1000.0), (2.0, 15.0), "xy", [5, 5]),
    ((1e-5, 1e5), (1e4, 1.0), "xy", [5, 5]),
    ((0.0, 1.0), (1.0, 1e6), "y", [10, 3]),
    ((5.0, 5.0 + 1e-14), (100.0, 100.0 * (1.0 + 1e-14)), "y", [5, 5]),
];

#[test]
fn axp_values_match_r() {
    if !common::r_available() {
        return;
    }
    let mut script = String::from("pdf(NULL)\n");
    for (xlim, ylim, log, lab) in WINDOWS {
        script.push_str(&format!(
            "par(lab = c({}, {}, 7)); plot.new()\n\
             suppressWarnings(plot.window(c({:e}, {:e}), c({:e}, {:e}), log = '{log}'))\n\
             cat(sprintf('%.17g', c(par('xaxp'), par('yaxp'))), '\\n')\n",
            lab[0], lab[1], xlim.0, xlim.1, ylim.0, ylim.1
        ));
    }
    let r = rscript("values", &script);
    assert_eq!(r.lines().count(), WINDOWS.len(), "R output: {r}");
    for (line, (xlim, ylim, log, lab)) in r.lines().zip(WINDOWS) {
        let want: Vec<f64> = line.split_whitespace().map(|t| t.parse().unwrap()).collect();
        let mut dev = Device::memory("pdf")
            .width(PAGE_W)
            .height(PAGE_H)
            .open()
            .unwrap();
        dev.par().lab([lab[0], lab[1], 7]).draw();
        dev.plot_new();
        dev.plot_window()
            .xlim(xlim.0, xlim.1)
            .ylim(ylim.0, ylim.1)
            .with(
                &PlotWindowOptions::default()
                    .xlog(log.contains('x'))
                    .ylog(log.contains('y')),
            )
            .draw();
        let p = dev.current_par();
        let ours: Vec<f64> = p.xaxp.iter().chain(&p.yaxp).copied().collect();
        let ok = ours
            .iter()
            .zip(&want)
            .all(|(a, b)| (a - b).abs() <= 1e-12 * b.abs().max(1e-300));
        assert!(ok, "xlim {xlim:?} ylim {ylim:?} log '{log}': ours {ours:?}, R {want:?}");
    }
}

struct Case {
    name: &'static str,
    r: &'static str,
    draw: fn(&mut Device<AnyBackend>),
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "par_after_window",
            r: "plot.new(); plot.window(c(0, 10), c(1, 1000), log = 'y')\n\
                par(xaxp = c(0, 10, 2), yaxp = c(1, 1000, 2))\n\
                axis(1); axis(2); axis(3, xaxp = c(2, 8, 3)); axis(4, yaxp = c(1, 100, 3))",
            draw: |d| {
                d.plot_new();
                d.plot_window()
                    .xlim(0.0, 10.0)
                    .ylim(1.0, 1000.0)
                    .with(&PlotWindowOptions::default().ylog(true))
                    .draw();
                d.par().xaxp([0.0, 10.0, 2.0]).draw();
                d.par().yaxp([1.0, 1000.0, 2.0]).draw();
                d.axis().side(1).draw();
                d.axis().side(2).draw();
                let o = AxisOptions::default().axp([2.0, 8.0, 3.0]);
                d.axis().side(3).with(&o).draw();
                let o = AxisOptions::default().axp([1.0, 100.0, 3.0]);
                d.axis().side(4).with(&o).draw();
            },
        },
        Case {
            name: "plot_inline",
            r: "plot(1:10, xaxp = c(1, 10, 3), yaxp = c(2, 8, 6), xlab = 'x', ylab = 'y')",
            draw: |d| {
                let x: Vec<f64> = (1..=10).map(f64::from).collect();
                let o = PlotOptions::default()
                    .xaxp([1.0, 10.0, 3.0])
                    .yaxp([2.0, 8.0, 6.0])
                    .xlab("x")
                    .ylab("y");
                d.plot().x(&x).y(&x).with(&o).draw();
                // Like R, `par()` keeps what `plot.window()` computed.
                assert_eq!(d.current_par().xaxp, [2.0, 10.0, 4.0]);
            },
        },
    ]
}

#[test]
fn axp_ticks_match_r() {
    if !common::r_available() {
        return;
    }
    let cases = cases();
    let page = (PAGE_W, PAGE_H);
    let r_cases: Vec<_> = cases.iter().map(|c| (c.name, c.r)).collect();
    let r = r::run_r("axp", page, "", &r_cases);
    for (c, r_pdf) in cases.iter().zip(&r.pdfs) {
        let (ours, ()) = r::ours(page, c.draw);
        pdf::assert_same_pdf(
            c.name,
            pdf::parse(&ours),
            pdf::parse(r_pdf),
            &pdf::Compare::default(),
        );
    }
}
