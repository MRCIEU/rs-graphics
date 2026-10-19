// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks `par(new = TRUE)` against a real R `pdf()` device's
//! uncompressed output: a second y axis overlaid on a plot, overlays in a
//! multi-figure page, `plot.new()` not resetting it, drawing and outer
//! `mtext()` (not) resetting it, and setting it before anything is
//! plotted. Compares every text origin and every data line's vertices
//! (paths of 5 or more points). Skipped if `Rscript` isn't available.

use std::process::Command;

use rs_graphics::backend::pdf::PdfBackend;
use rs_graphics::color::{Color, ColorSpec};
use rs_graphics::device::{AxisOptions, Device, MtextOptions};
use rs_graphics::highlevel::{PlotOptions, PlotType};
use rs_graphics::par::{Par, Sides};
use rs_graphics::prelude::*;

fn rscript_available() -> bool {
    Command::new("Rscript")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

const PAGE_W: f64 = 8.0;
const PAGE_H: f64 = 6.0;

type Dev = Device<PdfBackend>;

fn xs() -> Vec<f64> {
    (1..=10).map(|i| i as f64).collect()
}

/// R: `pl(y, main, ...)` draws `plot(x, y, type = "l", ...)` with the
/// given title and labels "x"/"y" (or none, for `axes = FALSE`).
const R_PRELUDE: &str = "x <- 1:10\n\
    pl <- function(y, main = '', axes = TRUE, ...) {\n\
      lab <- if (axes) c('x', 'y') else c('', '')\n\
      plot(x, y, type = 'l', main = main, axes = axes, xlab = lab[1], ylab = lab[2], ...)\n\
    }\n";

/// Ours: as `pl()` above.
fn pl(dev: &mut Dev, y: &[f64], main: Option<&str>, axes: bool) {
    let opts = PlotOptions {
        type_: PlotType::Lines,
        main: main.map(Into::into),
        axes,
        xlab: axes.then(|| "x".into()),
        ylab: axes.then(|| "y".into()),
        ..PlotOptions::default()
    };
    dev.plot(&xs(), y, &opts);
}

fn squares() -> Vec<f64> {
    xs().iter().map(|v| v * v).collect()
}

fn roots() -> Vec<f64> {
    xs().iter().map(|v| v.sqrt()).collect()
}

fn falling() -> Vec<f64> {
    xs().iter().map(|v| 11.0 - v).collect()
}

struct Case {
    name: &'static str,
    r: &'static str,
    setup: fn(&mut Par),
    draw: fn(&mut Dev),
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "second_axis",
            r: "par(mar = c(5, 4, 4, 5)); pl(x^2, 'Two scales')\n\
                par(new = TRUE); pl(sqrt(x), axes = FALSE, col = 'red')\n\
                axis(4); mtext('sqrt', side = 4, line = 3)",
            setup: |p| p.mar = Sides::new(5.0, 4.0, 4.0, 5.0),
            draw: |d| {
                pl(d, &squares(), Some("Two scales"), true);
                d.set_new(true);
                let opts = PlotOptions {
                    type_: PlotType::Lines,
                    axes: false,
                    col: vec![ColorSpec::Color(Color::parse("red").unwrap())],
                    ..PlotOptions::default()
                };
                d.plot(&xs(), &roots(), &opts);
                d.axis(4, &AxisOptions::default());
                let o = MtextOptions {
                    line: 3.0,
                    ..Default::default()
                };
                d.mtext("sqrt", 4, &o);
            },
        },
        Case {
            name: "mfrow_overlay",
            r: "par(mfrow = c(2, 2)); pl(x^2, 'A')\n\
                par(new = TRUE); pl(rev(x)^2, axes = FALSE)\n\
                pl(x, 'B'); pl(sqrt(x), 'C')",
            setup: |_| {},
            draw: |d| {
                d.mfrow(2, 2);
                pl(d, &squares(), Some("A"), true);
                d.set_new(true);
                let rev: Vec<f64> = squares().into_iter().rev().collect();
                pl(d, &rev, None, false);
                pl(d, &xs(), Some("B"), true);
                pl(d, &roots(), Some("C"), true);
            },
        },
        Case {
            name: "plot_new_keeps_it",
            r: "par(mfrow = c(1, 2)); pl(x^2, 'A')\n\
                par(new = TRUE); plot.new(); plot.new()\n\
                plot.window(c(0, 20), c(0, 1)); axis(3)\n\
                pl(x, 'B')",
            setup: |_| {},
            draw: |d| {
                d.mfrow(1, 2);
                pl(d, &squares(), Some("A"), true);
                d.set_new(true);
                d.plot_new();
                d.plot_new();
                d.plot_window((0.0, 20.0), (0.0, 1.0), &PlotWindowOptions::default());
                d.axis(3, &AxisOptions::default());
                pl(d, &xs(), Some("B"), true);
            },
        },
        Case {
            name: "drawing_resets_it",
            r: "par(mfrow = c(1, 2)); pl(x^2, 'A')\n\
                par(new = TRUE); points(5, 50, pch = 19)\n\
                pl(falling(), 'B')",
            setup: |_| {},
            draw: |d| {
                d.mfrow(1, 2);
                pl(d, &squares(), Some("A"), true);
                d.set_new(true);
                let black = ColorSpec::Index(1);
                d.points(
                    &[5.0],
                    &[50.0],
                    &XyOptions {
                        pch: vec![19],
                        col: vec![black],
                        ..XyOptions::default()
                    },
                );
                assert!(!d.is_new());
                pl(d, &falling(), Some("B"), true);
            },
        },
        Case {
            name: "outer_mtext_keeps_it",
            r: "par(mfrow = c(1, 2), oma = c(0, 0, 2, 0)); pl(x^2, 'A')\n\
                par(new = TRUE); mtext('Outer', side = 3, outer = TRUE)\n\
                pl(falling(), axes = FALSE); pl(x, 'B')",
            setup: |p| p.oma = Sides::new(0.0, 0.0, 2.0, 0.0),
            draw: |d| {
                d.mfrow(1, 2);
                pl(d, &squares(), Some("A"), true);
                d.set_new(true);
                let o = MtextOptions {
                    outer: true,
                    ..Default::default()
                };
                d.mtext("Outer", 3, &o);
                assert!(d.is_new());
                pl(d, &falling(), None, false);
                pl(d, &xs(), Some("B"), true);
            },
        },
        Case {
            name: "ignored_before_plotting",
            r: "par(mfrow = c(1, 2)); suppressWarnings(par(new = TRUE))\n\
                pl(x^2, 'A'); pl(x, 'B')",
            setup: |_| {},
            draw: |d| {
                d.mfrow(1, 2);
                d.set_new(true);
                assert!(!d.is_new());
                pl(d, &squares(), Some("A"), true);
                pl(d, &xs(), Some("B"), true);
            },
        },
    ]
}

