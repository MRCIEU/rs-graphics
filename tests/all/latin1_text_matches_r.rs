// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks non-ASCII (Latin-1) text against a real R `pdf()` device's
//! uncompressed output: the bytes written (R encodes e.g. `±` as `\261`)
//! (R writes them raw, we as octal escapes; both decode to the same bytes)
//! and the text origins of left-, centre- and right-justified strings, which
//! depend on the glyphs' AFM widths (looked up by Latin-1 glyph name, not by
//! the AFM's Standard Encoding codes, where 177 is `endash`). Skipped if
//! `Rscript` isn't available.

use crate::common;

use std::process::Command;

use rs_graphics::backend::pdf::PdfBackend;
use rs_graphics::color::{Color, ColorSpec};
use rs_graphics::device::Device;
use rs_graphics::par::{Par, Sides};
use rs_graphics::prelude::*;

/// `(x, y, string bytes)` of every text-showing operation, in drawing order.
/// Handles both `(...) Tj` and R's kerned `[(...) n (...)] TJ` (whose string
/// pieces are concatenated), with raw or escaped bytes.
fn texts(pdf: &[u8]) -> Vec<(f64, f64, Vec<u8>)> {
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(tm) = find(pdf, b" Tm", i) {
        // The six Tm operands precede it on the same line.
        let line_start = pdf[..tm].iter().rposition(|&b| b == b'\n').map_or(0, |p| p + 1);
        let operands: Vec<f64> = String::from_utf8_lossy(&pdf[line_start..tm])
            .split_whitespace()
            .filter_map(|t| t.parse().ok())
            .collect();
        let (x, y) = (operands[operands.len() - 2], operands[operands.len() - 1]);
        let op_end = [find(pdf, b"Tj", tm), find(pdf, b"TJ", tm)]
            .into_iter()
            .flatten()
            .min()
            .unwrap();
        out.push((x, y, string_bytes(&pdf[tm + 3..op_end])));
        i = op_end;
    }
    out
}

fn find(hay: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    hay[from..]
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| p + from)
}

/// The bytes of every `(...)` string literal in `ops`, decoding escapes.
fn string_bytes(ops: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut in_string = false;
    let mut i = 0;
    while i < ops.len() {
        let b = ops[i];
        if !in_string {
            in_string = b == b'(';
        } else if b == b')' {
            in_string = false;
        } else if b == b'\\' {
            if ops[i + 1].is_ascii_digit() {
                let oct = std::str::from_utf8(&ops[i + 1..i + 4]).unwrap();
                out.push(u8::from_str_radix(oct, 8).unwrap());
                i += 3;
            } else {
                out.push(ops[i + 1]);
                i += 1;
            }
        } else {
            out.push(b);
        }
        i += 1;
    }
    out
}

const LABELS: &[&str] = &["beta ± 1SE", "café crème", "Größe ×2", "© 1999 ¼ ½ ¾", "ÀÉÎÕÜ àéîõü"];

#[test]
fn latin1_text_matches_r() {
    if !common::r_available() {
        return;
    }
    let dir = std::env::temp_dir();
    let pdf_path = dir.join("rs_graphics_latin1.pdf");
    let mut script = format!(
        "pdf(file = '{}', width = 6, height = 6, compress = FALSE)\n\
         par(mar = c(0, 0, 0, 0))\nplot.new(); plot.window(c(0, 10), c(0, 16))\n",
        pdf_path.display()
    );
    for (i, label) in LABELS.iter().enumerate() {
        for (j, adj) in [0.0, 0.5, 1.0].iter().enumerate() {
            script.push_str(&format!(
                "text(5, {}, '{label}', adj = c({adj}, 0), font = {})\n",
                3 * i + j + 1,
                1 + (i % 2)
            ));
        }
    }
    script.push_str("invisible(dev.off())\n");
    let script_path = dir.join("rs_graphics_latin1.R");
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
    let r_pdf = std::fs::read(&pdf_path).unwrap();

    let par = Par {
        mar: Sides::new(0.0, 0.0, 0.0, 0.0),
        ..Par::default()
    };
    let mut dev = Device::new(PdfBackend::new(6.0, 6.0).with_compression(false), par);
    dev.plot_new();
    dev.plot_window().xlim(0.0, 10.0).ylim(0.0, 16.0).draw();
    let black = ColorSpec::Color(Color::BLACK);
    for (i, label) in LABELS.iter().enumerate() {
        for (j, adj) in [0.0, 0.5, 1.0].iter().enumerate() {
            let y = (3 * i + j + 1) as f64;
            dev.text()
                .x([5.0])
                .y([y])
                .labels([label])
                .with(&TextOptions {
                    adj: Some(*adj),
                    vadj: Some(0.0),
                    col: vec![black],
                    cex: vec![Some(1.0)],
                    font: vec![Some(FontFace::from(1 + (i % 2) as i32))],
                    ..TextOptions::default()
                })
                .draw();
        }
    }
    let ours = dev.into_backend().finish();

    let (r, o) = (texts(&r_pdf), texts(&ours));
    assert_eq!(o.len(), r.len());
    for (a, b) in o.iter().zip(&r) {
        let label = String::from_utf8_lossy(&b.2).into_owned();
        assert_eq!(a.2, b.2, "bytes of {label:?}");
        assert!(
            (a.0 - b.0).abs() < 0.05 && (a.1 - b.1).abs() < 0.05,
            "{label:?}: ours=({}, {}) r=({}, {})",
            a.0,
            a.1,
            b.0,
            b.1
        );
    }
}
