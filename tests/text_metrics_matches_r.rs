// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks `Device::text_metrics` against R's `strwidth()`/
//! `strheight()` (`units = "inches"`) on a `pdf()` device: plain text,
//! `cex`/`font`, multi-line text, `par(ps)`, `par(lheight)` and the
//! `cexbase` of a 2x2 `mfrow`. Strings avoid kerning pairs that R's `pdf()` skips (the
//! `KPend` bug we don't copy). Skipped if `Rscript` isn't available.

use std::process::Command;

use rs_graphics::backend::pdf::PdfBackend;
use rs_graphics::device::Device;
use rs_graphics::graphics::Graphics;
use rs_graphics::par::Par;

fn rscript_available() -> bool {
    Command::new("Rscript")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

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
fn text_metrics_match_r() {
    if !rscript_available() {
        eprintln!("Rscript not found on PATH; skipping R cross-check.");
        return;
    }
    let r_str = |s: &str| s.replace('\n', "\\n");
    let mut script = String::new();
    for c in CASES {
        script.push_str(&format!(
            "pdf(NULL, width = 7, height = 6)\n{}par(ps = {}, lheight = {}); plot.new()\n\
             cat(sprintf('%.10g', c(strwidth('{t}', 'inches', cex = {cex}, font = {f}), \
                                    strheight('{t}', 'inches', cex = {cex}, font = {f}))), '\\n')\n\
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
        ));
    }
    let path = std::env::temp_dir().join("rs_graphics_text_metrics.R");
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
        let par = Par {
            ps: c.ps,
            lheight: c.lheight,
            ..Par::default()
        };
        let mut dev = Device::new(PdfBackend::new(7.0, 6.0), par);
        if c.mfrow {
            dev.mfrow(2, 2);
        }
        dev.plot_new();
        let (w, h) = dev.text_metrics(c.text, c.font, c.cex);
        assert!(
            (w - want[0]).abs() < 1e-6 && (h - want[1]).abs() < 1e-6,
            "{:?} cex {} font {}: ours ({w}, {h}), R ({}, {})",
            c.text,
            c.cex,
            c.font,
            want[0],
            want[1]
        );
    }
}
