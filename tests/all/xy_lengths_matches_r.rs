// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! `plot()`, `points()`, `lines()` and `polygon()` return R's
//! `xy.coords()` error from `try_draw()` when `x` and `y` differ in length
//! (on a device and in a `Figure`), as R errors, while `text()` recycles
//! them, as R does. Skipped if `Rscript` isn't available.

use crate::common;

use std::process::Command;

use rs_graphics::prelude::*;

/// The calls, generic over the target so a device and a `Figure` run the
/// same code: `"ok"`, or the error's message (its call checked too).
fn call<G: Graphics>(g: &mut G, i: usize) -> String {
    let (name, result) = match i {
        0 => ("plot", g.plot().x([1, 2, 3]).y([1, 2]).try_draw()),
        1 => ("points", g.points().x([1, 2, 3]).y([1, 2]).try_draw()),
        2 => ("points", g.points().x([1]).y([1, 2, 3]).try_draw()),
        3 => ("lines", g.lines().x([1, 2, 3, 4]).y([1, 2]).try_draw()),
        4 => ("polygon", g.polygon().x([1, 2, 3]).y([1, 2]).try_draw()),
        5 => (
            "text",
            g.text().x([1, 2, 3]).y([1, 2]).labels(["a"]).try_draw(),
        ),
        6 => ("points", g.points().x([0.0; 0]).y([0.0; 0]).try_draw()),
        7 => ("lines", g.lines().x([1, 2, 3]).y([3, 1, 2]).try_draw()),
        _ => unreachable!(),
    };
    match result {
        Ok(()) => "ok".to_string(),
        Err(e) => {
            assert_eq!(e.call(), name);
            e.message().to_string()
        }
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

    for (i, (r, e)) in R_CALLS.iter().zip(&expected).enumerate() {
        let mut dev = Device::memory("pdf").width(6).height(6).open().unwrap();
        dev.plot().x(1..=10).y(1..=10).draw();
        let on_device = call(&mut dev, i);
        // A `Figure` fails at the call, not only when it is added.
        let mut fig = Figure::new();
        fig.plot_new();
        let in_figure = call(&mut fig, i);
        assert_eq!(&on_device, e, "device: {r}");
        assert_eq!(&in_figure, e, "figure: {r}");
    }
}
