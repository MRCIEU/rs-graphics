// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks `text()` placement against a real R `pdf()` device's
//! uncompressed output: single `adj` (vertical adjustment `NA`, i.e. ink
//! centring on the glyph bounding boxes), two-element `adj` (baseline
//! offset by `adj[2] * strheight("M")`), rotation, `cex` and `font`.
//! Skipped if `Rscript` isn't available.

use crate::common;

use std::process::Command;

use rs_graphics::backend::pdf::PdfBackend;
use rs_graphics::color::{Color, ColorSpec};
use rs_graphics::device::Device;
use rs_graphics::par::{Par, Sides};
use rs_graphics::prelude::*;

/// `(x, y, font size)` of every text matrix, in drawing order.
fn text_origins(pdf: &str) -> Vec<(f64, f64, f64)> {
    let mut out = Vec::new();
    let mut tf = 1.0;
    for line in pdf.lines() {
        let toks: Vec<&str> = line.split_whitespace().collect();
        for (i, t) in toks.iter().enumerate() {
            let num = |k: usize| toks[i - k].parse::<f64>().unwrap();
            if *t == "Tf" {
                tf = num(1);
            } else if *t == "Tm" {
                let (a, b) = (num(6), num(5));
                out.push((num(2), num(1), tf * a.hypot(b)));
            }
        }
    }
    out
}

/// `(label, hadj, vadj, srt, cex, font)`; `vadj = None` is a single `adj`.
const CASES: &[(&str, f64, Option<f64>, f64, f64, i32)] = &[
    ("Centre", 0.5, None, 0.0, 1.0, 1),
    ("left gyp", 0.0, None, 0.0, 1.0, 1),
    ("right 12", 1.0, None, 0.0, 1.5, 2),
    ("xxx", 0.5, None, 0.0, 1.0, 1),
    ("rotated", 0.5, None, 90.0, 1.0, 1),
    ("slanted Qj", 0.0, None, 30.0, 0.8, 3),
    ("baseline", 0.0, Some(0.0), 0.0, 1.0, 1),
    ("middle M", 0.5, Some(0.5), 0.0, 1.0, 1),
    ("top", 1.0, Some(1.0), 0.0, 1.2, 2),
    ("two adj srt", 0.3, Some(0.7), 45.0, 1.0, 1),
];

#[test]
fn text_placement_matches_r() {
    if !common::r_available() {
        return;
    }
    let dir = std::env::temp_dir();
    let pdf_path = dir.join("rs_graphics_text.pdf");
    let mut script = format!(
        "pdf(file = '{}', width = 6, height = 4, compress = FALSE)\n\
         par(mar = c(0, 0, 0, 0))\nplot.new(); plot.window(c(0, 10), c(0, 11))\n",
        pdf_path.display()
    );
    for (i, (label, h, v, srt, cex, font)) in CASES.iter().enumerate() {
        let adj = match v {
            Some(v) => format!("c({h}, {v})"),
            None => h.to_string(),
        };
        script.push_str(&format!(
            "text(5, {y}, '{label}', adj = {adj}, srt = {srt}, cex = {cex}, font = {font})\n",
            y = i + 1
        ));
    }
    script.push_str("invisible(dev.off())\n");
    let script_path = dir.join("rs_graphics_text.R");
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

    let mut par = Par::default();
    par.mar = Sides::new(0.0, 0.0, 0.0, 0.0);
    let mut dev = Device::new(PdfBackend::new(6.0, 4.0).with_compression(false), par);
    dev.plot_new();
    dev.plot_window().xlim(0.0, 10.0).ylim(0.0, 11.0).draw();
    let black = ColorSpec::Color(Color::BLACK);
    for (i, &(label, h, v, srt, cex, font)) in CASES.iter().enumerate() {
        dev.text()
            .x([5.0])
            .y([(i + 1) as f64])
            .labels([label])
            .with(&TextOptions {
                adj: Some(h),
                vadj: v,
                srt: Some(srt),
                col: vec![black],
                cex: vec![Some(cex)],
                font: vec![Some(FontFace::try_from(font).unwrap())],
                ..TextOptions::default()
            })
            .draw();
    }
    let ours = String::from_utf8_lossy(&dev.into_backend().finish()).into_owned();

    let (r, o) = (text_origins(&r_pdf), text_origins(&ours));
    assert_eq!(o.len(), r.len(), "text count\nours={o:?}\nr={r:?}");
    for ((a, b), case) in o.iter().zip(&r).zip(CASES) {
        assert!(
            (a.0 - b.0).abs() < 0.05 && (a.1 - b.1).abs() < 0.05 && (a.2 - b.2).abs() < 0.05,
            "{:?}: ours={a:?} r={b:?}",
            case.0
        );
    }
}
