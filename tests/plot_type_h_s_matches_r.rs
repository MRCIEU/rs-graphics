// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks `type = "h"`, `"s"` and `"S"` (in `plot()`, `lines()` and
//! `points()`) against a real R `pdf()` device's uncompressed output:
//! `"h"` lines from y = 0 (also when 0 is below the plot), from the bottom
//! of the plot on a log y axis, with colours recycled per line and `NA`
//! colours, `NA` values and values <= 0 on a log axis skipped; steps in
//! both directions, broken at `NA`, on linear and log axes, with `lty` and
//! `lwd`. Compares every stroked path in order: its vertices, colour,
//! width and dash pattern. Skipped if `Rscript` isn't available.

mod common;

use std::process::Command;

use rs_graphics::prelude::*;

const PAGE_W: f64 = 6.0;
const PAGE_H: f64 = 5.0;

const NA: f64 = f64::NAN;
const X: [f64; 8] = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
const Y: [f64; 8] = [3.0, -2.0, 5.0, NA, 8.0, 6.0, 9.0, 4.0];
/// For log y axes: a 0 (skipped there) and a wide range.
const Y_LOG: [f64; 8] = [3.0, 0.0, 5.0, NA, 8.0, 60.0, 900.0, 4.0];

type Draw = fn(&mut Device<PdfBackend>);

struct Case {
    name: &'static str,
    /// R calls after `pdf()`, with `axes = FALSE, ann = FALSE` on `plot()`.
    r: &'static str,
    ours: Draw,
}

/// `plot(type = type_, axes = FALSE)`, limits from the data as in R.
fn plot(type_: PlotType) -> PlotOptions {
    PlotOptions::default().type_(type_).axes(false)
}

const CASES: [Case; 6] = [
    Case {
        name: "h_colours",
        r: "plot(x, y, type = 'h', col = c('red', NA, 'blue'), lwd = 3, axes = FALSE, ann = FALSE)",
        ours: |dev| {
            let opts = plot(PlotType::Histogram);
            dev.plot(&X, &Y)
                .with(&opts)
                .col(["red".into(), ColorSpec::None, "blue".into()])
                .lwd(3.0)
                .draw();
        },
    },
    Case {
        name: "h_zero_below_plot",
        r: "plot(x, y + 10, type = 'h', axes = FALSE, ann = FALSE)",
        ours: |dev| {
            let y: Vec<f64> = Y.iter().map(|v| v + 10.0).collect();
            let opts = plot(PlotType::Histogram);
            dev.plot(&X, &y).with(&opts).draw();
        },
    },
    Case {
        name: "h_log",
        r: "plot(x, ylog, type = 'h', log = 'y', axes = FALSE, ann = FALSE)",
        ours: |dev| {
            let opts = plot(PlotType::Histogram).ylog(true);
            dev.plot(&X, &Y_LOG).with(&opts).draw();
        },
    },
    Case {
        name: "s_dashed",
        r: "plot(x, y, type = 's', lty = 2, col = 'darkgreen', axes = FALSE, ann = FALSE)",
        ours: |dev| {
            let opts = plot(PlotType::Steps);
            dev.plot(&X, &Y)
                .with(&opts)
                .lty(LineType::DASHED)
                .col("darkgreen")
                .draw();
        },
    },
    Case {
        name: "S_log_x",
        r: "plot(x, y, type = 'S', log = 'x', lwd = 2, axes = FALSE, ann = FALSE)",
        ours: |dev| {
            let opts = plot(PlotType::StepsVerticalFirst).xlog(true);
            dev.plot(&X, &Y).with(&opts).lwd(2.0).draw();
        },
    },
    Case {
        name: "lines_points",
        r: "plot(x, ylog, type = 'n', log = 'y', axes = FALSE, ann = FALSE)\n\
            lines(x, ylog, type = 's', col = 'blue')\n\
            points(x, ylog, type = 'h', col = c('red', 'grey'))\n\
            lines(x, ylog, type = 'S', lwd = 2)",
        ours: |dev| {
            let opts = plot(PlotType::None).ylog(true);
            dev.plot(&X, &Y_LOG).with(&opts).draw();
            dev.lines(&X, &Y_LOG)
                .type_(PlotType::Steps)
                .col("blue")
                .draw();
            dev.points(&X, &Y_LOG)
                .type_(PlotType::Histogram)
                .col(["red", "grey"])
                .draw();
            dev.lines(&X, &Y_LOG)
                .type_(PlotType::StepsVerticalFirst)
                .lwd(2.0)
                .draw();
        },
    },
];

fn r_vector(a: &[f64]) -> String {
    let s: Vec<String> = a
        .iter()
        .map(|x| {
            if x.is_nan() {
                "NA".into()
            } else {
                x.to_string()
            }
        })
        .collect();
    format!("c({})", s.join(", "))
}

