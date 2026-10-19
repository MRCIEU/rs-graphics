// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks `pie()` against a real R `pdf()` device's uncompressed
//! output: every vertex of every slice polygon and label tick line, and
//! every label/title text origin, for square, wide and tall pages,
//! `clockwise`/`init.angle`/`radius`, and `par(xaxs = "i")` (which
//! exercises `plot.window(asp = 1)`'s own range widening). Skipped if
//! `Rscript` isn't available.

use std::process::Command;

use rs_graphics::backend::pdf::PdfBackend;
use rs_graphics::device::Device;
use rs_graphics::highlevel::{pie, PieOptions};
use rs_graphics::par::{AxisStyle, Par};

fn rscript_available() -> bool {
    Command::new("Rscript")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Every stroked/filled `m`/`l` path's vertices, in drawing order (clips
/// and rectangles are skipped).
fn paths(pdf: &str) -> Vec<Vec<(f64, f64)>> {
    let mut out = Vec::new();
    let mut cur: Vec<(f64, f64)> = Vec::new();
    for tok_line in pdf.lines() {
        let toks: Vec<&str> = tok_line.split_whitespace().collect();
        for (i, t) in toks.iter().enumerate() {
            let num = |k: usize| toks[i - k].parse::<f64>().unwrap();
            match *t {
                "m" | "l" => cur.push((num(2), num(1))),
                "re" | "n" => cur.clear(),
                "S" | "s" | "f" | "B" | "b" | "f*" | "B*" | "b*" => {
                    if !cur.is_empty() {
                        out.push(std::mem::take(&mut cur));
                    }
                }
                _ => {}
            }
        }
    }
    out
}

/// `(x, y)` of every text matrix, in drawing order.
fn text_origins(pdf: &str) -> Vec<(f64, f64)> {
    let mut out = Vec::new();
    for line in pdf.lines() {
        let toks: Vec<&str> = line.split_whitespace().collect();
        for (i, t) in toks.iter().enumerate() {
            if *t == "Tm" {
                out.push((toks[i - 2].parse().unwrap(), toks[i - 1].parse().unwrap()));
            }
        }
    }
    out
}

struct Case {
    name: &'static str,
    width: f64,
    height: f64,
    /// Extra `pie()` arguments in R.
    r_args: &'static str,
    exact_axes: bool,
    clockwise: bool,
    /// `None`: R's default (90 when clockwise, else 0).
    init_angle_deg: Option<f64>,
    radius: f64,
}

fn cases() -> Vec<Case> {
    let base = Case {
        name: "",
        width: 6.0,
        height: 6.0,
        r_args: "",
        exact_axes: false,
        clockwise: false,
        init_angle_deg: None,
        radius: 0.8,
    };
    vec![
        Case {
            name: "square",
            ..base
        },
        Case {
            name: "wide",
            width: 7.0,
            height: 4.0,
            ..base
        },
        Case {
            name: "tall",
            width: 4.0,
            height: 7.0,
            ..base
        },
        Case {
            name: "clockwise_radius",
            r_args: ", clockwise = TRUE, radius = 1",
            clockwise: true,
            radius: 1.0,
            ..base
        },
        Case {
            name: "init_angle_exact_axes",
            width: 6.5,
            height: 5.0,
            r_args: ", init.angle = 45",
            exact_axes: true,
            init_angle_deg: Some(45.0),
            ..base
        },
    ]
}

const VALUES: [f64; 5] = [18.0, 25.0, 12.0, 30.0, 15.0];
const LABELS: [&str; 5] = ["Alpha", "Beta", "Gamma", "Delta", "Epsilon"];

#[test]
fn pie_matches_r() {
    if !rscript_available() {
        eprintln!("Rscript not found on PATH; skipping R cross-check.");
        return;
    }
    let dir = std::env::temp_dir();
    let cases = cases();
    let mut script = String::new();
    for c in &cases {
        script.push_str(&format!(
            "pdf(file = '{}', width = {}, height = {}, compress = FALSE)\n\
             par(xaxs = '{ax}', yaxs = '{ax}')\n\
             pie(c(18, 25, 12, 30, 15), labels = c('Alpha', 'Beta', 'Gamma', 'Delta', 'Epsilon'), \
             main = 'pie()'{})\ninvisible(dev.off())\n",
            dir.join(format!("rs_graphics_pie_{}.pdf", c.name)).display(),
            c.width,
            c.height,
            c.r_args,
            ax = if c.exact_axes { "i" } else { "r" },
        ));
    }
    let script_path = dir.join("rs_graphics_pie.R");
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

    for c in &cases {
        let path = dir.join(format!("rs_graphics_pie_{}.pdf", c.name));
        let r_pdf = String::from_utf8_lossy(&std::fs::read(&path).unwrap()).into_owned();

        let mut par = Par::default();
        if c.exact_axes {
            par.xaxs = AxisStyle::Exact;
            par.yaxs = AxisStyle::Exact;
        }
        let mut dev = Device::new(PdfBackend::new(c.width, c.height).with_compression(false), par);
        let opts = PieOptions {
            labels: Some(LABELS.iter().map(|s| s.to_string()).collect()),
            main: Some("pie()".into()),
            clockwise: c.clockwise,
            init_angle_deg: c.init_angle_deg,
            radius: c.radius,
            ..PieOptions::default()
        };
        pie(&mut dev, &VALUES, &opts);
        let ours = String::from_utf8_lossy(&dev.into_backend().finish()).into_owned();

        let (r_p, o_p) = (paths(&r_pdf), paths(&ours));
        assert_eq!(o_p.len(), r_p.len(), "{}: path count", c.name);
        for (k, (a, b)) in o_p.iter().zip(&r_p).enumerate() {
            assert_eq!(a.len(), b.len(), "{}: path {k} vertex count", c.name);
            for (p, q) in a.iter().zip(b) {
                assert!(
                    (p.0 - q.0).abs() < 0.05 && (p.1 - q.1).abs() < 0.05,
                    "{}: path {k} vertex differs: ours={p:?} r={q:?}",
                    c.name
                );
            }
        }

        let (r_t, o_t) = (text_origins(&r_pdf), text_origins(&ours));
        assert_eq!(o_t.len(), r_t.len(), "{}: text count", c.name);
        for (a, b) in o_t.iter().zip(&r_t) {
            assert!(
                (a.0 - b.0).abs() < 0.05 && (a.1 - b.1).abs() < 0.05,
                "{}: text origin differs: ours={a:?} r={b:?}",
                c.name
            );
        }
    }
}
