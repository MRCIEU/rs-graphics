// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! `plot_new()` panics with R's message when the regions don't fit
//! ("figure margins too large", "figure region too large", "plot region
//! too large"), in the same cases as R's `plot.new()` errors, and leaves
//! the figure counter where R's `invalidError()` does, so a retry reuses
//! the figure.

mod common;

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::process::Command;

use rs_graphics::prelude::*;

/// One case: R code after `pdf(file, w, h)` that ends in `plot.new()`, and
/// the same set-up in Rust (`plot_new()` is called after it).
struct Case {
    name: &'static str,
    size: (f64, f64),
    r: &'static str,
    rust: fn(&mut Device<PdfBackend>),
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "small_device",
            size: (1.5, 1.5),
            r: "",
            rust: |_| {},
        },
        Case {
            name: "narrow_device",
            size: (1.2, 7.0),
            r: "",
            rust: |_| {},
        },
        Case {
            name: "oma_wider_than_device",
            size: (7.0, 7.0),
            r: "par(oma = c(40, 0, 0, 0))",
            rust: |d| d.par().oma([40, 0, 0, 0]).draw(),
        },
        Case {
            name: "oma_with_mfrow",
            size: (7.0, 7.0),
            r: "par(oma = c(40, 0, 0, 0), mfrow = c(2, 1))",
            rust: |d| d.par().oma([40, 0, 0, 0]).mfrow(2, 1).draw(),
        },
        Case {
            name: "fin_too_wide",
            size: (7.0, 7.0),
            r: "par(fin = c(10, 3))",
            rust: |d| d.par().fin([10, 3]).draw(),
        },
        Case {
            name: "fin_with_omi_beyond_device",
            size: (7.0, 7.0),
            r: "par(omi = c(4, 0, 4, 0)); par(fin = c(3, 3))",
            rust: |d| {
                d.par().omi([4, 0, 4, 0]).draw();
                d.par().fin([3, 3]).draw();
            },
        },
        Case {
            name: "pin_too_wide",
            size: (7.0, 7.0),
            r: "par(pin = c(10, 3))",
            rust: |d| d.par().pin([10, 3]).draw(),
        },
        Case {
            name: "plt_beyond_figure",
            size: (7.0, 7.0),
            r: "par(plt = c(0, 1.2, 0, 1))",
            rust: |d| d.par().plt([0.0, 1.2, 0.0, 1.0]).draw(),
        },
        Case {
            name: "mai_too_large",
            size: (7.0, 7.0),
            r: "par(mai = c(4, 0, 4, 0))",
            rust: |d| d.par().mai([4, 0, 4, 0]).draw(),
        },
        Case {
            name: "mai_just_over_half",
            size: (2.0, 2.0),
            r: "par(mai = c(1, 0, 1.0000001, 0))",
            rust: |d| d.par().mai([1.0, 0.0, 1.0000001, 0.0]).draw(),
        },
        Case {
            name: "many_panels",
            size: (4.0, 4.0),
            r: "par(mfrow = c(6, 6))",
            rust: |d| d.par().mfrow(6, 6).draw(),
        },
        Case {
            name: "lcm_layout_too_wide",
            size: (4.0, 4.0),
            r: "layout(matrix(1:2, 1), widths = c(lcm(8), lcm(8)))",
            rust: |d| {
                let l = Layout::new(&[vec![1, 2]])
                    .unwrap()
                    .widths(&[LayoutSize::Cm(8.0), LayoutSize::Cm(8.0)]);
                d.layout(l);
            },
        },
        Case {
            name: "fits_square_no_margins",
            size: (7.0, 7.0),
            r: "par(mar = c(0, 0, 0, 0), pty = 's')",
            rust: |d| d.par().mar([0, 0, 0, 0]).pty("s").draw(),
        },
        Case {
            name: "fits_default",
            size: (7.0, 7.0),
            r: "",
            rust: |_| {},
        },
    ]
}

fn device(w: f64, h: f64) -> Device<PdfBackend> {
    Device::new(PdfBackend::new(w, h), Par::default())
}

/// The panic message of `f`, or `"ok"`.
fn outcome(f: impl FnOnce()) -> String {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(()) => "ok".to_string(),
        Err(e) => e
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| e.downcast_ref::<String>().cloned())
            .unwrap_or_default(),
    }
}