/// A stroked path: its vertices, stroke colour (ours `RG`, R's `SCN` in
/// its sRGB colour space), width and dash array.
#[derive(Debug)]
struct Stroked {
    points: Vec<f64>,
    colour: [f64; 3],
    width: f64,
    dash: Vec<f64>,
}

/// The content streams of an uncompressed PDF.
fn content(pdf: &str) -> String {
    let mut out = String::new();
    let mut rest = pdf;
    while let Some(i) = rest.find("stream\n") {
        let body = &rest[i + 7..];
        let j = body.find("endstream").unwrap();
        out.push_str(&body[..j]);
        out.push('\n');
        rest = &body[j + 9..];
    }
    out
}

fn stroked_paths(pdf: &str) -> Vec<Stroked> {
    let mut out = Vec::new();
    let mut path: Vec<f64> = Vec::new();
    let mut nums: Vec<f64> = Vec::new();
    let (mut colour, mut width, mut dash) = ([0.0; 3], 1.0, Vec::new());
    // The last `[ ... ]` array, the dash pattern if `d` follows.
    let mut array: Option<Vec<f64>> = None;
    let mut in_array = false;
    for t in content(pdf).split_whitespace() {
        if t.starts_with('[') {
            in_array = true;
            array = Some(Vec::new());
        }
        if in_array {
            let v = t.trim_start_matches('[').trim_end_matches(']');
            if let (Ok(v), Some(a)) = (v.parse::<f64>(), array.as_mut()) {
                a.push(v);
            }
            in_array = !t.ends_with(']');
            continue;
        }
        if let Ok(v) = t.parse::<f64>() {
            nums.push(v);
            continue;
        }
        match t {
            "m" | "l" if nums.len() >= 2 => path.extend(&nums[nums.len() - 2..]),
            "RG" | "SCN" if nums.len() >= 3 => colour.copy_from_slice(&nums[nums.len() - 3..]),
            "w" if !nums.is_empty() => width = nums[nums.len() - 1],
            "d" => dash = array.take().unwrap_or_default(),
            "S" => {
                out.push(Stroked {
                    points: std::mem::take(&mut path),
                    colour,
                    width,
                    dash: dash.clone(),
                });
            }
            "f" | "B" | "b" | "n" | "s" => path.clear(),
            _ => {}
        }
        nums.clear();
    }
    out
}

fn draw_ours(c: &Case) -> String {
    let mut dev = Device::new(
        PdfBackend::new(PAGE_W, PAGE_H).with_compression(false),
        Par::default(),
    );
    (c.ours)(&mut dev);
    String::from_utf8_lossy(&dev.into_backend().finish()).into_owned()
}

#[test]
fn plot_types_h_s_match_r() {
    if !common::r_available() {
        return;
    }
    let dir = std::env::temp_dir();
    let mut script = format!(
        "x <- {}\ny <- {}\nylog <- {}\n",
        r_vector(&X),
        r_vector(&Y),
        r_vector(&Y_LOG)
    );
    for c in &CASES {
        let path = dir.join(format!("rs_graphics_type_h_s_{}.pdf", c.name));
        script.push_str(&format!(
            "pdf(file = '{}', width = {PAGE_W}, height = {PAGE_H}, compress = FALSE)\n{}\ninvisible(dev.off())\n",
            path.display(),
            c.r
        ));
    }
    let script_path = dir.join("rs_graphics_type_h_s.R");
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
        let name = c.name;
        let path = dir.join(format!("rs_graphics_type_h_s_{name}.pdf"));
        let r = stroked_paths(&String::from_utf8_lossy(&std::fs::read(&path).unwrap()));
        let ours = stroked_paths(&draw_ours(c));
        assert!(!r.is_empty(), "{name}: nothing stroked by R");
        assert_eq!(
            ours.len(),
            r.len(),
            "{name}: path count\nours={ours:?}\nr={r:?}"
        );
        for (a, b) in ours.iter().zip(&r) {
            let close = |p: &[f64], q: &[f64], tol: f64| {
                p.len() == q.len() && p.iter().zip(q).all(|(u, v)| (u - v).abs() < tol)
            };
            assert!(
                close(&a.points, &b.points, 0.05),
                "{name}: vertices differ\nours={a:?}\nr={b:?}"
            );
            assert!(
                close(&a.colour, &b.colour, 0.002),
                "{name}: colour\nours={a:?}\nr={b:?}"
            );
            assert!(
                (a.width - b.width).abs() < 0.01,
                "{name}: width\nours={a:?}\nr={b:?}"
            );
            assert!(
                close(&a.dash, &b.dash, 0.01),
                "{name}: dash\nours={a:?}\nr={b:?}"
            );
        }
    }
}
