// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks string widths from the Helvetica AFM metrics against R's
//! own `strwidth()` on a `pdf()` device (which uses the same metrics by
//! default), through `strwidth()` on a null device (PDF's measurements).
//! Skipped if `Rscript` isn't available.

use crate::common;

use std::process::Command;

use rs_graphics::prelude::*;

fn r_strwidth(s: &str, pointsize: f64, cex: f64, font: i32) -> f64 {
    let script = format!(
        "pdf(file = tempfile(), pointsize = {pointsize}, family = 'Helvetica'); \
         cat(strwidth('{s}', units = 'in', cex = {cex}, font = {font})); \
         invisible(dev.off())"
    );
    let out = Command::new("Rscript")
        .arg("-e")
        .arg(&script)
        .output()
        .expect("run Rscript");
    assert!(
        out.status.success(),
        "Rscript failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().parse().unwrap()
}

#[test]
fn str_width_matches_r() {
    if !common::r_available() {
        return;
    }

    let cases: &[(&str, f64, f64, i32)] = &[
        ("Hello, World!", 12.0, 1.0, 1),
        ("Hello, World!", 12.0, 1.5, 1),
        ("The quick brown fox", 10.0, 1.0, 1),
        ("Bold Text", 14.0, 1.0, 2),
        ("Italic Text", 14.0, 1.0, 3),
        ("0123456789", 12.0, 2.0, 1),
        ("x", 12.0, 1.0, 1),
    ];

    for &(s, ps, cex, font) in cases {
        let expected = r_strwidth(s, ps, cex, font);
        let mut dev = Device::null().pointsize(ps).open().unwrap();
        let actual = dev
            .strwidth()
            .s(s)
            .cex(cex)
            .font(font)
            .units("inches")
            .get();
        // R's PostScript/PDF kern-pair lookup has an internal indexing quirk
        // (`devPS.c`'s `KPstart`/`KPend` construction silently drops the
        // last-in-file kerning pair for some first characters). We
        // intentionally don't replicate that engine bug - we apply every
        // documented kerning pair (skipping ones involving "space", which
        // *is* documented behaviour). This can very occasionally leave us
        // one kerning pair (a handful of AFM units) away from R for a given
        // string, so allow a small relative tolerance rather than an exact
        // match.
        let tol = (expected.abs() * 0.01).max(1e-6);
        assert!(
            (actual - expected).abs() < tol,
            "mismatch for {s:?} ps={ps} cex={cex} font={font}: rust={actual} r={expected}"
        );
    }
}
