// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks sizes R takes from the device's starting point size
//! (`pdf(pointsize = )`, R's `cra`) rather than the current `par(ps)`:
//! margin lines, plotting symbols, `type = "b"` gaps and `legend()`
//! spacing, after `par(ps)` is changed. Compares every stroked line, the
//! bounding box of every curved path (the points) and every label (string
//! and origin) with an uncompressed R `pdf()`. Skipped if `Rscript` isn't
//! available.

mod common;

use std::process::Command;

use rs_graphics::backend::pdf::PdfBackend;
use rs_graphics::color::ColorSpec;
use rs_graphics::device::{Device, LegendKeyword, LegendPos};
use rs_graphics::highlevel::{PlotOptions, PlotType};
use rs_graphics::lty::LineType;
use rs_graphics::par::Par;
use rs_graphics::graphics::Graphics;

const PAGE_W: f64 = 7.0;
const PAGE_H: f64 = 6.0;

/// `(device pointsize, par(ps) set afterwards)`.
const CASES: &[(f64, f64)] = &[(12.0, 20.0), (9.0, 14.0), (14.0, 8.0)];

fn r_script(start: f64, ps: f64, path: &str) -> String {
    format!(
        "pdf(file = '{path}', width = {PAGE_W}, height = {PAGE_H}, pointsize = {start}, \
             compress = FALSE)\n\
         par(ps = {ps})\n\
         plot(c(1, 2, 3, 4), c(1, 3, 2, 4), type = 'b', xlab = 'x', ylab = 'y')\n\
         legend('topleft', legend = c('a', 'bb'), pch = 1, lty = 1)\n\
         invisible(dev.off())\n"
    )
}

fn draw_ours(start: f64, ps: f64) -> String {
    let par = Par { ps: start, ..Par::default() };
    let mut dev = Device::new(PdfBackend::new(PAGE_W, PAGE_H).with_compression(false), par);
    dev.par().ps(ps).draw();
    let opts = PlotOptions {
        type_: Some(PlotType::Both),
        xlab: Some("x".into()),
        ylab: Some("y".into()),
        ..PlotOptions::default()
    };
    dev.plot()
        .x([1.0, 2.0, 3.0, 4.0])
        .y([1.0, 3.0, 2.0, 4.0])
        .with(&opts)
        .draw();
    let black = ColorSpec::Index(1);
    dev.legend()
        .pos(LegendPos::Keyword(LegendKeyword::TopLeft))
        .labels(["a", "bb"])
        .col(black)
        .pch(1)
        .lty(LineType::from_code(1))
        .draw();
    String::from_utf8_lossy(&dev.into_backend().finish()).into_owned()
}

#[derive(Debug, Default)]
struct Extracted {
    /// Vertices of stroked paths without curves (rectangles as corners).
    lines: Vec<Vec<f64>>,
    /// Bounding boxes (all points, incl. control points) of curved paths.
    curves: Vec<[f64; 4]>,
    labels: Vec<(String, f64, f64)>,
}

