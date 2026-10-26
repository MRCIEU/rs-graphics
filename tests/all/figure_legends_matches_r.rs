// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks recorded [`Figure`]s with legends, drawn into multi-panel
//! pages with `add_figure`/`add_figure_at`, against the same R code on an
//! uncompressed R `pdf()`: `mfrow` with `add_figure_at` on a fresh device
//! (R's `plot.new(); par(mfg = )`), a figure's own `par()` not leaking into
//! the next, a shared-key panel in a `layout()`, and one figure added three
//! times to a 3x2 page (panels advance; `cex` 0.66). Compares every text
//! item (string, origin, size) and every legend box, and each
//! `FigureResult::legends` entry with R's `legend()` return value.
//! Skipped if `Rscript` isn't available.

use crate::common;

use std::process::Command;

use rs_graphics::prelude::*;

const PAGE_W: f64 = 8.0;
const PAGE_H: f64 = 6.0;

const DOSE: [f64; 6] = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
const RESPONSE: [f64; 6] = [1.8, 3.1, 3.9, 5.2, 5.8, 7.1];
const FITTED: [f64; 6] = [2.0, 3.0, 4.0, 5.0, 6.0, 7.0];

/// R definitions shared by every scenario. `key(...)` draws the legend
/// and prints its return value as `label left top w h text.x... text.y...`.
const R_PRELUDE: &str = r#"
dose <- 1:6
response <- c(1.8, 3.1, 3.9, 5.2, 5.8, 7.1)
fitted <- c(2, 3, 4, 5, 6, 7)
key <- function(label, ...) {
    l <- legend(..., legend = c("observed", "fitted"), pch = c(1, NA),
                lty = c(NA, 1), col = c("black", "red"))
    cat(label, unlist(l$rect[c("left", "top", "w", "h")]), l$text$x, l$text$y, "\n")
}
fit <- function(label, ...) {
    plot(dose, response, xlab = "x", ylab = "y", main = "Fit")
    lines(dose, fitted, col = "red")
    key(label, ...)
}
"#;

const LABELS: [&str; 2] = ["observed", "fitted"];

/// R's `key()` arguments: a point, then a line.
fn keys() -> LegendOptions {
    LegendOptions::default()
        .pch([Some(1), None])
        .lty([None, Some(LineType::SOLID)])
        .col(["black", "red"])
}

/// R's `fit()`: points, the fitted line and a legend at `pos`.
fn fit_figure(pos: LegendPos, opts: &LegendOptions) -> Figure {
    let mut fig = Figure::new();
    fig.plot()
        .x(DOSE)
        .y(RESPONSE)
        .xlab("x")
        .ylab("y")
        .main("Fit")
        .draw();
    fig.lines().x(DOSE).y(FITTED).col("red").draw();
    fig.legend().pos(pos).labels(LABELS).with(opts).draw();
    fig
}

/// A legend's layout and the `usr` of the panel it was drawn in.
type Drawn = (LegendLayout, (f64, f64, f64, f64));

struct Case {
    name: &'static str,
    /// R code after `R_PRELUDE`; each `key()` prints `name.k` with k from
    /// 0 in drawing order.
    r: &'static str,
    /// Draws the page; returns each legend's layout and the `usr` of the
    /// panel it was drawn in, in drawing order.
    draw: fn(&mut Device<PdfBackend>) -> Vec<Drawn>,
}

