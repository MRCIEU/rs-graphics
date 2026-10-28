// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks kerning inside PDF strings against a real R `pdf()`
//! device's uncompressed output: a string with kerning pairs is a `TJ`
//! array (`[(A) 70 (V)] TJ`), one without a plain `Tj`, in all four
//! Helvetica faces, with Latin-1 characters, scaled and rotated. Compares
//! every text item (see `common::pdf::assert_same`) and its pieces and
//! kerns.
//!
//! R's lookup never applies the last kerning pair of each first character
//! (`devPS.c`: `KPend` is stored inclusive but looped over exclusively),
//! a bug we don't copy, so the strings here avoid those pairs;
//! `last_pair_of_a_character_is_kerned` checks ours on its own. Skipped if
//! `Rscript` isn't available.

use crate::common;
use crate::common::{pdf, r};

use rs_graphics::prelude::*;

const PAGE_W: f64 = 7.0;
const PAGE_H: f64 = 7.0;

/// `(x, y, label, font, cex, srt)`, drawn with `text()`.
const ITEMS: &[(f64, f64, &str, i32, f64, f64)] = &[
    (5.0, 9.5, "Toyota AVATAR", 1, 1.0, 0.0),
    (5.0, 8.7, "WAVE Yo Te", 1, 1.5, 0.0),
    (5.0, 7.9, "no pairs here", 1, 1.0, 0.0),
    (5.0, 7.1, "Toyota AVATAR", 2, 1.0, 0.0),
    (5.0, 6.3, "Toyota AVATAR", 3, 1.2, 0.0),
    (5.0, 5.5, "Toyota AVATAR", 4, 0.8, 0.0),
    (5.0, 4.7, "caf\u{e9} \u{c5}V", 1, 1.0, 0.0),
    (5.0, 3.9, "(LT) \\ Av", 1, 1.0, 0.0),
    (2.0, 2.0, "Tilted Type", 2, 1.0, 30.0),
    (8.0, 2.0, "Vertical", 1, 1.0, 90.0),
    (5.0, 0.8, "A", 1, 1.0, 0.0),
];

fn r_string(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            c if c.is_ascii() => out.push(c),
            c => out.push_str(&format!("\\u{:04x}", c as u32)),
        }
    }
    out.push('"');
    out
}

#[test]
fn kerned_strings_match_r() {
    if !common::r_available() {
        return;
    }
    let page = (PAGE_W, PAGE_H);
    let mut script = String::from("plot.new(); plot.window(c(0, 10), c(0, 10))\n");
    for &(x, y, s, font, cex, srt) in ITEMS {
        script.push_str(&format!(
            "text({x}, {y}, {}, font = {font}, cex = {cex}, srt = {srt})\n",
            r_string(s)
        ));
    }
    let r_pdf = r::run_r("kerning", page, "", &[("all", script.as_str())])
        .pdfs
        .remove(0);
    let (ours_pdf, ()) = r::ours(page, |d| {
        d.plot_new();
        d.plot_window().xlim(0, 10).ylim(0, 10).draw();
        for &(x, y, s, font, cex, srt) in ITEMS {
            d.text()
                .x([x])
                .y([y])
                .labels([s])
                .font(font)
                .cex(cex)
                .srt(srt)
                .draw();
        }
    });
    let (ours, r) = (pdf::parse(&ours_pdf).page(), pdf::parse(&r_pdf).page());
    assert!(
        r.texts.iter().filter(|t| t.pieces.len() > 1).count() >= 8,
        "too few kerned strings in R's output: {:#?}",
        r.texts
    );
    pdf::assert_same("kerning", &ours, &r, &pdf::Compare::default());
    for (a, b) in ours.texts.iter().zip(&r.texts) {
        assert_eq!(
            a.pieces, b.pieces,
            "pieces differ\n ours={a:?}\n r=   {b:?}"
        );
    }
}

/// R never applies the last kerning pair of each first character (its
/// bug); we do: `F r` is Helvetica's last pair for `F`, in "Frequency".
#[test]
fn last_pair_of_a_character_is_kerned() {
    let (ours, ()) = r::ours((PAGE_W, PAGE_H), |d| {
        d.plot_new();
        d.plot_window().xlim(0, 10).ylim(0, 10).draw();
        d.text().x([5]).y([5]).labels(["Fr"]).draw();
    });
    let t = pdf::parse(&ours).page().texts;
    assert_eq!(
        t[0].pieces,
        [
            pdf::Piece::Str("F".into()),
            pdf::Piece::Kern(45.0),
            pdf::Piece::Str("r".into())
        ]
    );
}
