// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks `Device::strwidth`/`strheight` against R's `strwidth()`/
//! `strheight()` on a `pdf()` device, in each of `units = "inches"`,
//! `"user"` (incl. a reversed axis) and `"figure"`: plain text,
//! `cex`/`font`, multi-line text, `par(ps)`, `par(lheight)` and the
//! `cexbase` of a 2x2 `mfrow`. Strings avoid kerning pairs that R's `pdf()` skips (the
//! `KPend` bug we don't copy). Skipped if `Rscript` isn't available.

use crate::common;

use std::process::Command;

use rs_graphics::device::Device;
use rs_graphics::graphics::Graphics;

struct Case {
    text: &'static str,
    cex: f64,
    font: i32,
    ps: f64,
    lheight: f64,
    mfrow: bool,
}

const CASES: &[Case] = &[
    Case {
        text: "axis label",
        cex: 1.0,
        font: 1,
        ps: 12.0,
        lheight: 1.0,
        mfrow: false,
    },
    Case {
        text: "bold mix",
        cex: 1.5,
        font: 2,
        ps: 12.0,
        lheight: 1.0,
        mfrow: false,
    },
    Case {
        text: "one\nlonger line\nx",
        cex: 1.0,
        font: 3,
        ps: 12.0,
        lheight: 1.0,
        mfrow: false,
    },
    Case {
        text: "small",
        cex: 0.7,
        font: 1,
        ps: 9.0,
        lheight: 1.0,
        mfrow: false,
    },
    Case {
        text: "two\nlines",
        cex: 1.2,
        font: 4,
        ps: 10.0,
        lheight: 1.0,
        mfrow: true,
    },
    Case {
        text: "three\nshort\nlines",
        cex: 1.0,
        font: 1,
        ps: 12.0,
        lheight: 2.5,
        mfrow: false,
    },
];

#[test]
fn strwidth_and_strheight_match_r() {
    if !common::r_available() {
        return;
    }
    let r_str = |s: &str| s.replace('\n', "\\n");
    let mut script = String::new();
    for c in CASES {
        script.push_str(&format!(
            "pdf(NULL, width = 7, height = 6)\n{}par(ps = {}, lheight = {}); plot.new()\n\
             plot.window(xlim = {xlim}, ylim = c(-2, 5))\n\
             u <- c('inches', 'user', 'figure')\n\
             cat(sprintf('%.10g', c(sapply(u, function(un) strwidth('{t}', un, cex = {cex}, font = {f})), \
                                    sapply(u, function(un) strheight('{t}', un, cex = {cex}, font = {f})))), '\\n')\n\
             invisible(dev.off())\n",
            if c.mfrow {
                "par(mfrow = c(2, 2)); "
            } else {
                ""
            },
            c.ps,
            c.lheight,
            t = r_str(c.text),
            cex = c.cex,
            f = c.font,
            xlim = if c.mfrow { "c(10, 0)" } else { "c(0, 10)" },
        ));
    }
    let path = std::env::temp_dir().join("rs_graphics_strwidth.R");
    std::fs::write(&path, script).unwrap();
    let out = Command::new("Rscript")
        .arg(&path)
        .output()
        .expect("run Rscript");
    assert!(
        out.status.success(),
        "Rscript failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let r = String::from_utf8_lossy(&out.stdout).into_owned();
    assert_eq!(r.lines().count(), CASES.len(), "R output: {r}");

    for (line, c) in r.lines().zip(CASES) {
        let want: Vec<f64> = line
            .split_whitespace()
            .map(|t| t.parse().unwrap())
            .collect();
        // As R's `par(ps = , lheight = )` after `pdf()`: the device keeps
        // its starting point size (12), which sizes the margins.
        let mut dev = Device::memory("pdf").width(7.0).height(6.0).open().unwrap();
        dev.par().ps(c.ps).lheight(c.lheight).draw();
        if c.mfrow {
            dev.par().mfrow(2, 2).draw();
        }
        dev.plot_new();
        let xlim = if c.mfrow { (10, 0) } else { (0, 10) };
        dev.plot_window().xlim(xlim.0, xlim.1).ylim(-2, 5).draw();
        let mut ours = Vec::new();
        for height in [false, true] {
            for units in ["inches", "user", "figure"] {
                let call = if height { dev.strheight() } else { dev.strwidth() };
                ours.push(call.s(c.text).cex(c.cex).font(c.font).units(units).get());
            }
        }
        for (k, (a, b)) in ours.iter().zip(&want).enumerate() {
            assert!(
                (a - b).abs() < 1e-6 * b.abs().max(1.0),
                "{:?} cex {} font {}, value {k} (widths in inches/user/figure, then heights): ours {a}, R {b}",
                c.text,
                c.cex,
                c.font,
            );
        }
    }
}

#[test]
#[should_panic(expected = "argument \"s\" is missing, with no default")]
fn strwidth_without_s_panics() {
    let mut dev = Device::memory("pdf").width(7.0).height(6.0).open().unwrap();
    dev.plot_new();
    dev.strwidth().cex(2).get();
}
