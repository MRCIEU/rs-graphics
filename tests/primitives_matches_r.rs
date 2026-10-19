// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks the options-struct primitives against an uncompressed R
//! `pdf()`: per-element recycling of styles and coordinates in
//! `points()`/`segments()`/`arrows()`/`rect()`/`polygon()`/`text()`,
//! `lines(type = )`, R's `par("fg")` defaults, `NA`-separated polygons,
//! vectorised and default `text()` labels, `box(which = )` and
//! `plot.window(asp = )`, and `plot()`'s axes taking their style from
//! `par()`. Every painted path (operator, vertices, fill and
//! stroke colour, width, dash; circles by operator only) and every text
//! item (string, origin, size, colour) is compared in order. Skipped if
//! `Rscript` isn't available.

use std::process::Command;

use rs_graphics::backend::pdf::PdfBackend;
use rs_graphics::prelude::*;

fn rscript_available() -> bool {
    Command::new("Rscript")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

const PAGE_W: f64 = 7.0;
const PAGE_H: f64 = 6.0;

type Dev = Device<PdfBackend>;

fn cols(names: &[&str]) -> Vec<ColorSpec> {
    names.iter().map(|&n| ColorSpec::from(n)).collect()
}

/// `plot.new(); plot.window(c(0, 10), c(0, 10))`.
fn window(d: &mut Dev) {
    d.plot_new();
    d.plot_window((0.0, 10.0), (0.0, 10.0)).draw();
}

const WINDOW_R: &str = "plot.new(); plot.window(c(0, 10), c(0, 10))\n";

struct Case {
    name: &'static str,
    r: &'static str,
    draw: fn(&mut Dev),
}

const Y: [f64; 9] = [2.0, 5.0, 3.0, 8.0, 1.0, 6.0, 4.0, 9.0, 7.0];

fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "points_recycled",
            r: "points(1:9, 1:9, pch = c(1, 19, 21, 3), col = c('red', 'blue', 'darkgreen'),\n\
                bg = c('yellow', 'pink'), cex = c(1, 2), lwd = c(1, 3))",
            draw: |d| {
                let x: Vec<f64> = (1..=9).map(f64::from).collect();
                let opts = XyOptions {
                    pch: vec![1, 19, 21, 3],
                    col: cols(&["red", "blue", "darkgreen"]),
                    bg: cols(&["yellow", "pink"]),
                    cex: vec![1.0, 2.0],
                    lwd: vec![1.0, 3.0],
                    ..XyOptions::default()
                };
                d.points(&x, &x).with(&opts).draw();
            },
        },
        Case {
            name: "lines_types",
            r: "x <- 1:9; y <- c(2, 5, 3, 8, 1, 6, 4, 9, 7)\n\
                lines(x, y, type = 'b', pch = 19, col = 'red', lwd = 2, lty = 2)\n\
                lines(x, y + 0.5, type = 'o', pch = 0, col = c('blue', 'green'))\n\
                points(x, y - 0.5, type = 'c', lty = 3)\n\
                lines(x, y - 1, type = 'p', pch = 4)",
            draw: |d| {
                let x: Vec<f64> = (1..=9).map(f64::from).collect();
                let shift = |s: f64| Y.iter().map(|v| v + s).collect::<Vec<_>>();
                let b = XyOptions {
                    type_: Some(PlotType::Both),
                    pch: vec![19],
                    col: cols(&["red"]),
                    lwd: vec![2.0],
                    lty: Some(LineType::DASHED),
                    ..XyOptions::default()
                };
                d.lines(&x, &Y).with(&b).draw();
                let o = XyOptions {
                    type_: Some(PlotType::Overplotted),
                    pch: vec![0],
                    col: cols(&["blue", "green"]),
                    ..XyOptions::default()
                };
                d.lines(&x, &shift(0.5)).with(&o).draw();
                let c = XyOptions {
                    type_: Some(PlotType::BrokenLines),
                    lty: Some(LineType::DOTTED),
                    ..XyOptions::default()
                };
                d.points(&x, &shift(-0.5)).with(&c).draw();
                let p = XyOptions {
                    type_: Some(PlotType::Points),
                    pch: vec![4],
                    ..XyOptions::default()
                };
                d.lines(&x, &shift(-1.0)).with(&p).draw();
            },
        },
        Case {
            name: "segments_arrows",
            r: "par(fg = 'blue')\n\
                segments(1, 1:4, 9, 1:4, lty = 1:2, lwd = c(1, 4))\n\
                segments(c(1, 2), 5, c(3, NA), 6, col = 'red')\n\
                arrows(c(2, 5, 7), 6, c(3, 8, 9), 9, code = 3, angle = 90, length = 0.1,\n\
                       col = c('red', NA))\n\
                arrows(1, 9, 1, 7)",
            draw: |d| {
                d.par.fg = Color::parse("blue").unwrap();
                let solid_dashed = SegmentsOptions {
                    lty: vec![LineType::SOLID, LineType::DASHED],
                    lwd: vec![1.0, 4.0],
                    ..SegmentsOptions::default()
                };
                d.segments(&[1.0], &[1.0, 2.0, 3.0, 4.0], &[9.0], &[1.0, 2.0, 3.0, 4.0])
                    .with(&solid_dashed)
                    .draw();
                let red = SegmentsOptions {
                    col: cols(&["red"]),
                    ..SegmentsOptions::default()
                };
                d.segments(&[1.0, 2.0], &[5.0], &[3.0, f64::NAN], &[6.0])
                    .with(&red)
                    .draw();
                let bars = ArrowsOptions {
                    code: 3,
                    angle: 90.0,
                    length: 0.1,
                    col: vec!["red".into(), ColorSpec::None],
                    ..ArrowsOptions::default()
                };
                d.arrows(&[2.0, 5.0, 7.0], &[6.0], &[3.0, 8.0, 9.0], &[9.0])
                    .with(&bars)
                    .draw();
                d.arrows(&[1.0], &[9.0], &[1.0], &[7.0]).draw();
            },
        },
        Case {
            name: "rect_polygon",
            r: "par(fg = 'darkgreen')\n\
                rect(c(1, 3, 5), 1, c(2, 4, 6), c(2, 3, 4), col = c('grey', NA),\n\
                     border = c('red', 'blue', NA), lwd = 2)\n\
                rect(7, 1, 9, 3, lty = 2)\n\
                polygon(c(1, 2, 3, NA, 5, 6, 7, NA, 8, 9, 9), c(5, 8, 5, NA, 5, 8, 5, NA, 6, 6, 9),\n\
                        col = c('orange', NA, 'cyan'), border = c(1, 2), lty = c(1, 2, 3))",
            draw: |d| {
                d.par.fg = Color::parse("darkgreen").unwrap();
                let three = RectOptions {
                    col: vec!["grey".into(), ColorSpec::None],
                    border: vec!["red".into(), "blue".into(), ColorSpec::None],
                    lwd: vec![2.0],
                    ..RectOptions::default()
                };
                d.rect(&[1.0, 3.0, 5.0], &[1.0], &[2.0, 4.0, 6.0], &[2.0, 3.0, 4.0])
                    .with(&three)
                    .draw();
                let dashed = RectOptions {
                    lty: vec![LineType::DASHED],
                    ..RectOptions::default()
                };
                d.rect(&[7.0], &[1.0], &[9.0], &[3.0]).with(&dashed).draw();
                let na = f64::NAN;
                let polys = PolygonOptions {
                    col: vec!["orange".into(), ColorSpec::None, "cyan".into()],
                    border: vec![ColorSpec::Index(1), ColorSpec::Index(2)],
                    lty: vec![LineType::SOLID, LineType::DASHED, LineType::DOTTED],
                    ..PolygonOptions::default()
                };
                d.polygon(
                    &[1.0, 2.0, 3.0, na, 5.0, 6.0, 7.0, na, 8.0, 9.0, 9.0],
                    &[5.0, 8.0, 5.0, na, 5.0, 8.0, 5.0, na, 6.0, 6.0, 9.0],
                )
                .with(&polys)
                .draw();
            },
        },
        Case {
            name: "text_vector",
            r: "text(c(2, 5, 8), c(2, 5, 8), c('a', 'bb'), col = c('red', 'blue'), cex = c(1, 1.5, 2),\n\
                     font = c(1, 2))\n\
                text(c(2, 4), 9)\n\
                text(9.5, 1, 'edge', adj = 0, xpd = TRUE)\n\
                text(3, 3, 'rot', srt = 45, adj = c(0, 1))\n\
                par(adj = 1); text(5, 7, 'right')",
            draw: |d| {
                let vec_text = TextOptions {
                    col: cols(&["red", "blue"]),
                    cex: vec![1.0, 1.5, 2.0],
                    font: vec![1, 2],
                    ..TextOptions::default()
                };
                d.text(&[2.0, 5.0, 8.0], &[2.0, 5.0, 8.0], &["a", "bb"])
                    .with(&vec_text)
                    .draw();
                let numbers: [&str; 0] = [];
                d.text(&[2.0, 4.0], &[9.0], &numbers).draw();
                let edge = TextOptions {
                    adj: Some(0.0),
                    xpd: Some(true),
                    ..TextOptions::default()
                };
                d.text(&[9.5], &[1.0], &["edge"]).with(&edge).draw();
                let rot = TextOptions {
                    adj: Some(0.0),
                    vadj: Some(1.0),
                    srt: 45.0,
                    ..TextOptions::default()
                };
                d.text(&[3.0], &[3.0], &["rot"]).with(&rot).draw();
                d.par.adj = 1.0;
                d.text(&[5.0], &[7.0], &["right"]).draw();
            },
        },
        Case {
            name: "box_regions",
            r: "par(oma = c(1, 2, 1, 1), mfrow = c(1, 2), lty = 3, col = 'darkred')\n\
                plot.new(); box(); box('figure', col = 'red')\n\
                box('inner', lty = 2, lwd = 2); box('outer', col = 'blue')",
            draw: |d| {
                d.par.oma = Sides::from([1.0, 2.0, 1.0, 1.0]);
                d.par.lty = LineType::DOTTED;
                d.par.col = Color::parse("darkred").unwrap();
                d.mfrow(1, 2);
                d.plot_new();
                d.box_().draw();
                d.box_()
                    .with(&BoxOptions {
                        which: BoxWhich::Figure,
                        col: Some("red".into()),
                        ..BoxOptions::default()
                    })
                    .draw();
                d.box_()
                    .with(&BoxOptions {
                        which: BoxWhich::Inner,
                        lty: Some(LineType::DASHED),
                        lwd: Some(2.0),
                        ..BoxOptions::default()
                    })
                    .draw();
                d.box_()
                    .with(&BoxOptions {
                        which: BoxWhich::Outer,
                        col: Some("blue".into()),
                        ..BoxOptions::default()
                    })
                    .draw();
            },
        },
        Case {
            // `plot()`'s axes take their colour, size, font and `las` from
            // `par()` (R's `localAxis()`); labels use `par(col.axis)`.
            name: "plot_axis_par",
            r: "par(fg = 'red', col.axis = 'blue', cex.axis = 1.3, font.axis = 2, las = 1)\n\
                plot(c(1, 3), c(2, 5), pch = 19, ann = FALSE)",
            draw: |d| {
                let red = Color::parse("red").unwrap();
                d.par.fg = red;
                d.par.col = red;
                d.par.col_axis = Color::parse("blue").unwrap();
                d.par.cex_axis = 1.3;
                d.par.font_axis = 2;
                d.par.las = 1;
                d.par.ann = false;
                d.plot(&[1.0, 3.0], &[2.0, 5.0]).pch(19).draw();
            },
        },
        Case {
            name: "arrows_log",
            r: "plot.new(); plot.window(c(1, 100), c(1, 10), log = 'x')\n\
                arrows(c(0, 10, -5), 2, c(50, 20, 30), c(5, 8, 9))",
            draw: |d| {
                d.plot_new();
                d.plot_window((1.0, 100.0), (1.0, 10.0)).xlog(true).draw();
                d.arrows(
                    &[0.0, 10.0, -5.0],
                    &[2.0],
                    &[50.0, 20.0, 30.0],
                    &[5.0, 8.0, 9.0],
                )
                .draw();
            },
        },
        Case {
            name: "asp",
            r: "plot.new(); plot.window(c(0, 10), c(0, 2), asp = 1); rect(0, 0, 10, 2)\n\
                plot.window(c(1, 100), c(0, 2), log = 'x', asp = 1); rect(1, 0, 100, 2, border = 'red')",
            draw: |d| {
                d.plot_new();
                let asp = PlotWindowOptions {
                    asp: Some(1.0),
                    ..PlotWindowOptions::default()
                };
                d.plot_window((0.0, 10.0), (0.0, 2.0)).with(&asp).draw();
                d.rect(&[0.0], &[0.0], &[10.0], &[2.0]).draw();
                // Ignored on a log axis, as in R.
                let log = PlotWindowOptions { xlog: true, ..asp };
                d.plot_window((1.0, 100.0), (0.0, 2.0)).with(&log).draw();
                let red = RectOptions {
                    border: cols(&["red"]),
                    ..RectOptions::default()
                };
                d.rect(&[1.0], &[0.0], &[100.0], &[2.0]).with(&red).draw();
            },
        },
    ]
}

