// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks `par(pty = "s")`, `par(plt = )` and `par(fig = )` against a
//! real R `pdf()` device's uncompressed output: every painted path in
//! drawing order (operator and vertices, `re` expanded; circles by
//! operator only) and every text item in order (origin and string).
//! Skipped if `Rscript` isn't available.

mod common;

use std::process::Command;

use rs_graphics::backend::pdf::PdfBackend;
use rs_graphics::device::Device;
use rs_graphics::highlevel::PlotOptions;
use rs_graphics::par::{Par, Pty, Sides};
use rs_graphics::graphics::Graphics;

type Dev = Device<PdfBackend>;

/// R: `pl(n, main)` is `plot(1:n, main = main, xlab = 'x', ylab = 'y')`.
const R_PRELUDE: &str = "pl <- function(n, main) plot(1:n, main = main, xlab = 'x', ylab = 'y')\n";

fn pl(d: &mut Dev, n: usize, main: &'static str) {
    let x: Vec<f64> = (1..=n).map(|i| i as f64).collect();
    let o = PlotOptions {
        main: Some(main.into()),
        xlab: Some("x".into()),
        ylab: Some("y".into()),
        ..PlotOptions::default()
    };
    d.plot(&x, &x).with(&o).draw();
}

struct Case {
    name: &'static str,
    size: (f64, f64),
    r: &'static str,
    setup: fn(&mut Par),
    draw: fn(&mut Dev),
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "pty_wide",
            size: (8.0, 5.0),
            r: "par(pty = 's'); pl(10, 'Square')",
            setup: |p| p.pty = Pty::Square,
            draw: |d| pl(d, 10, "Square"),
        },
        Case {
            name: "pty_tall",
            size: (4.0, 7.0),
            r: "par(pty = 's'); pl(10, 'Square')",
            setup: |p| p.pty = Pty::Square,
            draw: |d| pl(d, 10, "Square"),
        },
        Case {
            name: "plt",
            size: (7.0, 6.0),
            r: "par(plt = c(0.2, 0.9, 0.3, 0.8)); pl(10, 'plt')",
            setup: |p| p.plt = Some([0.2, 0.9, 0.3, 0.8]),
            draw: |d| pl(d, 10, "plt"),
        },
        Case {
            name: "pty_then_plt",
            size: (7.0, 6.0),
            r: "par(pty = 's', plt = c(0.1, 0.95, 0.2, 0.9)); pl(10, 'plt wins')",
            setup: |p| {
                p.pty = Pty::Square;
                p.plt = Some([0.1, 0.95, 0.2, 0.9]);
            },
            draw: |d| pl(d, 10, "plt wins"),
        },
        Case {
            name: "fig_new",
            size: (8.0, 6.0),
            r: "par(fig = c(0, 0.6, 0, 1)); pl(10, 'left')\n\
                par(fig = c(0.6, 1, 0.3, 1), new = TRUE); pl(5, 'right')",
            setup: |_| {},
            draw: |d| {
                d.fig([0.0, 0.6, 0.0, 1.0]);
                pl(d, 10, "left");
                d.fig([0.6, 1.0, 0.3, 1.0]);
                d.set_new(true);
                pl(d, 5, "right");
            },
        },
        Case {
            name: "fig_oma",
            size: (7.0, 6.0),
            r: "par(oma = c(2, 3, 1, 1), fig = c(0.1, 0.9, 0.2, 0.9)); pl(10, 'oma')",
            setup: |p| p.oma = Sides::new(2.0, 3.0, 1.0, 1.0),
            draw: |d| {
                d.fig([0.1, 0.9, 0.2, 0.9]);
                pl(d, 10, "oma");
            },
        },
        Case {
            name: "fig_after_mfrow",
            size: (8.0, 6.0),
            r: "par(mfrow = c(2, 2)); pl(10, 'A')\n\
                par(fig = c(0.5, 1, 0, 0.5), new = TRUE); pl(5, 'inset')",
            setup: |_| {},
            draw: |d| {
                d.mfrow(2, 2);
                pl(d, 10, "A");
                d.fig([0.5, 1.0, 0.0, 0.5]);
                d.set_new(true);
                pl(d, 5, "inset");
            },
        },
    ]
}

#[derive(Debug)]
struct Painted {
    op: String,
    vertices: Vec<f64>,
}

#[derive(Debug)]
struct Text {
    string: String,
    x: f64,
    y: f64,
}