fn extract(pdf: &str) -> Extracted {
    let mut out = Extracted::default();
    let mut path: Vec<f64> = Vec::new();
    let mut curved = false;
    let mut nums: Vec<f64> = Vec::new();
    let mut origin: Option<(f64, f64)> = None;
    for line in pdf.lines() {
        if let Some(i) = line.find(" Tm") {
            let m: Vec<f64> = line[..i]
                .split_whitespace()
                .rev()
                .take(2)
                .filter_map(|t| t.parse().ok())
                .collect();
            origin = (m.len() == 2).then(|| (m[1], m[0]));
        }
        if let (Some((x, y)), Some(a), Some(b)) = (origin, line.find('('), line.rfind(')')) {
            if line.contains("Tj") || line.contains("TJ") {
                let text = shown_text(&line[a..=b]);
                out.labels.push((text, x, y));
                origin = None;
                continue;
            }
        }
        for t in line.split_whitespace() {
            if let Ok(v) = t.parse::<f64>() {
                nums.push(v);
                continue;
            }
            match t {
                "m" | "l" if nums.len() >= 2 => path.extend(&nums[nums.len() - 2..]),
                "c" if nums.len() >= 6 => {
                    path.extend(&nums[nums.len() - 6..]);
                    curved = true;
                }
                "S" | "s" | "f" | "B" | "b" => {
                    if curved {
                        let xs = path.iter().step_by(2);
                        let ys = path.iter().skip(1).step_by(2);
                        let (min, max) = (f64::INFINITY, f64::NEG_INFINITY);
                        out.curves.push([
                            xs.clone().fold(min, |a, &b| a.min(b)),
                            xs.fold(max, |a, &b| a.max(b)),
                            ys.clone().fold(min, |a, &b| a.min(b)),
                            ys.fold(max, |a, &b| a.max(b)),
                        ]);
                    } else if t == "S" {
                        out.lines.push(corners_sorted(&path));
                    }
                    path.clear();
                    curved = false;
                }
                // A rectangle as its corners, like R's polyline `box()`.
                "re" if nums.len() >= 4 => {
                    let [x, y, w, h] = nums[nums.len() - 4..] else { unreachable!() };
                    path.extend([x, y, x + w, y, x + w, y + h, x, y + h]);
                }
                "n" => {
                    path.clear();
                    curved = false;
                }
                _ => {}
            }
            nums.clear();
        }
    }
    out
}

/// The text shown by a `Tj` or `TJ` operand: every `(...)` string joined,
/// as R kerns inside strings (`[(b) 10 (b)] TJ`).
fn shown_text(operand: &str) -> String {
    operand
        .split('(')
        .skip(1)
        .filter_map(|part| part.split_once(')').map(|(t, _)| t))
        .collect()
}

/// Four-point paths (rectangles) as sorted corners: R draws `box()` and
/// `rect()` from different starting corners.
fn corners_sorted(path: &[f64]) -> Vec<f64> {
    if path.len() != 8 {
        return path.to_vec();
    }
    let mut pts: Vec<[f64; 2]> = path.chunks(2).map(|p| [p[0], p[1]]).collect();
    pts.sort_by(|a, b| a.partial_cmp(b).unwrap());
    pts.concat()
}

fn close(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 0.05)
}

#[test]
fn start_ps_sizes_match_r() {
    if !common::r_available() {
        return;
    }
    let dir = std::env::temp_dir();
    let pdf = |i: usize| dir.join(format!("rs_graphics_start_ps_{i}.pdf"));
    let mut script = String::new();
    for (i, &(start, ps)) in CASES.iter().enumerate() {
        script.push_str(&r_script(start, ps, &pdf(i).display().to_string()));
    }
    let script_path = dir.join("rs_graphics_start_ps.R");
    std::fs::write(&script_path, script).unwrap();
    let out = Command::new("Rscript").arg(&script_path).output().expect("run Rscript");
    assert!(out.status.success(), "Rscript failed: {}", String::from_utf8_lossy(&out.stderr));

    for (i, &(start, ps)) in CASES.iter().enumerate() {
        let r = extract(&String::from_utf8_lossy(&std::fs::read(pdf(i)).unwrap()));
        let ours = extract(&draw_ours(start, ps));
        let name = format!("pointsize {start}, ps {ps}");

        assert!(!r.lines.is_empty() && !r.curves.is_empty(), "{name}: nothing from R");
        assert_eq!(ours.lines.len(), r.lines.len(), "{name}: line count\nours={ours:#?}\nr={r:#?}");
        for (a, b) in ours.lines.iter().zip(&r.lines) {
            assert!(close(a, b), "{name}: line differs: ours={a:?} r={b:?}");
        }
        assert_eq!(ours.curves.len(), r.curves.len(), "{name}: point count");
        for (a, b) in ours.curves.iter().zip(&r.curves) {
            assert!(close(a, b), "{name}: point differs: ours={a:?} r={b:?}");
        }
        let strings = |l: &[(String, f64, f64)]| l.iter().map(|t| t.0.clone()).collect::<Vec<_>>();
        assert_eq!(strings(&ours.labels), strings(&r.labels), "{name}: labels");
        for (a, b) in ours.labels.iter().zip(&r.labels) {
            assert!(close(&[a.1, a.2], &[b.1, b.2]), "{name}: label origin: ours={a:?} r={b:?}");
        }
    }
}