fn layouts(r: FigureResult, usr: (f64, f64, f64, f64)) -> Vec<Drawn> {
    r.legends.into_iter().map(|l| (l, usr)).collect()
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "mfrow_at",
            r: r#"par(mfrow = c(2, 2))
plot.new(); par(mfg = c(1, 2))
fit("mfrow_at.0", "bottomright", title = "Key")
par(mfg = c(2, 1)); op <- par(las = 1)
fit("mfrow_at.1", 1.5, 6.8)
par(op)
par(mfg = c(2, 2)); plot(dose, fitted, type = "l", main = "After")"#,
            draw: |d| {
                d.par().mfrow(2, 2).draw();
                let a = fit_figure(
                    LegendPos::Keyword(LegendKeyword::BottomRight),
                    &keys().title("Key"),
                );
                let mut out = layouts(d.add_figure_at(1, 2, &a), d.usr());
                // las = 1 is recorded in the figure, so it must not leak
                // into the next panel.
                let mut b = Figure::new();
                b.par().las(1).draw();
                b.plot()
                    .x(DOSE)
                    .y(RESPONSE)
                    .xlab("x")
                    .ylab("y")
                    .main("Fit")
                    .draw();
                b.lines().x(DOSE).y(FITTED).col("red").draw();
                b.legend()
                    .pos(LegendPos::Xy(1.5, 6.8))
                    .labels(LABELS)
                    .with(&keys())
                    .draw();
                out.extend(layouts(d.add_figure_at(2, 1, &b), d.usr()));
                d.par().mfg(2, 2).draw();
                d.plot()
                    .x(DOSE)
                    .y(FITTED)
                    .type_(PlotType::Lines)
                    .xlab("dose")
                    .ylab("fitted")
                    .main("After")
                    .draw();
                out
            },
        },
        Case {
            name: "layout_key",
            r: r#"layout(matrix(1:3, 1), widths = c(2, 2, 1))
plot(dose, response, xlab = "x", ylab = "y", main = "A")
plot(dose, fitted, type = "l", xlab = "x", ylab = "y", main = "B", col = "red")
par(mar = c(0, 0, 0, 0)); plot.new()
key("layout_key.0", "center")"#,
            draw: |d| {
                let rel = LayoutSize::Relative;
                d.layout()
                    .mat(&[[1, 2, 3]])
                    .widths([rel(2.0), rel(2.0), rel(1.0)])
                    .draw();
                let mut a = Figure::new();
                a.plot()
                    .x(DOSE)
                    .y(RESPONSE)
                    .xlab("x")
                    .ylab("y")
                    .main("A")
                    .draw();
                let mut b = Figure::new();
                b.plot()
                    .x(DOSE)
                    .y(FITTED)
                    .type_(PlotType::Lines)
                    .xlab("x")
                    .ylab("y")
                    .main("B")
                    .col("red")
                    .draw();
                let mut key = Figure::new();
                key.par().mar([0.0, 0.0, 0.0, 0.0]).draw();
                key.plot_new();
                key.legend()
                    .pos(LegendPos::Keyword(LegendKeyword::Center))
                    .labels(LABELS)
                    .with(&keys())
                    .draw();
                d.add_figure(&a);
                d.add_figure(&b);
                let r = d.add_figure(&key);
                layouts(r, d.usr())
            },
        },
        Case {
            name: "mfrow_3x2",
            r: r#"par(mfrow = c(3, 2))
fit("mfrow_3x2.0", "topleft", inset = 0.05)
fit("mfrow_3x2.1", "topleft", inset = 0.05)
fit("mfrow_3x2.2", "topleft", inset = 0.05)"#,
            draw: |d| {
                d.par().mfrow(3, 2).draw();
                let fig = fit_figure(
                    LegendPos::Keyword(LegendKeyword::TopLeft),
                    &keys().inset((0.05, 0.05)),
                );
                let mut out = Vec::new();
                for _ in 0..3 {
                    out.extend(layouts(d.add_figure(&fig), d.usr()));
                }
                out
            },
        },
    ]
}

/// One text item: string (kerned `TJ` pieces joined), origin and size.
#[derive(Debug)]
struct Text {
    string: String,
    x: f64,
    y: f64,
    size: f64,
}

/// Text items from each `BT ... ET` block. R writes `/F2 1 Tf a b c d x y
/// Tm (..) Tj` on one line (size in the matrix); we write the size in
/// `Tf` and a unit matrix, over several lines. Size = `Tf` size times the
/// matrix scale.
fn texts(pdf: &str) -> Vec<Text> {
    let mut out = Vec::new();
    let mut rest = pdf;
    while let Some(bt) = rest.find("BT\n") {
        let block_start = bt + 3;
        let Some(et) = rest[block_start..].find("ET\n") else {
            break;
        };
        let block = &rest[block_start..block_start + et];
        rest = &rest[block_start + et + 3..];

        // String: the parenthesised pieces after Tm, joined (R's kerned
        // `TJ` arrays split it).
        let Some(tm) = block.find(" Tm") else {
            continue;
        };
        let mut string = String::new();
        let mut depth = 0;
        let mut chars = block[tm + 3..].chars();
        while let Some(c) = chars.next() {
            match c {
                '\\' if depth > 0 => {
                    if let Some(n) = chars.next() {
                        string.push(n);
                    }
                }
                '(' => depth += 1,
                ')' => depth -= 1,
                _ if depth > 0 => string.push(c),
                _ => {}
            }
        }
        let tokens: Vec<&str> = block[..tm].split_whitespace().collect();
        let Some(tf) = tokens.iter().position(|&t| t == "Tf") else {
            continue;
        };
        let num = |t: &str| t.parse::<f64>().unwrap();
        let tf_size = num(tokens[tf - 1]);
        let m: Vec<f64> = tokens[tf + 1..].iter().map(|t| num(t)).collect();
        assert_eq!(m.len(), 6, "text matrix in {block:?}");
        out.push(Text {
            string,
            x: m[4],
            y: m[5],
            size: tf_size * (m[0].powi(2) + m[1].powi(2)).sqrt(),
        });
    }
    out
}

/// `[x, y_bottom, w, h]` of every rectangle (`re`), normalised so h > 0.
fn rects(pdf: &str) -> Vec<[f64; 4]> {
    let mut out = Vec::new();
    for line in pdf.lines() {
        let line = line.trim();
        if !line.ends_with(" re") {
            continue;
        }
        let n: Vec<f64> = line
            .split_whitespace()
            .filter_map(|t| t.parse().ok())
            .collect();
        if n.len() == 4 {
            out.push(if n[3] < 0.0 {
                [n[0], n[1] + n[3], n[2], -n[3]]
            } else {
                [n[0], n[1], n[2], n[3]]
            });
        }
    }
    out
}