/// Cases drawn on `plot.new(); plot.window(c(0, 10), c(0, 10))`.
fn on_window(name: &str) -> bool {
    !matches!(name, "box_regions" | "asp" | "plot_axis_par")
}

#[derive(Clone, Debug)]
struct Gs {
    fill: [f64; 3],
    stroke: [f64; 3],
    lwd: f64,
    dash: Vec<f64>,
}

#[derive(Debug)]
struct Painted {
    op: String,
    vertices: Vec<f64>,
    fill: [f64; 3],
    stroke: [f64; 3],
    lwd: f64,
    dash: Vec<f64>,
}

#[derive(Debug)]
struct Text {
    string: String,
    x: f64,
    y: f64,
    size: f64,
    fill: [f64; 3],
}

/// Painted paths (clipping paths excluded) and text items, in order, with
/// the graphics state each was drawn in.
fn extract(pdf: &str) -> (Vec<Painted>, Vec<Text>) {
    let mut painted = Vec::new();
    let mut texts = Vec::new();
    let mut gs = Gs {
        fill: [0.0; 3],
        stroke: [0.0; 3],
        lwd: 1.0,
        dash: Vec::new(),
    };
    let mut stack = Vec::new();
    let mut nums: Vec<f64> = Vec::new();
    let mut array: Option<Vec<f64>> = None;
    let mut path: Vec<f64> = Vec::new();
    let mut curved = false;
    let mut font_size = 1.0;
    let mut rest = pdf;
    while let Some(i) = rest.find("stream\n") {
        let content = &rest[i + 7..];
        let j = content.find("endstream").unwrap_or(content.len());
        let body = &content[..j];
        rest = &content[j..];
        if !body.contains(" w") && !body.contains("Tf") {
            continue; // e.g. R's ICC profile
        }
        let mut chars = body.char_indices().peekable();
        let mut token = String::new();
        let mut tokens: Vec<String> = Vec::new();
        // Tokens, with `( ... )` strings kept whole.
        while let Some((_, c)) = chars.next() {
            if c == '(' {
                let mut s = String::from("(");
                let mut depth = 1;
                for (_, c) in chars.by_ref() {
                    s.push(c);
                    match c {
                        '(' => depth += 1,
                        ')' => {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                tokens.push(s);
            } else if c.is_whitespace() || c == '[' || c == ']' {
                if !token.is_empty() {
                    tokens.push(std::mem::take(&mut token));
                }
                if c != ' ' && !c.is_whitespace() {
                    tokens.push(c.to_string());
                }
            } else {
                token.push(c);
            }
        }
        if !token.is_empty() {
            tokens.push(token);
        }
        for t in &tokens {
            if let Ok(v) = t.parse::<f64>() {
                match &mut array {
                    Some(a) => a.push(v),
                    None => nums.push(v),
                }
                continue;
            }
            let last = |k: usize| nums[nums.len() - k..].to_vec();
            match t.as_str() {
                "[" => {
                    array = Some(Vec::new());
                    continue;
                }
                "]" => {
                    gs.dash = array.take().unwrap_or_default();
                    continue;
                }
                s if s.starts_with('(') => {
                    if let Some(t) = texts.last_mut() {
                        let t: &mut Text = t;
                        t.string.push_str(&s[1..s.len() - 1]);
                    }
                    continue;
                }
                "q" => stack.push(gs.clone()),
                "Q" => gs = stack.pop().unwrap(),
                "rg" | "scn" => gs.fill = last(3).try_into().unwrap(),
                "RG" | "SCN" => gs.stroke = last(3).try_into().unwrap(),
                "g" => gs.fill = [nums[nums.len() - 1]; 3],
                "G" => gs.stroke = [nums[nums.len() - 1]; 3],
                "w" => gs.lwd = nums[nums.len() - 1],
                "Tf" => font_size = nums[nums.len() - 1],
                "Tm" => {
                    let m = last(6);
                    texts.push(Text {
                        string: String::new(),
                        x: m[4],
                        y: m[5],
                        size: font_size * m[0].hypot(m[1]),
                        fill: gs.fill,
                    });
                }
                "m" | "l" => path.extend(last(2)),
                "re" => {
                    // Corners, normalised to a positive width and height.
                    let mut n = last(4);
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
                "c" => curved = true,
                "n" => {
                    path.clear();
                    curved = false;
                }
                "S" | "s" | "f" | "F" | "f*" | "B" | "B*" | "b" | "b*" => {
                    let mut vertices = std::mem::take(&mut path);
                    // Circles are compared by operator only: R and we
                    // start them at different points.
                    if std::mem::take(&mut curved) {
                        vertices.clear();
                    }
                    let fills = !matches!(t.as_str(), "S" | "s");
                    let strokes = !matches!(t.as_str(), "f" | "F" | "f*");
                    painted.push(Painted {
                        op: t.clone(),
                        vertices,
                        fill: if fills { gs.fill } else { [0.0; 3] },
                        stroke: if strokes { gs.stroke } else { [0.0; 3] },
                        lwd: if strokes { gs.lwd } else { 0.0 },
                        dash: if strokes { gs.dash.clone() } else { Vec::new() },
                    });
                }
                _ => {}
            }
            nums.clear();
        }
    }
    (painted, texts)
}

fn close(a: &[f64], b: &[f64], tol: f64) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < tol)
}

#[test]
fn primitives_match_r() {
    if !rscript_available() {
        eprintln!("Rscript not found on PATH; skipping R cross-check.");
        return;
    }
    let dir = std::env::temp_dir();
    let cases = cases();
    let mut script = String::new();
    for c in &cases {
        let path = dir.join(format!("rs_graphics_primitives_{}.pdf", c.name));
        script.push_str(&format!(
            "pdf(file = '{}', width = {PAGE_W}, height = {PAGE_H}, compress = FALSE)\n{}{}\n\
             invisible(dev.off())\n",
            path.display(),
            if on_window(c.name) { WINDOW_R } else { "" },
            c.r
        ));
    }
    let script_path = dir.join("rs_graphics_primitives.R");
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
        let path = dir.join(format!("rs_graphics_primitives_{}.pdf", c.name));
        let latin1 = |b: &[u8]| -> String { b.iter().map(|&c| c as char).collect() };
        let (r_paths, r_texts) = extract(&latin1(&std::fs::read(&path).unwrap()));
        let mut dev = Device::new(
            PdfBackend::new(PAGE_W, PAGE_H).with_compression(false),
            Par::default(),
        );
        if on_window(c.name) {
            window(&mut dev);
        }
        (c.draw)(&mut dev);
        let (our_paths, our_texts) = extract(&latin1(&dev.into_backend().finish()));
        let name = c.name;

        assert!(
            !r_paths.is_empty() || !r_texts.is_empty(),
            "{name}: nothing from R"
        );
        assert_eq!(
            our_paths.len(),
            r_paths.len(),
            "{name}: painted path count\nours={our_paths:#?}\nr={r_paths:#?}"
        );
        for (k, (a, b)) in our_paths.iter().zip(&r_paths).enumerate() {
            let ctx = format!("{name}, path {k}:\n ours={a:?}\n r=   {b:?}");
            assert_eq!(a.op, b.op, "operator differs. {ctx}");
            assert!(
                close(&a.vertices, &b.vertices, 0.05),
                "vertices differ. {ctx}"
            );
            assert!(close(&a.fill, &b.fill, 0.005), "fill differs. {ctx}");
            assert!(close(&a.stroke, &b.stroke, 0.005), "stroke differs. {ctx}");
            // R writes widths to 2 decimals.
            assert!((a.lwd - b.lwd).abs() < 0.0051, "width differs. {ctx}");
            assert!(close(&a.dash, &b.dash, 0.005), "dash differs. {ctx}");
        }
        assert_eq!(
            our_texts.len(),
            r_texts.len(),
            "{name}: text count\nours={our_texts:#?}\nr={r_texts:#?}"
        );
        for (a, b) in our_texts.iter().zip(&r_texts) {
            let ctx = format!("{name}:\n ours={a:?}\n r=   {b:?}");
            assert_eq!(a.string, b.string, "string differs. {ctx}");
            assert!(
                (a.x - b.x).abs() < 0.05 && (a.y - b.y).abs() < 0.05,
                "origin differs. {ctx}"
            );
            assert!((a.size - b.size).abs() < 0.01, "size differs. {ctx}");
            assert!(close(&a.fill, &b.fill, 0.005), "colour differs. {ctx}");
        }
    }
}
