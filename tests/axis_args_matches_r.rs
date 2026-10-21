// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks `axis()`'s arguments (`line`, `pos`, `outer`, `tck`/`tcl`,
//! `hadj`, `padj`, `gap.axis`, `tick`, `labels` (incl. `NA`), `lty`, `lwd`/`lwd.ticks`,
//! `col`/`col.ticks`/`col.axis`, `cex.axis`, `font`, `xaxt`/`yaxt`) and
//! its tick limits (reversed axes, `par(xpd = TRUE)`) against a real R `pdf()` device's
//! uncompressed output: every stroked path in drawing order (vertices,
//! colour, width, dash pattern) and every label (origin, direction,
//! colour). Skipped if `Rscript` isn't available.

mod common;

use std::process::Command;

use rs_graphics::backend::pdf::PdfBackend;
use rs_graphics::color::{Color, ColorSpec};
use rs_graphics::device::{AxisOptions, Device};
use rs_graphics::lty::LineType;
use rs_graphics::plotmath::Label as AxisLabel;
use rs_graphics::par::{AxisType, Par, Sides};
use rs_graphics::prelude::*;

const PAGE_W: f64 = 7.0;
const PAGE_H: f64 = 6.0;

struct Case {
    name: &'static str,
    /// R code run after `plot.new()`/`plot.window()`.
    r: &'static str,
    /// `par()` settings for R, run before `plot.new()`.
    r_par: &'static str,
    xlim: (f64, f64),
    setup: fn(&mut Par),
    draw: fn(&mut Device<PdfBackend>),
}

fn col(name: &str) -> Option<ColorSpec> {
    Some(ColorSpec::Color(Color::parse(name).unwrap()))
}

/// `c(...)` of strings, `"NA"` being `NA`.
fn labels(l: &[&str]) -> Vec<Option<AxisLabel>> {
    l.iter().map(|s| (*s != "NA").then(|| AxisLabel::from(*s))).collect()
}

/// `c(rbind(NA, strrep(LETTERS[1:10], 3)), 'ZZZ')`.
fn alternating_na() -> Vec<Option<AxisLabel>> {
    let mut l: Vec<Option<AxisLabel>> = ('A'..='J')
        .flat_map(|c| [None, Some(AxisLabel::from(c.to_string().repeat(3)))])
        .collect();
    l.push(Some("ZZZ".into()));
    l
}