/// Clip rectangles (`re W n`) are on lines of their own followed by `W n`;
/// those are skipped by matching only rectangles that are stroked or
/// filled. R and we write a drawn rectangle as `x y w h re` then a paint
/// operator on the next line.
fn drawn_rects(pdf: &str) -> Vec<[f64; 4]> {
    let lines: Vec<&str> = pdf.lines().map(str::trim).collect();
    let mut out = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if !line.ends_with(" re") {
            continue;
        }
        let next = lines.get(i + 1).copied().unwrap_or("");
        if next.starts_with('W') {
            continue;
        }
        out.extend(rects(line));
    }
    out
}

#[test]
fn figure_legends_in_panels_match_r() {
    if !common::r_available() {
        return;
    }
    let dir = std::env::temp_dir();
    let cases = cases();
    let mut script = String::from(R_PRELUDE);
    for c in &cases {
        script.push_str(&format!(
            "pdf(file = '{}/rs_graphics_figure_legends_{}.pdf', width = {PAGE_W}, \
             height = {PAGE_H}, compress = FALSE)\n{}\ninvisible(dev.off())\n",
            dir.display(),
            c.name,
            c.r
        ));
    }
    let path = dir.join("rs_graphics_figure_legends.R");
    std::fs::write(&path, script).unwrap();
    let out = Command::new("Rscript")
        .arg(&path)
        .output()
        .expect("run Rscript");
    assert!(
        out.status.success(),
        "Rscript failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let r_stdout = String::from_utf8_lossy(&out.stdout).into_owned();

    for c in &cases {
        let r_pdf = String::from_utf8_lossy(
            &std::fs::read(dir.join(format!("rs_graphics_figure_legends_{}.pdf", c.name))).unwrap(),
        )
        .into_owned();
        let mut dev = Device::new(
            PdfBackend::new(PAGE_W, PAGE_H).with_compression(false),
            Par::default(),
        );
        let legends = (c.draw)(&mut dev);
        let ours = String::from_utf8_lossy(&dev.finish()).into_owned();

        // Every text item, in drawing order.
        let (r_t, our_t) = (texts(&r_pdf), texts(&ours));
        assert!(
            r_t.len() > 20,
            "{}: R drew only {} text items",
            c.name,
            r_t.len()
        );
        let strings = |t: &[Text]| t.iter().map(|t| t.string.clone()).collect::<Vec<_>>();
        assert_eq!(strings(&our_t), strings(&r_t), "{}: text strings", c.name);
        for (a, b) in our_t.iter().zip(&r_t) {
            assert!(
                (a.x - b.x).abs() < 0.05
                    && (a.y - b.y).abs() < 0.05
                    && (a.size - b.size).abs() < 0.01,
                "{}: {:?} differs: ours={a:?} r={b:?}",
                c.name,
                a.string
            );
        }

        // Legend boxes: every rectangle R draws (R draws the plot frame as
        // a polyline, we as a rectangle, so ours may have extra ones).
        let our_r = drawn_rects(&ours);
        let r_rects = drawn_rects(&r_pdf);
        assert_eq!(
            r_rects.len(),
            legends.len(),
            "{}: one rectangle (the legend box) per legend in R's output",
            c.name
        );
        for b in r_rects {
            assert!(
                our_r
                    .iter()
                    .any(|a| (0..4).all(|k| (a[k] - b[k]).abs() < 0.05)),
                "{}: R's rectangle {b:?} not drawn by us: {our_r:?}",
                c.name
            );
        }

        // Each legend's return value, as R's legend() returns it.
        for (k, (layout, usr)) in legends.iter().enumerate() {
            let label = format!("{}.{k}", c.name);
            let line = r_stdout
                .lines()
                .find(|l| l.split_whitespace().next() == Some(label.as_str()))
                .unwrap_or_else(|| panic!("{label}: no return value printed by R"));
            let r: Vec<f64> = line
                .split_whitespace()
                .skip(1)
                .map(|v| v.parse().unwrap())
                .collect();
            let (tx, ty) = (
                (usr.1 - usr.0).abs() / 5000.0,
                (usr.3 - usr.2).abs() / 5000.0,
            );
            let mut flat = vec![
                (layout.left, tx),
                (layout.top, ty),
                (layout.w, tx),
                (layout.h, ty),
            ];
            flat.extend(layout.text_x.iter().map(|&v| (v, tx)));
            flat.extend(layout.text_y.iter().map(|&v| (v, ty)));
            assert_eq!(r.len(), flat.len(), "{label}: number of returned values");
            for (i, (&(a, tol), &b)) in flat.iter().zip(&r).enumerate() {
                assert!(
                    (a - b).abs() < tol,
                    "{label}: returned value {i} differs: ours={a} r={b}"
                );
            }
        }
    }
}