/// Runs R code and returns its standard output.
fn run_r(code: &str) -> String {
    let out = Command::new("Rscript")
        .args(["--vanilla", "-e", code])
        .output()
        .expect("run Rscript");
    assert!(
        out.status.success(),
        "R failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap()
}

#[test]
fn region_errors_match_r() {
    if !common::r_available() {
        return;
    }
    let cases = cases();
    let mut r = String::from(
        "t <- function(w, h, expr) { f <- tempfile(fileext = '.pdf'); pdf(f, w, h); \
         r <- tryCatch({ expr; plot.new(); 'ok' }, error = function(e) conditionMessage(e)); \
         invisible(dev.off()); cat(r, '\\n', sep = '') }\n",
    );
    for c in &cases {
        r.push_str(&format!("t({}, {}, {{ {} }})\n", c.size.0, c.size.1, c.r));
    }
    let expected: Vec<String> = run_r(&r).lines().map(str::to_string).collect();
    assert_eq!(expected.len(), cases.len());
    // The cases exercise every message (and both sides of the checks).
    for msg in [
        "ok",
        "figure margins too large",
        "figure region too large",
        "plot region too large",
    ] {
        assert!(expected.iter().any(|e| e == msg), "no case gives {msg:?}");
    }
    // Keep the panic messages of the expected failures out of the output.
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let got: Vec<String> = cases
        .iter()
        .map(|c| {
            let mut dev = device(c.size.0, c.size.1);
            outcome(|| {
                (c.rust)(&mut dev);
                dev.plot_new();
            })
        })
        .collect();
    std::panic::set_hook(hook);
    for ((c, e), g) in cases.iter().zip(&expected).zip(&got) {
        assert_eq!(g, e, "case {}", c.name);
    }
}

/// R's `invalidError()` steps the figure counter back, so after an error
/// and a fix, `plot.new()` uses the figure that failed.
#[test]
fn failed_figure_is_retried_like_r() {
    if !common::r_available() {
        return;
    }
    let r = run_r(
        "f <- tempfile(fileext = '.pdf'); pdf(f, 7, 7); par(mfrow = c(1, 3)); \
         plot.new(); par(mar = c(60, 1, 1, 1)); m <- tryCatch(plot.new(), error = conditionMessage); \
         a <- par('mfg')[2]; par(mar = c(5, 4, 4, 2) + 0.1); plot.new(); \
         b <- par('mfg')[2]; plot.new(); c <- par('mfg')[2]; invisible(dev.off()); \
         g <- tempfile(fileext = '.pdf'); pdf(g, 7, 7); par(mar = c(40, 1, 1, 1)); \
         m2 <- tryCatch(plot.new(), error = conditionMessage); d <- par('mfg')[2]; \
         par(mar = c(5, 4, 4, 2) + 0.1); plot.new(); e <- par('mfg')[2]; \
         invisible(dev.off()); cat(m, m2, a, b, c, d, e, sep = '|')",
    );
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let mut dev = device(7.0, 7.0);
    dev.par().mfrow(1, 3).draw();
    dev.plot_new();
    dev.par().mar([60, 1, 1, 1]).draw();
    let err = outcome(|| dev.plot_new());
    let a = dev.current_figure();
    dev.par().mar([5.1, 4.1, 4.1, 2.1]).draw();
    dev.plot_new();
    let b = dev.current_figure();
    dev.plot_new();
    let c = dev.current_figure();
    // An error on the very first plot.
    let mut dev = device(7.0, 7.0);
    dev.par().mar([40, 1, 1, 1]).draw();
    let err2 = outcome(|| dev.plot_new());
    let d = dev.current_figure();
    dev.par().mar([5.1, 4.1, 4.1, 2.1]).draw();
    dev.plot_new();
    let e = dev.current_figure();
    std::panic::set_hook(hook);
    assert_eq!(format!("{err}|{err2}|{a}|{b}|{c}|{d}|{e}"), r.trim());
    assert!(r.starts_with("figure margins too large|figure margins too large|"));
}

/// High-level plots start with `plot_new()`, so they fail the same way.
#[test]
#[should_panic(expected = "figure margins too large")]
fn high_level_plots_panic_too() {
    let mut dev = device(1.0, 1.0);
    dev.plot(&[1, 2], &[1, 2]).draw();
}