fn seq(from: f64, to: f64, by: f64) -> Vec<f64> {
    let n = ((to - from) / by).round() as usize;
    (0..=n).map(|i| from + i as f64 * by).collect()
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "line",
            r_par: "",
            r: "axis(1, line = 1); axis(2, line = -1); \
                axis(3, line = 2, las = 2); axis(4, line = 0.5, las = 1)",
            xlim: (0.0, 10.0),
            setup: |_| {},
            draw: |d| {
                d.axis()
                    .side(1)
                    .with(&AxisOptions {
                        line: Some(1.0),
                        ..Default::default()
                    })
                    .draw();
                d.axis()
                    .side(2)
                    .with(&AxisOptions {
                        line: Some(-1.0),
                        ..Default::default()
                    })
                    .draw();
                let o = AxisOptions {
                    line: Some(2.0),
                    las: Some(2),
                    ..Default::default()
                };
                d.axis().side(3).with(&o).draw();
                let o = AxisOptions { line: Some(0.5), las: Some(1), ..Default::default() };
                d.axis().side(4).with(&o).draw();
            },
        },
        Case {
            name: "pos",
            r_par: "",
            r: "axis(1, pos = 2); axis(2, pos = 3, las = 1); \
                axis(3, pos = 8, line = 3); axis(4, pos = 12)",
            xlim: (0.0, 10.0),
            setup: |_| {},
            draw: |d| {
                d.axis()
                    .side(1)
                    .with(&AxisOptions {
                        pos: Some(2.0),
                        ..Default::default()
                    })
                    .draw();
                let o = AxisOptions {
                    pos: Some(3.0),
                    las: Some(1),
                    ..Default::default()
                };
                d.axis().side(2).with(&o).draw();
                let o = AxisOptions { pos: Some(8.0), line: Some(3.0), ..Default::default() };
                d.axis().side(3).with(&o).draw();
                d.axis()
                    .side(4)
                    .with(&AxisOptions {
                        pos: Some(12.0),
                        ..Default::default()
                    })
                    .draw();
            },
        },
        Case {
            name: "outer",
            r_par: "par(oma = c(2, 3, 2, 1))",
            r: "axis(1, outer = TRUE); axis(2, outer = TRUE, line = 1); \
                axis(3, outer = TRUE, line = -1); axis(4, outer = TRUE, las = 2)",
            xlim: (0.0, 10.0),
            setup: |p| p.oma = Sides::new(2.0, 3.0, 2.0, 1.0),
            draw: |d| {
                d.axis()
                    .side(1)
                    .with(&AxisOptions {
                        outer: true,
                        ..Default::default()
                    })
                    .draw();
                let o = AxisOptions {
                    outer: true,
                    line: Some(1.0),
                    ..Default::default()
                };
                d.axis().side(2).with(&o).draw();
                let o = AxisOptions { outer: true, line: Some(-1.0), ..Default::default() };
                d.axis().side(3).with(&o).draw();
                let o = AxisOptions { outer: true, las: Some(2), ..Default::default() };
                d.axis().side(4).with(&o).draw();
            },
        },
        Case {
            name: "tck",
            r_par: "",
            r: "axis(1, tck = 0.02); axis(2, tck = -0.03); \
                axis(3, tck = 1, col.ticks = 'grey'); axis(4, tcl = 0.4)",
            xlim: (0.0, 10.0),
            setup: |_| {},
            draw: |d| {
                d.axis()
                    .side(1)
                    .with(&AxisOptions {
                        tck: Some(0.02),
                        ..Default::default()
                    })
                    .draw();
                d.axis()
                    .side(2)
                    .with(&AxisOptions {
                        tck: Some(-0.03),
                        ..Default::default()
                    })
                    .draw();
                let o = AxisOptions {
                    tck: Some(1.0),
                    col_ticks: col("grey"),
                    ..Default::default()
                };
                d.axis().side(3).with(&o).draw();
                d.axis()
                    .side(4)
                    .with(&AxisOptions {
                        tcl: Some(0.4),
                        ..Default::default()
                    })
                    .draw();
            },
        },
        Case {
            name: "adj",
            r_par: "",
            r: "axis(1, hadj = 0, padj = c(0, 1)); \
                axis(2, las = 1, hadj = 0.5, padj = -0.5); \
                axis(3, las = 2, hadj = 0.2); axis(4, padj = c(1, NA, 0.5))",
            xlim: (0.0, 10.0),
            setup: |_| {},
            draw: |d| {
                let o = AxisOptions { hadj: Some(0.0), padj: vec![0.0, 1.0], ..Default::default() };
                d.axis().side(1).with(&o).draw();
                let o = AxisOptions {
                    las: Some(1),
                    hadj: Some(0.5),
                    padj: vec![-0.5],
                    ..Default::default()
                };
                d.axis().side(2).with(&o).draw();
                let o = AxisOptions { las: Some(2), hadj: Some(0.2), ..Default::default() };
                d.axis().side(3).with(&o).draw();
                let o = AxisOptions { padj: vec![1.0, f64::NAN, 0.5], ..Default::default() };
                d.axis().side(4).with(&o).draw();
            },
        },
        Case {
            name: "gap",
            r_par: "",
            r: "axis(1, at = seq(0, 10, 0.5), gap.axis = -0.5); \
                axis(3, at = seq(0, 10, 0.5), gap.axis = 2); \
                axis(2, at = seq(0, 10, 0.25), las = 1, gap.axis = 0); \
                axis(4, at = seq(0, 10, 0.25), las = 1, gap.axis = 3)",
            xlim: (0.0, 10.0),
            setup: |_| {},
            draw: |d| {
                let at = Some(seq(0.0, 10.0, 0.5));
                let o = AxisOptions { at: at.clone(), gap_axis: Some(-0.5), ..Default::default() };
                d.axis().side(1).with(&o).draw();
                let o = AxisOptions { at, gap_axis: Some(2.0), ..Default::default() };
                d.axis().side(3).with(&o).draw();
                let at = Some(seq(0.0, 10.0, 0.25));
                let o = AxisOptions {
                    at: at.clone(),
                    las: Some(1),
                    gap_axis: Some(0.0),
                    ..Default::default()
                };
                d.axis().side(2).with(&o).draw();
                let o = AxisOptions { at, las: Some(1), gap_axis: Some(3.0), ..Default::default() };
                d.axis().side(4).with(&o).draw();
            },
        },
        Case {
            name: "styles",
            r_par: "par(lwd = 3)",
            r: "axis(1, lwd = 2, lwd.ticks = 0.5, col = 'red', col.ticks = 'blue', \
                     col.axis = 'darkgreen', cex.axis = 0.8, font = 2); \
                axis(2, lty = 'dashed', labels = FALSE); \
                axis(3, tick = FALSE, at = c(2, 5, 7), labels = c('a', 'bb', 'ccc')); \
                axis(4, lwd = 0, lwd.ticks = 2)",
            xlim: (0.0, 10.0),
            setup: |p| p.lwd = 3.0,
            draw: |d| {
                let o = AxisOptions {
                    lwd: Some(2.0),
                    lwd_ticks: Some(0.5),
                    col: col("red"),
                    col_ticks: col("blue"),
                    col_axis: col("darkgreen"),
                    cex_axis: Some(0.8),
                    font: Some(FontFace::Bold),
                    ..Default::default()
                };
                d.axis().side(1).with(&o).draw();
                let o = AxisOptions {
                    lty: Some(LineType::from_code(2)),
                    draw_labels: false,
                    ..Default::default()
                };
                d.axis().side(2).with(&o).draw();
                let o = AxisOptions {
                    tick: false,
                    at: Some(vec![2.0, 5.0, 7.0]),
                    labels: Some(labels(&["a", "bb", "ccc"])),
                    ..Default::default()
                };
                d.axis().side(3).with(&o).draw();
                let o = AxisOptions {
                    lwd: Some(0.0),
                    lwd_ticks: Some(2.0),
                    ..Default::default()
                };
                d.axis().side(4).with(&o).draw();
            },
        },
        Case {
            name: "axt",
            r_par: "par(xaxt = 'n')",
            r: "axis(1); axis(3, xaxt = 's'); axis(2, yaxt = 'n'); axis(4)",
            xlim: (0.0, 10.0),
            setup: |p| p.xaxt = AxisType::None,
            draw: |d| {
                d.axis().side(1).draw();
                let o = AxisOptions { axt: Some(AxisType::Standard), ..Default::default() };
                d.axis().side(3).with(&o).draw();
                let o = AxisOptions { axt: Some(AxisType::None), ..Default::default() };
                d.axis().side(2).with(&o).draw();
                d.axis().side(4).draw();
            },
        },
        Case {
            name: "reversed_xpd",
            r_par: "",
            r: "axis(3); par(xpd = TRUE); axis(1, at = c(-1, 0, 5, 10, 11), labels = c('a', 'b', 'c', 'd', 'e')); \
                axis(2, at = c(-2, 5, 12), labels = c('lo', 'mid', 'hi'), las = 1); \
                axis(4, at = c(8, 2), labels = c('eight', 'two'))",
            xlim: (10.0, 0.0),
            setup: |_| {},
            draw: |d| {
                d.axis().side(3).draw();
                d.par().xpd(Xpd::Figure).draw();
                // Letters, not negative numbers: R's `pdf()` draws `-` as
                // the wider `/minus` glyph, which we deliberately don't.
                let o = AxisOptions {
                    at: Some(vec![-1.0, 0.0, 5.0, 10.0, 11.0]),
                    labels: Some(labels(&["a", "b", "c", "d", "e"])),
                    ..Default::default()
                };
                d.axis().side(1).with(&o).draw();
                let o = AxisOptions {
                    at: Some(vec![-2.0, 5.0, 12.0]),
                    labels: Some(labels(&["lo", "mid", "hi"])),
                    las: Some(1),
                    ..Default::default()
                };
                d.axis().side(2).with(&o).draw();
                let o = AxisOptions {
                    at: Some(vec![8.0, 2.0]),
                    labels: Some(labels(&["eight", "two"])),
                    ..Default::default()
                };
                d.axis().side(4).with(&o).draw();
            },
        },
        Case {
            name: "na_labels",
            r_par: "",
            r: "axis(1, at = 0:4 * 2.5, labels = c('a', NA, 'c', NA, 'e')); \
                axis(3, at = seq(0, 10, 0.5), labels = c(rbind(NA, strrep(LETTERS[1:10], 3)), 'ZZZ')); \
                axis(2, at = c(1, 5, 9), labels = c(NA, NA, NA))",
            xlim: (0.0, 10.0),
            setup: |_| {},
            draw: |d| {
                let o = AxisOptions {
                    at: Some(seq(0.0, 10.0, 2.5)),
                    labels: Some(labels(&["a", "NA", "c", "NA", "e"])),
                    ..Default::default()
                };
                d.axis().side(1).with(&o).draw();
                // NA between labels spaced so that `gap.axis` would drop
                // every one of them if the NAs took part.
                let o = AxisOptions {
                    at: Some(seq(0.0, 10.0, 0.5)),
                    labels: Some(alternating_na()),
                    ..Default::default()
                };
                d.axis().side(3).with(&o).draw();
                let o = AxisOptions {
                    at: Some(vec![1.0, 5.0, 9.0]),
                    labels: Some(labels(&["NA", "NA", "NA"])),
                    ..Default::default()
                };
                d.axis().side(2).with(&o).draw();
            },
        },
        Case {
            name: "mgp",
            r_par: "par(mgp = c(2, 0.7, 0.2))",
            r: "axis(1, mgp = c(3, 0.3, 0)); axis(2); \
                axis(3, mgp = c(3, 1.5, 0.5), line = 1); axis(4, mgp = c(3, 0.2, -0.5), pos = 9, las = 1)",
            xlim: (0.0, 10.0),
            setup: |p| p.mgp = [2.0, 0.7, 0.2],
            draw: |d| {
                d.axis().side(1).mgp([3.0, 0.3, 0.0]).draw();
                // `par(mgp)` when not given inline.
                d.axis().side(2).draw();
                d.axis().side(3).mgp([3.0, 1.5, 0.5]).line(1).draw();
                d.axis().side(4).mgp([3.0, 0.2, -0.5]).pos(9).las(1).draw();
            },
        },
        Case {
            name: "mex_2",
            r_par: "par(mex = 2, mgp = c(2.5, 0.8, 0))",
            r: "axis(1); axis(2); axis(3, las = 2); axis(4, las = 1); \
                title(main = 'M', sub = 'S', xlab = 'X', ylab = 'Y'); \
                mtext('A', 3, line = 1); mtext('B', 4, line = 1)",
            xlim: (0.0, 10.0),
            setup: |p| {
                p.mex = 2.0;
                p.mgp = [2.5, 0.8, 0.0];
            },
            draw: |d| {
                d.axis().side(1).draw();
                d.axis().side(2).draw();
                d.axis().side(3).las(2).draw();
                d.axis().side(4).las(1).draw();
                d.title().main("M").sub("S").xlab("X").ylab("Y").draw();
                d.mtext().text("A").side(3).line(1).draw();
                d.mtext().text("B").side(4).line(1).draw();
            },
        },
        Case {
            name: "mex_0_7",
            r_par: "par(mex = 0.7)",
            r: "axis(1); axis(2); axis(3, las = 2); axis(4, las = 1); \
                title(main = 'M', sub = 'S', xlab = 'X', ylab = 'Y'); \
                mtext('A', 3, line = 1); mtext('B', 4, line = 1)",
            xlim: (0.0, 10.0),
            setup: |p| p.mex = 0.7,
            draw: |d| {
                d.axis().side(1).draw();
                d.axis().side(2).draw();
                d.axis().side(3).las(2).draw();
                d.axis().side(4).las(1).draw();
                d.title().main("M").sub("S").xlab("X").ylab("Y").draw();
                d.mtext().text("A").side(3).line(1).draw();
                d.mtext().text("B").side(4).line(1).draw();
            },
        },
        Case {
            name: "na_labels_reversed",
            r_par: "",
            r: "axis(1, at = seq(0, 10, 0.5), labels = c(rbind(NA, strrep(LETTERS[1:10], 3)), 'ZZZ'))",
            xlim: (10.0, 0.0),
            setup: |_| {},
            draw: |d| {
                let o = AxisOptions {
                    at: Some(seq(0.0, 10.0, 0.5)),
                    labels: Some(alternating_na()),
                    ..Default::default()
                };
                d.axis().side(1).with(&o).draw();
            },
        },
    ]
}

