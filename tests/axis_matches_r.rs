// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks the positions of axis tick labels, axis titles (`xlab`/
//! `ylab`) against the text-positioning operators in a real R `pdf()`
//! device's (uncompressed) output, for each `las` setting. Skipped if
//! `Rscript` isn't available.
//!
//! `main` is included: our PDF backend replicates R's pdf-device font-size
//! rounding (`floor(cex * ps + 0.5)`, so `cex.main = 1.2` is 14pt, not 14.4pt).

mod common;

use std::process::Command;

use rs_graphics::backend::pdf::PdfBackend;
use rs_graphics::device::Device;
use rs_graphics::graphics::Graphics;
use rs_graphics::highlevel::PlotOptions;
use rs_graphics::par::Par;

/// All `(a, b, c, d, x, y)` text matrices (`... Tm`) in an uncompressed PDF.
fn text_matrices(pdf: &str) -> Vec<[f64; 6]> {
    let mut out = Vec::new();
    for line in pdf.lines() {
        let Some(idx) = line.find(" Tm") else {
            continue;
        };
        let nums: Vec<f64> = line[..idx]
            .split_whitespace()
            .rev()
            .take(6)
            .filter_map(|t| t.parse().ok())
            .collect();
        if nums.len() == 6 {
            let mut m = [0.0; 6];
            for (i, v) in nums.iter().rev().enumerate() {
                m[i] = *v;
            }
            out.push(m);
        }
    }
    out
}

/// Sorted by position (`main` included).
fn normalise(mut v: Vec<[f64; 6]>) -> Vec<[f64; 6]> {
    v.sort_by(|a, b| {
        (a[4].round(), a[5].round())
            .partial_cmp(&(b[4].round(), b[5].round()))
            .unwrap()
    });
    v
}

fn sgn(v: f64) -> i32 {
    if v > 1e-9 {
        1
    } else if v < -1e-9 {
        -1
    } else {
        0
    }
}

#[test]
fn axis_and_label_text_positions_match_r() {
    if !common::r_available() {
        return;
    }

    let x: Vec<f64> = (1..=25).map(|i| i as f64).collect();
    let y: Vec<f64> = x.iter().map(|v| (v * 0.35).sin() * 8.0 + v * 0.6).collect();

    for las in 0..=3 {
        let path = std::env::temp_dir().join(format!("rs_graphics_axis_las{las}.pdf"));
        let script = format!(
            "pdf(file = '{}', width = 6, height = 4, compress = FALSE); \
             par(las = {las}); \
             x <- 1:25; y <- sin(x * 0.35) * 8 + x * 0.6; \
             plot(x, y, pch = 19, main = 'Title', xlab = 'x', ylab = 'y'); \
             invisible(dev.off())",
            path.display()
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
        let r_pdf = String::from_utf8_lossy(&std::fs::read(&path).unwrap()).into_owned();
        let r_tm = normalise(text_matrices(&r_pdf));

        let mut par = Par::default();
        par.las = las;
        let mut dev = Device::new(PdfBackend::new(6.0, 4.0).with_compression(false), par);
        let opts = PlotOptions {
            pch: vec![Some(19)],
            main: Some("Title".into()),
            xlab: Some("x".into()),
            ylab: Some("y".into()),
            ..PlotOptions::default()
        };
        dev.plot().x(&x).y(&y).with(&opts).draw();
        let ours = String::from_utf8_lossy(&dev.into_backend().finish()).into_owned();
        let our_tm = normalise(text_matrices(&ours));

        assert!(r_tm.len() >= 10, "las={las}: too few text items from R");
        assert_eq!(
            our_tm.len(),
            r_tm.len(),
            "las={las}: different number of text items\nours={our_tm:?}\nr={r_tm:?}"
        );
        for (a, b) in our_tm.iter().zip(r_tm.iter()) {
            // R prints the font size in the matrix, we use unit vectors,
            // so compare only the direction of the text.
            assert_eq!(
                (sgn(a[0]), sgn(a[1])),
                (sgn(b[0]), sgn(b[1])),
                "las={las}: rotation differs: {a:?} vs {b:?}"
            );
            assert!(
                (a[4] - b[4]).abs() < 0.05 && (a[5] - b[5]).abs() < 0.05,
                "las={las}: text origin differs: ours={a:?} r={b:?}"
            );
        }
    }
}
