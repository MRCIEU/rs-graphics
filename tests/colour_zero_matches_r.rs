// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks colour 0 against R, where it is the background colour
//! (`par("bg")`, `RGBpar3`): with `pdf(bg = )`, with the default
//! transparent `pdf()` background (nothing drawn), and after `par(bg = )`.
//! Compares every painting operator in order, with its fill and stroke
//! colours, to an uncompressed R `pdf()`. Skipped if `Rscript` isn't
//! available.

use std::process::Command;

use rs_graphics::backend::pdf::PdfBackend;
use rs_graphics::color::{Color, ColorSpec};
use rs_graphics::device::Device;
use rs_graphics::lty::LineType;
use rs_graphics::par::Par;
use rs_graphics::prelude::*;

fn rscript_available() -> bool {
    Command::new("Rscript")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

const PAGE_W: f64 = 5.0;
const PAGE_H: f64 = 5.0;

struct Case {
    name: &'static str,
    /// `pdf(bg = )`, if any.
    page_bg: Option<&'static str>,
    /// `par(bg = )` after `plot.new()`, if any.
    par_bg: Option<&'static str>,
}

const CASES: &[Case] = &[
    Case { name: "pdf_bg", page_bg: Some("yellow"), par_bg: None },
    Case { name: "transparent", page_bg: None, par_bg: None },
    Case { name: "par_bg", page_bg: None, par_bg: Some("lightblue") },
];

const DRAW_R: &str = "plot.window(c(0, 1), c(0, 1))\n\
    points(0.5, 0.5, col = 0, pch = 19)\n\
    rect(0.1, 0.1, 0.3, 0.3, col = 0, border = 2)\n\
    text(0.7, 0.7, 'z', col = 0)\n\
    segments(0, 1, 1, 0, col = 0, lwd = 3)";

fn draw_ours(c: &Case) -> String {
    let mut backend = PdfBackend::new(PAGE_W, PAGE_H).with_compression(false);
    if let Some(bg) = c.page_bg {
        backend = backend.with_background(Color::parse(bg).unwrap());
    }
    let mut d = Device::new(backend, Par::default());
    d.plot_new();
    if let Some(bg) = c.par_bg {
        d.par.bg = Color::parse(bg).unwrap();
    }
    d.plot_window((0.0, 1.0), (0.0, 1.0)).draw();
    let zero = ColorSpec::Index(0);
    d.points(&[0.5], &[0.5])
        .with(&XyOptions {
            pch: vec![19],
            col: vec![zero],
            ..XyOptions::default()
        })
        .draw();
    let solid = LineType::from_code(1);
    d.rect(&[0.1], &[0.1], &[0.3], &[0.3])
        .with(&RectOptions {
            col: vec![zero],
            border: vec![ColorSpec::Index(2)],
            lwd: vec![1.0],
            lty: vec![solid],
            ..RectOptions::default()
        })
        .draw();
    d.text(&[0.7], &[0.7], &["z"])
        .with(&TextOptions {
            adj: Some(0.5),
            col: vec![zero],
            cex: vec![1.0],
            font: vec![1],
            ..TextOptions::default()
        })
        .draw();
    let line = SegmentsOptions {
        col: vec![zero],
        lty: vec![solid],
        lwd: vec![3.0],
        ..SegmentsOptions::default()
    };
    d.segments(&[0.0], &[1.0], &[1.0], &[0.0])
        .with(&line)
        .draw();
    String::from_utf8_lossy(&d.into_backend().finish()).into_owned()
}

/// A painting operator with the fill and stroke colours in force.
#[derive(Debug)]
struct Paint {
    op: String,
    /// `None` for strokes, which don't use it.
    fill: Option<[f64; 3]>,
    /// `None` for text, which doesn't use it.
    stroke: Option<[f64; 3]>,
}

fn same(a: &Paint, b: &Paint) -> bool {
    let near = |x: &[f64; 3], y: &[f64; 3]| x.iter().zip(y).all(|(p, q)| (p - q).abs() < 0.002);
    let both = |x: &Option<[f64; 3]>, y: &Option<[f64; 3]>| match (x, y) {
        (Some(x), Some(y)) => near(x, y),
        (None, None) => true,
        _ => false,
    };
    a.op == b.op && both(&a.fill, &b.fill) && both(&a.stroke, &b.stroke)
}

fn paints(pdf: &str) -> Vec<Paint> {
    let mut out = Vec::new();
    let (mut fill, mut stroke) = ([0.0; 3], [0.0; 3]);
    let mut stack = Vec::new();
    let mut nums: Vec<f64> = Vec::new();
    let rgb = |n: &[f64]| -> [f64; 3] { n[n.len() - 3..].try_into().unwrap() };
    let mut rest = pdf;
    while let Some(i) = rest.find("stream\n") {
        let content = &rest[i + 7..];
        let j = content.find("endstream").unwrap_or(content.len());
        // Skip non-drawing streams (R embeds an ICC profile as hex).
        let drawing = content[..j].contains(" re") || content[..j].contains(" m");
        for t in content[..j].split_whitespace().filter(|_| drawing) {
            if let Ok(v) = t.parse::<f64>() {
                nums.push(v);
                continue;
            }
            match t {
                "q" => stack.push((fill, stroke)),
                "Q" => (fill, stroke) = stack.pop().unwrap(),
                "rg" | "scn" => fill = rgb(&nums),
                "RG" | "SCN" => stroke = rgb(&nums),
                "f" | "f*" => out.push(Paint { op: t.into(), fill: Some(fill), stroke: None }),
                "B" | "b" => out.push(Paint {
                    op: t.into(),
                    fill: Some(fill),
                    stroke: Some(stroke),
                }),
                "S" | "s" => out.push(Paint { op: t.into(), fill: None, stroke: Some(stroke) }),
                "Tj" | "TJ" => out.push(Paint { op: "Tj".into(), fill: Some(fill), stroke: None }),
                _ => {}
            }
            nums.clear();
        }
        rest = &content[j..];
    }
    out
}

#[test]
fn colour_zero_matches_r() {
    if !rscript_available() {
        eprintln!("Rscript not found on PATH; skipping R cross-check.");
        return;
    }
    let dir = std::env::temp_dir();
    let pdf = |c: &Case| dir.join(format!("rs_graphics_colour_zero_{}.pdf", c.name));
    let mut script = String::new();
    for c in CASES {
        let bg = c.page_bg.map(|b| format!(", bg = '{b}'")).unwrap_or_default();
        let par_bg = c.par_bg.map(|b| format!("par(bg = '{b}')\n")).unwrap_or_default();
        script.push_str(&format!(
            "pdf(file = '{}', width = {PAGE_W}, height = {PAGE_H}{bg}, compress = FALSE)\n\
             plot.new()\n{par_bg}{DRAW_R}\ninvisible(dev.off())\n",
            pdf(c).display()
        ));
    }
    let script_path = dir.join("rs_graphics_colour_zero.R");
    std::fs::write(&script_path, script).unwrap();
    let out = Command::new("Rscript").arg(&script_path).output().expect("run Rscript");
    assert!(out.status.success(), "Rscript failed: {}", String::from_utf8_lossy(&out.stderr));

    for c in CASES {
        let r = paints(&String::from_utf8_lossy(&std::fs::read(pdf(c)).unwrap()));
        let ours = paints(&draw_ours(c));
        assert!(!r.is_empty(), "{}: nothing from R", c.name);
        assert!(
            ours.len() == r.len() && ours.iter().zip(&r).all(|(a, b)| same(a, b)),
            "{}: painting operators and colours\nours={ours:#?}\nr={r:#?}",
            c.name
        );
    }
}
