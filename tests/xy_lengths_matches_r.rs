// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! `plot()`, `points()`, `lines()` and `polygon()` panic with R's
//! `xy.coords()` message when `x` and `y` differ in length (on a device
//! and in a `Figure`), as R errors, while `text()` recycles them, as R
//! does. Skipped if `Rscript` isn't available.

mod common;

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::process::Command;

use rs_graphics::prelude::*;

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

/// The calls, generic over the target so a device and a `Figure` run the
/// same code.
fn call<G: Graphics>(g: &mut G, i: usize) {
    match i {
        0 => g.plot(&[1, 2, 3], &[1, 2]).draw(),
        1 => g.points(&[1, 2, 3], &[1, 2]).draw(),
        2 => g.points(&[1], &[1, 2, 3]).draw(),
        3 => g.lines(&[1, 2, 3, 4], &[1, 2]).draw(),
        4 => g.polygon(&[1, 2, 3], &[1, 2]).draw(),
        5 => g.text(&[1, 2, 3], &[1, 2], &["a"]).draw(),
        6 => g.points(&[0.0; 0], &[0.0; 0]).draw(),
        7 => g.lines(&[1, 2, 3], &[3, 1, 2]).draw(),
        _ => unreachable!(),
    }
}

const R_CALLS: [&str; 8] = [
    "plot(1:3, 1:2)",
    "points(1:3, 1:2)",
    "points(1, 1:3)",
    "lines(1:4, 1:2)",
    "polygon(1:3, 1:2)",
    "text(1:3, 1:2, 'a')",
    "points(numeric(0), numeric(0))",
    "lines(1:3, c(3, 1, 2))",
];

#[test]
fn unequal_xy_lengths_fail_like_r() {
    if !common::r_available() {
        return;
    }
    let script = format!(
        "pdf(NULL); plot(1:10)\n{}",
        R_CALLS
            .iter()
            .map(|e| {
                format!(
                    "cat(tryCatch({{ {e}; 'ok' }}, error = conditionMessage), '\\n', sep = '')\n"
                )
            })
            .collect::<String>()
    );
    let out = Command::new("Rscript")
        .args(["--vanilla", "-e", &script])
        .output()
        .expect("run Rscript");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let expected: Vec<String> = String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .map(str::to_string)
        .collect();
    assert_eq!(expected.len(), R_CALLS.len());
    assert!(expected.iter().any(|e| e == "ok"));
    assert!(expected.iter().any(|e| e == "'x' and 'y' lengths differ"));

    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    for (i, (r, e)) in R_CALLS.iter().zip(&expected).enumerate() {
        let mut dev = Device::new(PdfBackend::new(6, 6), Par::default());
        dev.plot(1..=10, 1..=10).draw();
        let on_device = outcome(|| call(&mut dev, i));
        // A `Figure` panics at the call, not only when it is added.
        let mut fig = Figure::new();
        fig.plot_new();
        let in_figure = outcome(|| call(&mut fig, i));
        assert_eq!(&on_device, e, "device: {r}");
        assert_eq!(&in_figure, e, "figure: {r}");
    }
    std::panic::set_hook(hook);
}