fn r_script(c: &Case, path: &str) -> String {
    format!(
        "pdf(file = '{path}', width = {PAGE_W}, height = {PAGE_H}, compress = FALSE)\n\
         {par}\n\
         plot.new(); plot.window(xlim = c({x0}, {x1}), ylim = c(0, 10))\n\
         {body}\n\
         invisible(dev.off())\n",
        par = c.r_par,
        x0 = c.xlim.0,
        x1 = c.xlim.1,
        body = c.r,
    )
}

fn draw_ours(c: &Case) -> String {
    let mut par = Par::default();
    (c.setup)(&mut par);
    let mut dev = Device::new(
        PdfBackend::new(PAGE_W, PAGE_H).with_compression(false),
        par,
    );
    dev.plot_new();
    dev.plot_window()
        .xlim(c.xlim.0, c.xlim.1)
        .ylim(0.0, 10.0)
        .draw();
    (c.draw)(&mut dev);
    String::from_utf8_lossy(&dev.into_backend().finish()).into_owned()
}

// -------------------------------------------------------------------
// A minimal PDF content-stream interpreter (see `boxplot_draw_matches_r.rs`)
// -------------------------------------------------------------------

#[derive(Clone, Debug)]
struct Gs {
    fill: [f64; 3],
    stroke: [f64; 3],
    lwd: f64,
    dash: Vec<f64>,
}

