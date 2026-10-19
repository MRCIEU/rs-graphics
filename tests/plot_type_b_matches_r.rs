// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks `plot(type = "b")` and `plot(type = "c")` against a real R
//! `pdf()` device's uncompressed output: the gapped line segments between
//! points (each shortened by half a character height at both ends, short
//! ones left out, broken at `NA`), with `par(cex)` and a log axis. Compares
//! every stroked two-point path in order and the number of points (none
//! for `"c"`). Skipped if
//! `Rscript` isn't available.

use std::process::Command;

use rs_graphics::backend::pdf::PdfBackend;
use rs_graphics::device::Device;
use rs_graphics::highlevel::{PlotOptions, PlotType};
use rs_graphics::par::Par;
use rs_graphics::graphics::Graphics;

fn rscript_available() -> bool {
    Command::new("Rscript")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

const PAGE_W: f64 = 6.0;
const PAGE_H: f64 = 5.0;

const X: [f64; 9] = [1.0, 2.0, 2.1, 4.0, 5.0, f64::NAN, 7.0, 8.0, 10.0];
const Y: [f64; 9] = [1.0, 3.0, 3.1, 2.0, 8.0, 5.0, 6.0, 30.0, 12.0];

struct Case {
    name: &'static str,
    /// `"b"` or `"c"` (the same lines, no points).
    type_: &'static str,
    cex: f64,
    ylog: bool,
}

const CASES: [Case; 3] = [
    Case {
        name: "plain",
        type_: "b",
        cex: 1.0,
        ylog: false,
    },
    Case {
        name: "cex_log",
        type_: "b",
        cex: 1.5,
        ylog: true,
    },
    Case {
        name: "type_c",
        type_: "c",
        cex: 1.0,
        ylog: false,
    },
];

fn r_script(c: &Case, path: &str) -> String {
    let v = |a: &[f64]| {
        let s: Vec<String> = a
            .iter()
            .map(|x| if x.is_nan() { "NA".into() } else { x.to_string() })
            .collect();
        format!("c({})", s.join(", "))
    };
    format!(
        "pdf(file = '{path}', width = {PAGE_W}, height = {PAGE_H}, compress = FALSE)\n\
         par(cex = {cex})\n\
         plot({x}, {y}, type = '{t}', log = '{log}', axes = FALSE, ann = FALSE)\n\
         invisible(dev.off())\n",
        cex = c.cex,
        t = c.type_,
        x = v(&X),
        y = v(&Y),
        log = if c.ylog { "y" } else { "" },
    )
}

fn draw_ours(c: &Case) -> String {
    let par = Par {
        cex: c.cex,
        ..Par::default()
    };
    let mut dev = Device::new(PdfBackend::new(PAGE_W, PAGE_H).with_compression(false), par);
    let opts = PlotOptions {
        type_: Some(PlotType::parse(c.type_).unwrap()),
        ylog: c.ylog,
        axes: false,
        ..PlotOptions::default()
    };
    dev.plot(&X, &Y).with(&opts).draw();
    String::from_utf8_lossy(&dev.into_backend().finish()).into_owned()
}

/// Every stroked two-point path, in order, and the number of curved paths.
fn extract(pdf: &str) -> (Vec<Vec<f64>>, usize) {
    let mut segments = Vec::new();
    let mut curves = 0;
    let mut path: Vec<f64> = Vec::new();
    let mut curved = false;
    let mut nums: Vec<f64> = Vec::new();
    for t in pdf.split_whitespace() {
        if let Ok(v) = t.parse::<f64>() {
            nums.push(v);
            continue;
        }
        match t {
            "m" | "l" if nums.len() >= 2 => path.extend(&nums[nums.len() - 2..]),
            "c" => curved = true,
            "S" | "s" | "f" | "B" | "b" | "n" => {
                if curved && t != "n" {
                    curves += 1;
                } else if t == "S" && path.len() == 4 {
                    segments.push(path.clone());
                }
                path.clear();
                curved = false;
            }
            _ => {}
        }
        nums.clear();
    }
    (segments, curves)
}

#[test]
fn type_b_matches_r() {
    if !rscript_available() {
        eprintln!("Rscript not found on PATH; skipping R cross-check.");
        return;
    }
    let dir = std::env::temp_dir();
    let mut script = String::new();
    for c in &CASES {
        let path = dir.join(format!("rs_graphics_type_b_{}.pdf", c.name));
        script.push_str(&r_script(c, &path.display().to_string()));
    }
    let script_path = dir.join("rs_graphics_type_b.R");
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

    for c in &CASES {
        let path = dir.join(format!("rs_graphics_type_b_{}.pdf", c.name));
        let (r_segs, r_curves) = extract(&String::from_utf8_lossy(&std::fs::read(&path).unwrap()));
        let (our_segs, our_curves) = extract(&draw_ours(c));
        let name = c.name;
        assert_eq!(our_curves, r_curves, "{name}: points drawn");
        assert!(!r_segs.is_empty(), "{name}: no segments from R");
        assert_eq!(r_curves == 0, c.type_ == "c", "{name}: points only for type b");
        assert_eq!(
            our_segs.len(),
            r_segs.len(),
            "{name}: segment count\nours={our_segs:?}\nr={r_segs:?}"
        );
        for (a, b) in our_segs.iter().zip(&r_segs) {
            assert!(
                a.iter().zip(b).all(|(p, q)| (p - q).abs() < 0.05),
                "{name}: segment differs: ours={a:?} r={b:?}"
            );
        }
    }
}
