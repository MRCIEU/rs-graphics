// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks every plotting symbol - `pch = 0:25` plus character `pch`
//! (`"."`, letters, punctuation) - against a real R `pdf()` device's
//! uncompressed output. Each scenario draws a chart of all the symbols (in
//! the spirit of the classic "pch chart") with a different `col`/`bg`/
//! `cex`/`lwd`/`par(cex)`. Compares every painted path and text item
//! (character symbols) in drawing order, so also the order of a symbol's
//! parts (see `common::pdf::assert_same`). Skipped if `Rscript` isn't
//! available.

use crate::common;
use crate::common::{pdf, r};

use rs_graphics::prelude::*;

const PAGE_W: f64 = 6.0;
const PAGE_H: f64 = 4.0;
const CHARS: &[char] = &['.', 'a', 'M', '+', '*', '#', 'o', '%'];

/// `(x, y)` user coordinates of `pch = i` in the chart: four rows of seven,
/// with character symbols on a fifth row.
fn pch_pos(i: usize) -> (f64, f64) {
    ((i % 7) as f64 + 1.0, 5.0 - (i / 7) as f64)
}

fn char_pos(i: usize) -> (f64, f64) {
    (i as f64 + 0.5, 1.0)
}

struct Case {
    name: &'static str,
    /// R colour names (`None` = `NA` for `bg`).
    col: &'static str,
    bg: Option<&'static str>,
    cex: f64,
    lwd: f64,
    par_cex: f64,
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "defaults",
            col: "black",
            bg: None,
            cex: 1.0,
            lwd: 1.0,
            par_cex: 1.0,
        },
        Case {
            name: "chart_blue_red_cex2",
            col: "blue",
            bg: Some("red"),
            cex: 2.0,
            lwd: 1.0,
            par_cex: 1.0,
        },
        Case {
            name: "small_thick",
            col: "darkgreen",
            bg: Some("yellow"),
            cex: 0.7,
            lwd: 3.0,
            par_cex: 1.0,
        },
        Case {
            name: "par_cex",
            col: "purple",
            bg: Some("orange"),
            cex: 1.0,
            lwd: 2.0,
            par_cex: 1.5,
        },
    ]
}

fn r_code(c: &Case) -> String {
    let xs: Vec<String> = (0..26).map(|i| pch_pos(i).0.to_string()).collect();
    let ys: Vec<String> = (0..26).map(|i| pch_pos(i).1.to_string()).collect();
    let cx: Vec<String> = (0..CHARS.len()).map(|i| char_pos(i).0.to_string()).collect();
    let chars: Vec<String> = CHARS.iter().map(|c| format!("'{c}'")).collect();
    let bg = c.bg.map_or("NA".to_string(), |b| format!("'{b}'"));
    format!(
        "par(mar = c(0, 0, 0, 0), cex = {pc})\n\
         plot.new(); plot.window(c(0, 8), c(0, 6))\n\
         points(c({xs}), c({ys}), pch = 0:25, col = '{col}', bg = {bg}, cex = {cex}, lwd = {lwd})\n\
         points(c({cx}), rep(1, {nc}), pch = c({chars}), col = '{col}', cex = {cex})",
        pc = c.par_cex,
        xs = xs.join(", "),
        ys = ys.join(", "),
        col = c.col,
        cex = c.cex,
        lwd = c.lwd,
        cx = cx.join(", "),
        nc = CHARS.len(),
        chars = chars.join(", "),
    )
}

fn draw_ours(c: &Case, dev: &mut Device<PdfBackend>) {
    dev.par().mar([0, 0, 0, 0]).cex(c.par_cex).draw();
    dev.plot_new();
    dev.plot_window().xlim(0, 8).ylim(0, 6).draw();
    let bg =
        c.bg.map_or(ColorSpec::None, |b| ColorSpec::try_from(b).unwrap());
    let (xs, ys): (Vec<f64>, Vec<f64>) = (0..26).map(pch_pos).unzip();
    dev.points()
        .x(&xs)
        .y(&ys)
        .pch((0..26).collect::<Vec<i32>>())
        .col(c.col)
        .bg(bg)
        .cex(c.cex)
        .lwd(c.lwd)
        .draw();
    let xs: Vec<f64> = (0..CHARS.len()).map(|i| char_pos(i).0).collect();
    dev.points()
        .x(&xs)
        .y(vec![1.0; CHARS.len()])
        .pch(CHARS)
        .col(c.col)
        .cex(c.cex)
        .draw();
}

// -------------------------------------------------------------------
// A minimal PDF content-stream interpreter: just enough graphics state to
// describe what each painting operator puts on the page.
// -------------------------------------------------------------------

#[test]
fn every_pch_matches_r() {
    if !common::r_available() {
        return;
    }
    let cases = cases();
    let page = (PAGE_W, PAGE_H);
    let codes: Vec<String> = cases.iter().map(r_code).collect();
    let r_cases: Vec<_> = cases
        .iter()
        .zip(&codes)
        .map(|(c, r)| (c.name, r.as_str()))
        .collect();
    let r = r::run_r("pch", page, "", &r_cases);
    let how = pdf::Compare::default();
    for (c, r_pdf) in cases.iter().zip(&r.pdfs) {
        let (ours_pdf, ()) = r::ours(page, |dev| draw_ours(c, dev));
        pdf::assert_same_pdf(c.name, pdf::parse(&ours_pdf), pdf::parse(r_pdf), &how);
    }
}