#[derive(Debug)]
struct Stroked {
    vertices: Vec<f64>,
    colour: [f64; 3],
    lwd: f64,
    dash: Vec<f64>,
}

#[derive(Debug)]
struct Label {
    /// Text-matrix direction (signs) and origin.
    dir: (i32, i32),
    origin: (f64, f64),
    colour: [f64; 3],
}

fn tokens(content: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut chars = content.chars().peekable();
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
        } else if c == '[' || c == ']' {
            chars.next();
            out.push(c.to_string());
        } else {
            let mut s = String::new();
            while let Some(&c) = chars.peek() {
                if c.is_whitespace() || c == '[' || c == ']' {
                    break;
                }
                s.push(c);
                chars.next();
            }
            out.push(s);
        }
    }
    out
}

fn sgn(v: f64) -> i32 {
    if v > 1e-9 {
        1
    } else if v < -1e-9 {
        -1
    } else {
        0
    }
}

fn interpret(pdf: &str) -> (Vec<Stroked>, Vec<Label>) {
    let mut strokes = Vec::new();
    let mut labels = Vec::new();
    let mut rest = pdf;
    while let Some(i) = rest.find("stream\n") {
        let body = &rest[i + 7..];
        let Some(j) = body.find("endstream") else {
            break;
        };
        let content = &body[..j];
        rest = &body[j + 9..];
        if !content.contains(" m") && !content.contains(" Tm") {
            continue;
        }
        let mut gs = Gs {
            fill: [0.0; 3],
            stroke: [0.0; 3],
            lwd: 1.0,
            dash: Vec::new(),
        };
        let mut stack: Vec<Gs> = Vec::new();
        let mut nums: Vec<f64> = Vec::new();
        let mut array: Option<Vec<f64>> = None;
        let mut path: Vec<f64> = Vec::new();
        for t in tokens(content) {
            if let Ok(v) = t.parse::<f64>() {
                match &mut array {
                    Some(a) => a.push(v),
                    None => nums.push(v),
                }
                continue;
            }
            let last = |k: usize| nums[nums.len() - k..].to_vec();
            match t.as_str() {
                "[" => array = Some(Vec::new()),
                "]" => {
                    gs.dash = array.take().unwrap_or_default();
                    continue;
                }
                "q" => stack.push(gs.clone()),
                "Q" => gs = stack.pop().unwrap(),
                "rg" | "scn" => gs.fill = last(3).try_into().unwrap(),
                "RG" | "SCN" => gs.stroke = last(3).try_into().unwrap(),
                "g" => gs.fill = [nums[nums.len() - 1]; 3],
                "G" => gs.stroke = [nums[nums.len() - 1]; 3],
                "w" => gs.lwd = nums[nums.len() - 1],
                "m" | "l" => path.extend(last(2)),
                "n" => path.clear(),
                "S" => strokes.push(Stroked {
                    vertices: std::mem::take(&mut path),
                    colour: gs.stroke,
                    lwd: gs.lwd,
                    dash: gs.dash.clone(),
                }),
                "f" | "F" | "f*" | "B" | "b" | "s" => path.clear(),
                "Tm" => {
                    let m = last(6);
                    labels.push(Label {
                        dir: (sgn(m[0]), sgn(m[1])),
                        origin: (m[4], m[5]),
                        colour: gs.fill,
                    });
                }
                _ => {}
            }
            nums.clear();
        }
    }
    (strokes, labels)
}

