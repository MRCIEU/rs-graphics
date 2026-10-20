// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks `barplot()` (vector, stacked / `beside` matrix, `horiz`,
//! `width`/`space`, `legend.text`) against a real R `pdf()` device's
//! uncompressed output: every bar/legend rectangle and every text origin.
//! Skipped if `Rscript` isn't available.

mod common;

use std::process::Command;

use rs_graphics::backend::pdf::PdfBackend;
use rs_graphics::color::{Color, ColorSpec};
use rs_graphics::device::Device;
use rs_graphics::highlevel::BarplotOptions;
use rs_graphics::par::Par;
use rs_graphics::plotmath::Label;
use rs_graphics::graphics::Graphics;

/// `(x, y)` of every text matrix, sorted.
fn text_origins(pdf: &str) -> Vec<(f64, f64)> {
    let mut out = Vec::new();
    for line in pdf.lines() {
        let Some(idx) = line.find(" Tm") else {
            continue;
        };
        // Reversed: [y, x, ...].
        let nums: Vec<f64> = line[..idx]
            .split_whitespace()
            .rev()
            .take(6)
            .filter_map(|t| t.parse().ok())
            .collect();
        if nums.len() == 6 {
            out.push((nums[1], nums[0]));
        }
    }
    out.sort_by(|a, b| a.partial_cmp(b).unwrap());
    out
}

/// `[x, y_bottom, w, h]` of every drawn rectangle (`... re` without a clip),
/// ignoring the full-page background, normalised so `h > 0`.
fn rects(pdf: &str) -> Vec<[f64; 4]> {
    let mut out = Vec::new();
    for line in pdf.lines() {
        let line = line.trim();
        if !line.ends_with(" re") {
            continue;
        }
        let nums: Vec<f64> = line
            .split_whitespace()
            .filter_map(|t| t.parse().ok())
            .collect();
        if nums.len() != 4 || (nums[2] >= 431.0 && nums[3] >= 287.0) {
            continue;
        }
        let (x, y, w, h) = (nums[0], nums[1], nums[2], nums[3]);
        out.push(if h < 0.0 {
            [x, y + h, w, -h]
        } else {
            [x, y, w, h]
        });
    }
    out.sort_by(|a, b| a.partial_cmp(b).unwrap());
    out
}

fn names(n: &[&str]) -> Option<Vec<Label>> {
    Some(n.iter().map(|&s| s.into()).collect())
}

fn red_green_blue() -> Option<Vec<ColorSpec>> {
    Some(
        ["red", "green", "blue"]
            .iter()
            .map(|c| ColorSpec::Color(Color::parse(c).unwrap()))
            .collect(),
    )
}

/// The 3 x 4 matrix used by the matrix scenarios, as columns.
fn mat() -> Vec<Vec<f64>> {
    vec![
        vec![3.0, 5.0, 2.0],
        vec![4.0, 1.0, 6.0],
        vec![2.0, 2.0, 7.0],
        vec![5.0, 3.0, 1.0],
    ]
}

const R_MAT: &str = "m <- matrix(c(3,5,2, 4,1,6, 2,2,7, 5,3,1), nrow = 3)";