/// Painted paths (clipping paths excluded) and text items, in order.
fn extract(pdf: &str) -> (Vec<Painted>, Vec<Text>) {
    let mut painted = Vec::new();
    let mut texts = Vec::new();
    let mut path: Vec<f64> = Vec::new();
    let mut curved = false;
    let mut pending = false;
    let mut nums: Vec<f64> = Vec::new();
    for line in pdf.lines() {
        // A text matrix, then the string(s) shown (on the same line in R's
        // output, the next line in ours), kerning numbers dropped.
        if let Some(i) = line.find(" Tm") {
            let m: Vec<f64> = line[..i]
                .split_whitespace()
                .rev()
                .take(2)
                .filter_map(|t| t.parse().ok())
                .collect();
            if m.len() == 2 {
                texts.push(Text { string: String::new(), x: m[1], y: m[0] });
                pending = true;
            }
        }
        if pending && (line.contains("Tj") || line.contains("TJ")) {
            let start = line.find(" Tm").map_or(0, |i| i + 3);
            let mut depth = 0;
            let string = &mut texts.last_mut().unwrap().string;
            for c in line[start..].chars() {
                match c {
                    '(' => depth += 1,
                    ')' => depth -= 1,
                    _ if depth > 0 => string.push(c),
                    _ => {}
                }
            }
            pending = false;
            continue;
        }
        if line.contains(" Tm") {
            continue;
        }
        for t in line.split_whitespace() {
            if let Ok(v) = t.parse::<f64>() {
                nums.push(v);
                continue;
            }
            match t {
                "m" | "l" if nums.len() >= 2 => path.extend(&nums[nums.len() - 2..]),
                "re" if nums.len() >= 4 => {
                    // Normalised to a positive width and height (R writes
                    // some with a negative height).
                    let mut n = nums[nums.len() - 4..].to_vec();
                    for (p, d) in [(0, 2), (1, 3)] {
                        if n[d] < 0.0 {
                            n[p] += n[d];
                            n[d] = -n[d];
                        }
                    }
                    path.extend([
                        n[0],
                        n[1],
                        n[0] + n[2],
                        n[1],
                        n[0] + n[2],
                        n[1] + n[3],
                        n[0],
                        n[1] + n[3],
                    ]);
                }
                // Circles (points) are compared by operator only: R and we
                // start them at different points.
                "c" => curved = true,
                "n" => {
                    path.clear();
                    curved = false;
                }
                "S" | "s" | "f" | "F" | "f*" | "B" | "B*" | "b" | "b*" => {
                    let mut vertices = std::mem::take(&mut path);
                    if std::mem::take(&mut curved) {
                        vertices.clear();
                    }
                    painted.push(Painted {
                        op: t.to_string(),
                        vertices,
                    });
                }
                _ => {}
            }
            nums.clear();
        }
    }
    (painted, texts)
}

#[test]
fn regions_match_r() {
    if !common::r_available() {
        return;
    }
    let dir = std::env::temp_dir();
    let cases = cases();
    let mut script = R_PRELUDE.to_string();
    for c in &cases {
        let path = dir.join(format!("rs_graphics_regions_{}.pdf", c.name));
        script.push_str(&format!(
            "pdf(file = '{}', width = {}, height = {}, compress = FALSE)\n{}\n\
             invisible(dev.off())\n",
            path.display(),
            c.size.0,
            c.size.1,
            c.r
        ));
    }
    let script_path = dir.join("rs_graphics_regions.R");
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
        let path = dir.join(format!("rs_graphics_regions_{}.pdf", c.name));
        let (r_paths, r_texts) = extract(&String::from_utf8_lossy(&std::fs::read(&path).unwrap()));
        let mut par = Par::default();
        (c.setup)(&mut par);
        let mut dev = Device::new(
            PdfBackend::new(c.size.0, c.size.1).with_compression(false),
            par,
        );
        (c.draw)(&mut dev);
        let (our_paths, our_texts) =
            extract(&String::from_utf8_lossy(&dev.into_backend().finish()));
        let name = c.name;

        assert!(!r_paths.is_empty(), "{name}: nothing from R");
        assert_eq!(our_paths.len(), r_paths.len(), "{name}: painted path count");
        for (k, (a, b)) in our_paths.iter().zip(&r_paths).enumerate() {
            let ctx = format!("{name}, path {k}:\n ours={a:?}\n r=   {b:?}");
            assert_eq!(a.op, b.op, "operator differs. {ctx}");
            assert!(
                a.vertices.len() == b.vertices.len()
                    && a.vertices.iter().zip(&b.vertices).all(|(p, q)| (p - q).abs() < 0.05),
                "vertices differ. {ctx}"
            );
        }
        assert_eq!(our_texts.len(), r_texts.len(), "{name}: text count");
        for (a, b) in our_texts.iter().zip(&r_texts) {
            assert_eq!(a.string, b.string, "{name}: text differs");
            assert!(
                (a.x - b.x).abs() < 0.05 && (a.y - b.y).abs() < 0.05,
                "{name}: text origin differs: ours={a:?} r={b:?}"
            );
        }
    }
}

#[test]
fn fig_without_new_starts_a_new_page() {
    let mut dev = Device::new(PdfBackend::new(6.0, 4.0).with_compression(false), Par::default());
    pl(&mut dev, 10, "first");
    dev.fig([0.5, 1.0, 0.5, 1.0]);
    pl(&mut dev, 3, "second");
    let out = String::from_utf8_lossy(&dev.into_backend().finish()).into_owned();
    // Two pages: "first" on page one, "second" on page two.
    assert!(out.contains("/Count 2"), "{out}");
    assert!(out.find("(first)").unwrap() < out.find("(second)").unwrap());
}

#[test]
#[should_panic(expected = "invalid value specified for graphical parameter \"fig\"")]
fn fig_must_be_ordered_fractions() {
    let mut dev = Device::new(PdfBackend::new(6.0, 4.0), Par::default());
    dev.fig([0.5, 0.2, 0.0, 1.0]);
}