fn close(a: &[f64], b: &[f64], tol: f64) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < tol)
}

#[test]
fn axis_arguments_match_r() {
    if !common::r_available() {
        return;
    }
    let dir = std::env::temp_dir();
    let cases = cases();
    let mut script = String::new();
    for c in &cases {
        let path = dir.join(format!("rs_graphics_axis_args_{}.pdf", c.name));
        script.push_str(&r_script(c, &path.display().to_string()));
    }
    let script_path = dir.join("rs_graphics_axis_args.R");
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
        let path = dir.join(format!("rs_graphics_axis_args_{}.pdf", c.name));
        let r_pdf = String::from_utf8_lossy(&std::fs::read(&path).unwrap()).into_owned();
        let (r_strokes, r_labels) = interpret(&r_pdf);
        let (our_strokes, our_labels) = interpret(&draw_ours(c));
        let name = c.name;

        assert_eq!(
            our_strokes.len(),
            r_strokes.len(),
            "{name}: stroked path count\nours={our_strokes:#?}\nr={r_strokes:#?}"
        );
        for (k, (a, b)) in our_strokes.iter().zip(&r_strokes).enumerate() {
            let ctx = format!("{name}, path {k}:\n ours={a:?}\n r=   {b:?}");
            assert!(close(&a.vertices, &b.vertices, 0.05), "vertices differ. {ctx}");
            assert!(close(&a.colour, &b.colour, 0.005), "colour differs. {ctx}");
            // R writes widths to 2 decimals.
            assert!((a.lwd - b.lwd).abs() < 0.0051, "line width differs. {ctx}");
            assert!(close(&a.dash, &b.dash, 0.005), "dash pattern differs. {ctx}");
        }

        assert!(!r_labels.is_empty(), "{name}: no labels from R");
        assert_eq!(
            our_labels.len(),
            r_labels.len(),
            "{name}: label count\nours={our_labels:#?}\nr={r_labels:#?}"
        );
        for (k, (a, b)) in our_labels.iter().zip(&r_labels).enumerate() {
            let ctx = format!("{name}, label {k}:\n ours={a:?}\n r=   {b:?}");
            assert_eq!(a.dir, b.dir, "direction differs. {ctx}");
            assert!(
                close(&[a.origin.0, a.origin.1], &[b.origin.0, b.origin.1], 0.05),
                "origin differs. {ctx}"
            );
            assert!(close(&a.colour, &b.colour, 0.005), "colour differs. {ctx}");
        }
    }
}

