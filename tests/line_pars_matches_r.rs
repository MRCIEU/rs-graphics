// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks R's inline `lend`/`ljoin`/`lmitre` on every primitive
//! (`lines()`, `points()`, `segments()`, `arrows()`, `rect()`, `polygon()`,
//! `box()`, `abline()`) against a real R `pdf()` device's uncompressed
//! output: the line cap, join, mitre limit and width in effect at every
//! stroke, in order, and that they don't outlast the call. Skipped if
//! `Rscript` isn't available.

mod common;

use std::process::Command;

use rs_graphics::prelude::*;

const R_CALLS: &str = "plot.new(); plot.window(c(0, 10), c(0, 10))\n\
    lines(1:3, c(1, 5, 2), lwd = 8, lend = 'butt', ljoin = 'mitre', lmitre = 3)\n\
    points(5, 5, pch = 0, cex = 3, lwd = 4, ljoin = 'bevel')\n\
    segments(1, 8, 4, 9, lwd = 6, ljoin = 'mitre', lend = 'square')\n\
    arrows(6, 1, 9, 3, lwd = 3, ljoin = 'mitre', lmitre = 2, lend = 'butt')\n\
    rect(6, 6, 8, 8, lwd = 5, ljoin = 'bevel', lty = 2, lend = 'square')\n\
    polygon(c(1, 2, 3), c(6, 7, 6), lwd = 5, ljoin = 'mitre', lmitre = 1.5)\n\
    box(lwd = 4, ljoin = 'bevel', lend = 'butt')\n\
    abline(h = 4, lwd = 5, lend = 'butt')\n\
    lines(c(7, 8, 9), c(9, 5, 9), lwd = 6)\n";

fn ours(d: &mut Device<PdfBackend>) {
    d.plot_new();
    d.plot_window((0, 10), (0, 10)).draw();
    d.lines(&[1, 2, 3], &[1, 5, 2])
        .lwd(8)
        .lend("butt")
        .ljoin("mitre")
        .lmitre(3)
        .draw();
    d.points(&[5], &[5])
        .pch(0)
        .cex(3)
        .lwd(4)
        .ljoin("bevel")
        .draw();
    d.segments(&[1], &[8], &[4], &[9])
        .lwd(6)
        .ljoin("mitre")
        .lend("square")
        .draw();
    d.arrows(&[6], &[1], &[9], &[3])
        .lwd(3)
        .ljoin("mitre")
        .lmitre(2)
        .lend("butt")
        .draw();
    d.rect(&[6], &[6], &[8], &[8])
        .lwd(5)
        .ljoin("bevel")
        .lty("dashed")
        .lend("square")
        .draw();
    d.polygon(&[1, 2, 3], &[6, 7, 6])
        .lwd(5)
        .ljoin("mitre")
        .lmitre(1.5)
        .draw();
    d.box_().lwd(4).ljoin("bevel").lend("butt").draw();
    d.abline().h(4).lwd(5).lend("butt").draw();
    // Back to `par()`'s values.
    d.lines(&[7, 8, 9], &[9, 5, 9]).lwd(6).draw();
}

/// `(cap, join, mitre, width)` in effect at each stroking operator.
fn strokes(pdf: &str) -> Vec<(i32, i32, f64, f64)> {
    let mut out = Vec::new();
    let mut rest = pdf;
    while let Some(i) = rest.find("stream\n") {
        let body = &rest[i + 7..];
        let Some(j) = body.find("endstream") else {
            break;
        };
        let content = &body[..j];
        rest = &body[j + 9..];
        // PDF's initial state: butt caps, mitre joins, limit 10, width 1.
        let mut gs = (0, 0, 10.0, 1.0);
        let mut stack = Vec::new();
        let mut nums: Vec<f64> = Vec::new();
        for t in content.split_whitespace() {
            if let Ok(v) = t.parse::<f64>() {
                nums.push(v);
                continue;
            }
            let last = nums.last().copied();
            match t {
                "q" => stack.push(gs),
                "Q" => gs = stack.pop().unwrap(),
                "J" => gs.0 = last.unwrap() as i32,
                "j" => gs.1 = last.unwrap() as i32,
                "M" => gs.2 = last.unwrap(),
                "w" => gs.3 = last.unwrap(),
                "S" | "s" | "B" | "B*" | "b" | "b*" => out.push(gs),
                _ => {}
            }
            nums.clear();
        }
    }
    out
}

#[test]
fn inline_line_pars_match_r() {
    if !common::r_available() {
        return;
    }
    let path = std::env::temp_dir().join("rs_graphics_line_pars.pdf");
    let script = format!(
        "pdf(file = '{}', width = 6, height = 6, compress = FALSE)\n{R_CALLS}invisible(dev.off())\n",
        path.display()
    );
    let out = Command::new("Rscript")
        .args(["-e", &script])
        .output()
        .expect("run Rscript");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let r = strokes(&String::from_utf8_lossy(&std::fs::read(&path).unwrap()));

    let mut dev = Device::new(
        PdfBackend::new(6, 6).with_compression(false),
        Par::default(),
    );
    ours(&mut dev);
    let ours = strokes(&String::from_utf8_lossy(&dev.into_backend().finish()));

    assert_eq!(ours.len(), r.len(), "stroke count\nours={ours:?}\nr={r:?}");
    for (k, (a, b)) in ours.iter().zip(&r).enumerate() {
        assert!(
            a.0 == b.0 && a.1 == b.1 && (a.2 - b.2).abs() < 1e-6 && (a.3 - b.3).abs() < 0.01,
            "stroke {k}: ours (cap, join, mitre, width) = {a:?}, R = {b:?}\nours={ours:?}\nr={r:?}"
        );
    }
}

#[test]
#[should_panic(expected = "invalid value specified for graphical parameter \"lmitre\"")]
fn lmitre_below_one_panics_like_r() {
    let mut dev = Device::new(PdfBackend::new(6, 6), Par::default());
    dev.plot_new();
    dev.lines(&[1, 2], &[1, 2]).lmitre(0.5).draw();
}