/// `(x, y)` of every text matrix, and the vertices of every stroked path
/// with at least 5 points (the data lines), each sorted.
fn extract(pdf: &str) -> (Vec<(f64, f64)>, Vec<Vec<f64>>) {
    let mut texts = Vec::new();
    let mut lines = Vec::new();
    let mut path: Vec<f64> = Vec::new();
    let mut nums: Vec<f64> = Vec::new();
    for t in pdf.split_whitespace() {
        if let Ok(v) = t.parse::<f64>() {
            nums.push(v);
            continue;
        }
        match t {
            "m" | "l" if nums.len() >= 2 => path.extend(&nums[nums.len() - 2..]),
            "Tm" if nums.len() >= 6 => texts.push((nums[nums.len() - 2], nums[nums.len() - 1])),
            "S" => {
                if path.len() >= 10 {
                    lines.push(std::mem::take(&mut path));
                }
                path.clear();
            }
            "n" | "f" | "B" | "b" | "s" | "h" | "re" => path.clear(),
            _ => {}
        }
        nums.clear();
    }
    let key = |p: &(f64, f64)| ((p.0 * 10.0).round() as i64, (p.1 * 10.0).round() as i64);
    texts.sort_by_key(key);
    lines.sort_by(|a, b| a.partial_cmp(b).unwrap());
    (texts, lines)
}

fn r_script(c: &Case, path: &str) -> String {
    format!(
        "pdf(file = '{path}', width = {PAGE_W}, height = {PAGE_H}, compress = FALSE)\n\
         falling <- function() 11 - x\n{}\ninvisible(dev.off())\n",
        c.r
    )
}

#[test]
fn par_new_matches_r() {
    if !rscript_available() {
        eprintln!("Rscript not found on PATH; skipping R cross-check.");
        return;
    }
    let dir = std::env::temp_dir();
    let cases = cases();
    let mut script = R_PRELUDE.to_string();
    for c in &cases {
        let path = dir.join(format!("rs_graphics_par_new_{}.pdf", c.name));
        script.push_str(&r_script(c, &path.display().to_string()));
    }
    let script_path = dir.join("rs_graphics_par_new.R");
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
        let path = dir.join(format!("rs_graphics_par_new_{}.pdf", c.name));
        let r_pdf = String::from_utf8_lossy(&std::fs::read(&path).unwrap()).into_owned();
        let (r_texts, r_lines) = extract(&r_pdf);

        let mut par = Par::default();
        (c.setup)(&mut par);
        let mut dev = Device::new(PdfBackend::new(PAGE_W, PAGE_H).with_compression(false), par);
        (c.draw)(&mut dev);
        let ours = String::from_utf8_lossy(&dev.into_backend().finish()).into_owned();
        let (our_texts, our_lines) = extract(&ours);
        let name = c.name;

        assert!(!r_texts.is_empty() && !r_lines.is_empty(), "{name}: nothing from R");
        assert_eq!(
            our_texts.len(),
            r_texts.len(),
            "{name}: text count\nours={our_texts:?}\nr={r_texts:?}"
        );
        for (a, b) in our_texts.iter().zip(&r_texts) {
            assert!(
                (a.0 - b.0).abs() < 0.05 && (a.1 - b.1).abs() < 0.05,
                "{name}: text origin differs: ours={a:?} r={b:?}\nours={our_texts:?}\nr={r_texts:?}"
            );
        }
        assert_eq!(our_lines.len(), r_lines.len(), "{name}: data line count");
        for (a, b) in our_lines.iter().zip(&r_lines) {
            assert!(
                a.len() == b.len() && a.iter().zip(b).all(|(p, q)| (p - q).abs() < 0.05),
                "{name}: data line differs:\n ours={a:?}\n r=   {b:?}"
            );
        }
    }
}