struct Case {
    name: &'static str,
    r: &'static str,
    run: fn(&mut Device<PdfBackend>),
    /// Compare text origins too. Off where negative tick labels appear: R's
    /// pdf device draws `-` as the wider `minus` glyph (see `devPS.c`,
    /// `/Differences [ 45/minus ]`), which we deliberately don't replicate.
    check_text: bool,
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "vector",
            r: r#"barplot(c(23, 41, 12, -5), names.arg = c("A","B","C","D"), main = "Vec", ylab = "v")"#,
            run: |d| {
                let o = BarplotOptions {
                    names: names(&["A", "B", "C", "D"]),
                    main: Some("Vec".into()),
                    ylab: Some("v".into()),
                    ..Default::default()
                };
                d.barplot([23.0, 41.0, 12.0, -5.0]).with(&o).draw();
            },
            check_text: true,
        },
        Case {
            name: "stacked_legend",
            r: r#"barplot(m, names.arg = c("a","b","c","d"), legend.text = c("r1","r2","r3"))"#,
            run: |d| {
                let o = BarplotOptions {
                    names: names(&["a", "b", "c", "d"]),
                    legend_text: names(&["r1", "r2", "r3"]),
                    ..Default::default()
                };
                d.barplot(mat()).with(&o).draw();
            },
            check_text: true,
        },
        Case {
            name: "beside_col",
            r: r#"barplot(m, beside = TRUE, names.arg = c("a","b","c","d"), col = c("red","green","blue"))"#,
            run: |d| {
                let o = BarplotOptions {
                    names: names(&["a", "b", "c", "d"]),
                    beside: true,
                    col: red_green_blue().expect("REASON"),
                    ..Default::default()
                };
                d.barplot(mat()).with(&o).draw();
            },
            check_text: true,
        },
        Case {
            name: "horiz_stacked",
            r: r#"barplot(m, horiz = TRUE, names.arg = c("a","b","c","d"))"#,
            run: |d| {
                let o = BarplotOptions {
                    names: names(&["a", "b", "c", "d"]),
                    horiz: true,
                    ..Default::default()
                };
                d.barplot(mat()).with(&o).draw();
            },
            check_text: true,
        },
        Case {
            name: "horiz_beside_space_width_legend",
            r: r#"barplot(m, horiz = TRUE, beside = TRUE, space = c(0.1, 0.5), width = c(1, 2, 1), legend.text = c("r1","r2","r3"))"#,
            run: |d| {
                let o = BarplotOptions {
                    horiz: true,
                    beside: true,
                    space: Some(vec![0.1, 0.5]),
                    width: vec![1.0, 2.0, 1.0],
                    legend_text: names(&["r1", "r2", "r3"]),
                    ..Default::default()
                };
                d.barplot(mat()).with(&o).draw();
            },
            check_text: true,
        },
        Case {
            name: "stacked_negative_ylim",
            r: r#"barplot(m - 3, ylim = c(-8, 12))"#,
            run: |d| {
                let cols: Vec<Vec<f64>> = mat()
                    .iter()
                    .map(|c| c.iter().map(|v| v - 3.0).collect())
                    .collect();
                let o = BarplotOptions {
                    ylim: Some((-8.0, 12.0)),
                    ..Default::default()
                };
                d.barplot(&cols).with(&o).draw();
            },
            check_text: false,
        },
    ]
}

#[test]
fn barplot_matches_r() {
    if !common::r_available() {
        return;
    }
    let cases = cases();
    let dir = std::env::temp_dir();
    let mut script = format!("{R_MAT}\n");
    for c in &cases {
        script.push_str(&format!(
            "pdf(file = '{}/rs_graphics_barplot_{}.pdf', width = 6, height = 4, compress = FALSE)\n\
             {}\ninvisible(dev.off())\n",
            dir.display(),
            c.name,
            c.r
        ));
    }
    let script_path = dir.join("rs_graphics_barplot.R");
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
        let path = dir.join(format!("rs_graphics_barplot_{}.pdf", c.name));
        let r_pdf = String::from_utf8_lossy(&std::fs::read(&path).unwrap()).into_owned();

        let mut dev = Device::new(PdfBackend::new(6.0, 4.0).with_compression(false), Par::default());
        (c.run)(&mut dev);
        let ours = String::from_utf8_lossy(&dev.into_backend().finish()).into_owned();

        let (r_t, our_t) = if c.check_text {
            (text_origins(&r_pdf), text_origins(&ours))
        } else {
            (Vec::new(), Vec::new())
        };
        assert_eq!(
            our_t.len(),
            r_t.len(),
            "{}: text item count\nours={our_t:?}\nr={r_t:?}",
            c.name
        );
        for (a, b) in our_t.iter().zip(&r_t) {
            assert!(
                (a.0 - b.0).abs() < 0.05 && (a.1 - b.1).abs() < 0.05,
                "{}: text origin differs: ours={a:?} r={b:?}\nours={our_t:?}\nr={r_t:?}",
                c.name
            );
        }

        let (r_r, our_r) = (rects(&r_pdf), rects(&ours));
        assert_eq!(
            our_r.len(),
            r_r.len(),
            "{}: rect count\nours={our_r:?}\nr={r_r:?}",
            c.name
        );
        for (a, b) in our_r.iter().zip(&r_r) {
            for k in 0..4 {
                assert!(
                    (a[k] - b[k]).abs() < 0.05,
                    "{}: rect differs: ours={a:?} r={b:?}",
                    c.name
                );
            }
        }
    }
}
