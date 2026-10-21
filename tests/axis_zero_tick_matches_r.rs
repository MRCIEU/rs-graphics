// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks default axis tick labels for ranges that straddle zero
//! against a real R `pdf()` device: R snaps a tick within `range / (100 *
//! n)` of zero to exactly 0 (`CreateAtVector()`), so e.g. -0.2..0.4 is
//! labelled `-0.2 -0.1 0.0 0.1 ...`, not `... 2.775558e-17 ...` (which also
//! turned every label scientific and made them overlap). Compares the label
//! strings only: negative labels are laid out slightly differently in PDF
//! output because R's `pdf()` draws `-` as `/minus` (a deliberate
//! difference, see CLAUDE.md); the page is wide enough that this doesn't
//! change which labels `gap.axis` drops. Skipped if `Rscript` isn't
//! available.

mod common;

use std::process::Command;

use rs_graphics::backend::pdf::PdfBackend;
use rs_graphics::color::{Color, ColorSpec};
use rs_graphics::device::Device;
use rs_graphics::par::Par;
use rs_graphics::prelude::*;

/// The string of every `(...) Tj`, in drawing order.
fn text_strings(pdf: &str) -> Vec<String> {
    pdf.lines()
        .filter_map(|l| {
            let l = l.trim_end();
            let end = l.strip_suffix(") Tj")?;
            Some(end[end.find('(')? + 1..].to_string())
        })
        .collect()
}

type Range = (f64, f64);

/// `(xlim, ylim)` of each panel.
const RANGES: &[(Range, Range)] = &[
    ((-0.2347, 0.4427), (-0.3, 0.9)),
    ((-0.7, 0.3), (-3.0, 1.2)),
    ((-1e-3, 3e-3), (-0.15, 0.06)),
];

#[test]
fn zero_ticks_match_r() {
    if !common::r_available() {
        return;
    }
    let dir = std::env::temp_dir();
    let pdf_path = dir.join("rs_graphics_zero_tick.pdf");
    let mut script = format!(
        "pdf(file = '{}', width = 15, height = 5, compress = FALSE)\npar(mfrow = c(1, 3))\n",
        pdf_path.display()
    );
    for ((x0, x1), (y0, y1)) in RANGES {
        script.push_str(&format!(
            "plot.new(); plot.window(c({x0}, {x1}), c({y0}, {y1})); axis(1); axis(2)\n"
        ));
    }
    script.push_str("invisible(dev.off())\n");
    let script_path = dir.join("rs_graphics_zero_tick.R");
    std::fs::write(&script_path, script).unwrap();
    let out = Command::new("Rscript")
        .arg(&script_path)
        .output()
        .expect("run Rscript");
    assert!(
        out.status.success(),
        "Rscript failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let r_pdf = String::from_utf8_lossy(&std::fs::read(&pdf_path).unwrap()).into_owned();

    let mut dev = Device::new(PdfBackend::new(15.0, 5.0).with_compression(false), Par::default());
    dev.par().mfrow(1, 3).draw();
    let black = ColorSpec::Color(Color::BLACK);
    for &(xlim, ylim) in RANGES {
        dev.plot_new();
        dev.plot_window()
            .xlim(xlim.0, xlim.1)
            .ylim(ylim.0, ylim.1)
            .draw();
        dev.axis()
            .side(1)
            .with(&AxisOptions {
                col: Some(black),
                col_axis: Some(black),
                cex_axis: Some(1.0),
                font: Some(1),
                las: Some(0),
                ..AxisOptions::default()
            })
            .draw();
        dev.axis()
            .side(2)
            .with(&AxisOptions {
                col: Some(black),
                col_axis: Some(black),
                cex_axis: Some(1.0),
                font: Some(1),
                las: Some(0),
                ..AxisOptions::default()
            })
            .draw();
    }
    let ours = String::from_utf8_lossy(&dev.into_backend().finish()).into_owned();

    let (r, o) = (text_strings(&r_pdf), text_strings(&ours));
    assert!(r.iter().any(|s| s == "0.0"), "R labels: {r:?}");
    assert_eq!(o, r);
}