/// `axis()`'s return value: R's `at` (or default ticks) sorted with `NA`
/// last, `NULL` (empty here) for an empty `at` or `xaxt = "n"`, in data
/// units on a log axis; and the same values from `Device::add_figure`.
#[test]
fn axis_returns_tick_positions_like_r() {
    if !common::r_available() {
        return;
    }
    let r = Command::new("Rscript")
        .args([
            "--vanilla",
            "-e",
            "pdf(NULL); p <- function(v) cat(paste(v, collapse = ','), '\n', sep = '')\n\
             plot.new(); plot.window(c(0, 10), c(0, 10))\n\
             p(axis(1)); p(axis(2, at = c(Inf, 3, NA, -Inf, 1))); p(axis(3, at = c(12, 5, -3)))\n\
             p(axis(4, at = numeric(0))); p(axis(1, xaxt = 'n')); p(axis(2, labels = FALSE, tick = FALSE))\n\
             plot.new(); plot.window(c(1, 1000), c(1, 10), log = 'x')\n\
             p(axis(1)); p(axis(3, at = c(100, 5, 2000)))",
        ])
        .output()
        .expect("run Rscript");
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    let expected = String::from_utf8(r.stdout).unwrap();

    fn scene<G: Graphics>(g: &mut G) -> Vec<G::AxisResult> {
        g.plot_new();
        g.plot_window().xlim(0, 10).ylim(0, 10).draw();
        let mut out = vec![
            g.axis().side(1).draw(),
            g.axis()
                .side(2)
                // A negative NaN too (as `0.0 / 0.0` gives on x86), which a
                // plain `total_cmp` sorts first.
                .at([f64::INFINITY, -f64::NAN, 3.0, f64::NEG_INFINITY, 1.0])
                .draw(),
            g.axis().side(3).at([12, 5, -3]).draw(),
            g.axis().side(4).at(Vec::<f64>::new()).draw(),
            g.axis().side(1).axt("n").draw(),
            g.axis().side(2).draw_labels(false).tick(false).draw(),
        ];
        g.plot_new();
        g.plot_window().xlim(1, 1000).ylim(1, 10).xlog(true).draw();
        out.push(g.axis().side(1).draw());
        out.push(g.axis().side(3).at([100, 5, 2000]).draw());
        out
    }
    let show = |v: &[f64]| {
        v.iter()
            .map(|x| match x {
                x if x.is_nan() => "NA".to_string(),
                x if x.is_infinite() => if *x > 0.0 { "Inf" } else { "-Inf" }.to_string(),
                x => format!("{x}"),
            })
            .collect::<Vec<_>>()
            .join(",")
    };
    let device = || Device::new(PdfBackend::new(PAGE_W, PAGE_H), Par::default());
    let mut dev = device();
    let ours: String = scene(&mut dev).iter().map(|v| show(v) + "\n").collect();
    assert_eq!(ours, expected);

    let mut fig = Figure::new();
    scene(&mut fig);
    let replayed = device().add_figure(&fig);
    let from_figure: String = replayed.axes.iter().map(|v| show(v) + "\n").collect();
    assert_eq!(from_figure, expected);
}
