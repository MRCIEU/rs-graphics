// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks `mtext()` placement (every side, inner and outer margins,
//! `line`, `at`, `adj`, `padj`, `cex`, `font`, and `par(las)` 0..=3) against
//! a real R `pdf()` device's uncompressed output, inside a 2x2 `mfrow` page
//! with outer margins (so the 0.83 multi-panel `cex` applies). Skipped if
//! `Rscript` isn't available.

mod common;

use std::process::Command;

use rs_graphics::backend::pdf::PdfBackend;
use rs_graphics::color::ColorSpec;
use rs_graphics::device::{Device, MtextOptions};
use rs_graphics::par::{Par, Sides};
use rs_graphics::prelude::*;

/// `(x, y, font size, rotation)` of every text matrix, in drawing order.
fn text_origins(pdf: &str) -> Vec<(f64, f64, f64, f64)> {
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
                out.push((num(2), num(1), tf * a.hypot(b), b.atan2(a).to_degrees()));
            }
        }
    }
    out
}

struct Case {
    text: &'static str,
    side: i32,
    line: f64,
    outer: bool,
    at: Option<f64>,
    adj: Option<f64>,
    padj: Option<f64>,
    cex: Option<f64>,
    font: Option<i32>,
    las: i32,
}

const fn case(text: &'static str, side: i32, line: f64) -> Case {
    Case {
        text,
        side,
        line,
        outer: false,
        at: None,
        adj: None,
        padj: None,
        cex: None,
        font: None,
        las: 0,
    }
}

fn cases() -> Vec<Case> {
    vec![
        case("bottom", 1, 0.0),
        case("left", 2, 1.5),
        case("top", 3, 0.5),
        case("right", 4, 2.0),
        Case { at: Some(3.0), ..case("at three", 1, 1.0) },
        Case { at: Some(4.5), adj: Some(0.0), ..case("left at", 2, 0.0) },
        Case { adj: Some(1.0), padj: Some(1.0), ..case("right top", 3, 0.0) },
        Case { cex: Some(1.5), font: Some(2), ..case("big bold", 3, 2.0) },
        Case { las: 1, ..case("las one", 2, 1.0) },
        Case { las: 1, ..case("las one r", 4, 1.0) },
        Case { las: 2, ..case("las two", 1, 1.0) },
        Case { las: 3, at: Some(2.0), ..case("las three", 3, 0.0) },
        Case { outer: true, ..case("outer bottom", 1, 0.5) },
        Case { outer: true, ..case("outer left", 2, 1.0) },
        Case { outer: true, cex: Some(1.4), font: Some(2), ..case("Outer Title", 3, 1.0) },
        Case { outer: true, at: Some(0.25), adj: Some(0.0), ..case("outer right", 4, 0.0) },
        Case { outer: true, line: -2.0, ..case("inside top", 3, 0.0) },
        Case { outer: true, las: 1, ..case("outer las", 2, 0.5) },
    ]
}

fn r_arg<T: std::fmt::Display>(name: &str, v: Option<T>) -> String {
    v.map(|v| format!(", {name} = {v}")).unwrap_or_default()
}

#[test]
fn mtext_placement_matches_r() {
    if !common::r_available() {
        return;
    }
    let cases = cases();
    let dir = std::env::temp_dir();
    let pdf_path = dir.join("rs_graphics_mtext.pdf");
    let mut script = format!(
        "pdf(file = '{}', width = 8, height = 6, compress = FALSE)\n\
         par(mfrow = c(2, 2), oma = c(2, 3, 4, 1.5))\n\
         plot.new(); plot.window(c(0, 10), c(0, 5))\n",
        pdf_path.display()
    );
    for c in &cases {
        script.push_str(&format!(
            "mtext('{}', side = {}, line = {}, outer = {}{}{}{}{}{}, las = {})\n",
            c.text,
            c.side,
            c.line,
            if c.outer { "TRUE" } else { "FALSE" },
            r_arg("at", c.at),
            r_arg("adj", c.adj),
            r_arg("padj", c.padj),
            r_arg("cex", c.cex),
            r_arg("font", c.font),
            c.las
        ));
    }
    // Inline `las` doesn't change `par(las)`; without it, `par(las)` applies.
    script.push_str("par(las = 1); mtext('par las', side = 2, line = 2.5)\n");
    script.push_str("invisible(dev.off())\n");
    let script_path = dir.join("rs_graphics_mtext.R");
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

    let par = Par {
        oma: Sides::new(2.0, 3.0, 4.0, 1.5),
        ..Par::default()
    };
    let mut dev = Device::new(PdfBackend::new(8.0, 6.0).with_compression(false), par);
    dev.par().mfrow(2, 2).draw();
    dev.plot_new();
    dev.plot_window().xlim(0.0, 10.0).ylim(0.0, 5.0).draw();
    for c in &cases {
        dev.mtext()
            .text(c.text)
            .side(c.side)
            .with(&MtextOptions {
                line: vec![c.line],
                outer: c.outer,
                at: c.at.into_iter().collect(),
                adj: c.adj.into_iter().collect(),
                padj: c.padj.into_iter().collect(),
                cex: c.cex.into_iter().map(Some).collect(),
                col: Vec::<ColorSpec>::new(),
                font: c
                    .font
                    .into_iter()
                    .map(|f| Some(FontFace::from(f)))
                    .collect(),
                las: Some(c.las),
            })
            .draw();
    }
    assert_eq!(dev.current_par().las, 0);
    dev.par().las(1).draw();
    dev.mtext().text("par las").side(2).line(2.5).draw();
    let ours = String::from_utf8_lossy(&dev.into_backend().finish()).into_owned();

    let (r, o) = (text_origins(&r_pdf), text_origins(&ours));
    assert_eq!(o.len(), r.len(), "text count\nours={o:?}\nr={r:?}");
    let texts: Vec<&str> = cases.iter().map(|c| c.text).chain(["par las"]).collect();
    for ((a, b), c) in o.iter().zip(&r).zip(&texts) {
        assert!(
            (a.0 - b.0).abs() < 0.05
                && (a.1 - b.1).abs() < 0.05
                && (a.2 - b.2).abs() < 0.05
                && (a.3 - b.3).abs() < 0.01,
            "{c:?}: ours={a:?} r={b:?}"
        );
    }
}
