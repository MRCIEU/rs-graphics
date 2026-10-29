// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks `mtext()` placement (every side, inner and outer margins,
//! `line`, `at`, `adj`, `padj`, `cex`, `font`, and inline `las` 0..=3, plus
//! `par(las)` without it) against
//! a real R `pdf()` device's uncompressed output, inside a 2x2 `mfrow` page
//! with outer margins (so the 0.83 multi-panel `cex` applies): every text
//! item (see `common::pdf::assert_same`), except its clip (R clips
//! `mtext()` to the device). Skipped if `Rscript` isn't available.

use crate::common;
use crate::common::{pdf, r};

use rs_graphics::device::MtextOptions;
use rs_graphics::par::Sides;
use rs_graphics::prelude::*;

struct Case {
    text: &'static str,
    side: i32,
    line: f64,
    outer: bool,
    at: Option<f64>,
    adj: Option<f64>,
    padj: Option<f64>,
    cex: Option<f64>,
    font: Option<i32>,
    las: i32,
}

const fn case(text: &'static str, side: i32, line: f64) -> Case {
    Case {
        text,
        side,
        line,
        outer: false,
        at: None,
        adj: None,
        padj: None,
        cex: None,
        font: None,
        las: 0,
    }
}

fn cases() -> Vec<Case> {
    vec![
        case("bottom", 1, 0.0),
        case("left", 2, 1.5),
        case("top", 3, 0.5),
        case("right", 4, 2.0),
        Case { at: Some(3.0), ..case("at three", 1, 1.0) },
        Case { at: Some(4.5), adj: Some(0.0), ..case("left at", 2, 0.0) },
        Case { adj: Some(1.0), padj: Some(1.0), ..case("right top", 3, 0.0) },
        Case { cex: Some(1.5), font: Some(2), ..case("big bold", 3, 2.0) },
        Case { las: 1, ..case("las one", 2, 1.0) },
        Case { las: 1, ..case("las one r", 4, 1.0) },
        Case { las: 2, ..case("las two", 1, 1.0) },
        Case { las: 3, at: Some(2.0), ..case("las three", 3, 0.0) },
        Case { outer: true, ..case("outer bottom", 1, 0.5) },
        Case { outer: true, ..case("outer left", 2, 1.0) },
        Case { outer: true, cex: Some(1.4), font: Some(2), ..case("Outer Title", 3, 1.0) },
        Case { outer: true, at: Some(0.25), adj: Some(0.0), ..case("outer right", 4, 0.0) },
        Case { outer: true, line: -2.0, ..case("inside top", 3, 0.0) },
        Case { outer: true, las: 1, ..case("outer las", 2, 0.5) },
    ]
}

fn r_arg<T: std::fmt::Display>(name: &str, v: Option<T>) -> String {
    v.map(|v| format!(", {name} = {v}")).unwrap_or_default()
}

#[test]
fn mtext_placement_matches_r() {
    if !common::r_available() {
        return;
    }
    let cases = cases();
    let page = (8.0, 6.0);
    let mut script = String::from(
        "par(mfrow = c(2, 2), oma = c(2, 3, 4, 1.5))\n\
         plot.new(); plot.window(c(0, 10), c(0, 5))\n",
    );
    for c in &cases {
        script.push_str(&format!(
            "mtext('{}', side = {}, line = {}, outer = {}{}{}{}{}{}, las = {})\n",
            c.text,
            c.side,
            c.line,
            if c.outer { "TRUE" } else { "FALSE" },
            r_arg("at", c.at),
            r_arg("adj", c.adj),
            r_arg("padj", c.padj),
            r_arg("cex", c.cex),
            r_arg("font", c.font),
            c.las
        ));
    }
    // Inline `las` doesn't change `par(las)`; without it, `par(las)` applies.
    script.push_str("par(las = 1); mtext('par las', side = 2, line = 2.5)\n");
    // `NA` elements in the recycled vectors (`None` here): the default for
    // that label.
    script.push_str(
        "mtext(c('n1', 'n2', 'n3'), side = 3, line = 1, at = c(2, NA, 8), \
         adj = c(NA, 0, 1), padj = c(1, NA, 0))\n",
    );
    let r_pdf = r::run_r("mtext", page, "", &[("all", &script)])
        .pdfs
        .remove(0);

    let par = ParOptions::default().oma(Sides::new(2.0, 3.0, 4.0, 1.5));
    let (ours, ()) = r::ours_with(page, par, |dev| {
        dev.par().mfrow(2, 2).draw();
        dev.plot_new();
        dev.plot_window().xlim(0.0, 10.0).ylim(0.0, 5.0).draw();
        for c in &cases {
            dev.mtext()
                .text(c.text)
                .side(c.side)
                .with(
                    &MtextOptions::default()
                        .line(vec![c.line])
                        .outer(c.outer)
                        .at(c.at.into_iter().collect::<Vec<_>>())
                        .adj(c.adj.into_iter().collect::<Vec<_>>())
                        .padj(c.padj.into_iter().collect::<Vec<_>>())
                        .cex(c.cex.into_iter().map(Some).collect::<Vec<_>>())
                        .font(
                            c.font
                                .into_iter()
                                .map(|f| Some(FontFace::try_from(f).unwrap()))
                                .collect::<Vec<_>>(),
                        )
                        .las(c.las),
                )
                .draw();
        }
        assert_eq!(dev.current_par().las, 0);
        dev.par().las(1).draw();
        dev.mtext().text("par las").side(2).line(2.5).draw();
        dev.mtext()
            .text(["n1", "n2", "n3"])
            .side(3)
            .line(1)
            .at([Some(2.0), None, Some(8.0)])
            .adj([None, Some(0.0), Some(1.0)])
            .padj([Some(1.0), None, Some(0.0)])
            .draw();
    });

    let (ours, r) = (pdf::parse(&ours), pdf::parse(&r_pdf));
    assert_eq!(r.pages[0].texts.len(), cases.len() + 4);
    // R clips `mtext()` to the device even for inner margins (its `C_mtext`
    // tests `outer` on the SEXP); we clip it to the figure.
    let how = pdf::Compare {
        clip: false,
        angle: 0.01,
        ..Default::default()
    };
    pdf::assert_same_pdf("mtext", ours, r, &how);
}
