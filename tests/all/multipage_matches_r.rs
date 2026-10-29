// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks multi-page PDF output against a real R `pdf()` device's
//! uncompressed output: the number of pages, and on each page every painted
//! path and text item in drawing order (see `common::pdf::assert_same`).
//! Pages start as in R: each `plot.new()` on a one-figure page, `mfrow`
//! wrapping, and `par(fig = )` without `new = TRUE`; `par(new = TRUE)` adds
//! none.
//! Skipped if `Rscript` isn't available. Also, without R: compressed pages
//! inflate to the uncompressed ones.

use crate::common;
use crate::common::{pdf, r};

use rs_graphics::backend::AnyBackend;
use rs_graphics::graphics::Graphics;
use rs_graphics::device::Device;
use rs_graphics::highlevel::PlotOptions;

const PAGE_W: f64 = 6.0;
const PAGE_H: f64 = 5.0;

type Dev = Device<AnyBackend>;

/// R: `pl(n, main)` is `plot(1:n, main = main, xlab = 'x', ylab = 'y')`.
const R_PRELUDE: &str = "pl <- function(n, main) plot(1:n, main = main, xlab = 'x', ylab = 'y')\n";

fn pl(d: &mut Dev, n: usize, main: &'static str) {
    let x: Vec<f64> = (1..=n).map(|i| i as f64).collect();
    let o = PlotOptions::default().main(main).xlab("x").ylab("y");
    d.plot().x(&x).y(&x).with(&o).draw();
}

struct Case {
    name: &'static str,
    pages: usize,
    r: &'static str,
    draw: fn(&mut Dev),
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "three_plots",
            pages: 3,
            r: "pl(3, 'one'); pl(4, 'two'); pl(5, 'three')",
            draw: |d| {
                pl(d, 3, "one");
                pl(d, 4, "two");
                pl(d, 5, "three");
            },
        },
        Case {
            name: "mfrow_wraps",
            pages: 2,
            r: "par(mfrow = c(1, 2)); pl(3, 'A'); pl(4, 'B'); pl(5, 'C')",
            draw: |d| {
                d.par().mfrow([1, 2]).draw();
                pl(d, 3, "A");
                pl(d, 4, "B");
                pl(d, 5, "C");
            },
        },
        Case {
            name: "fig_and_new",
            pages: 2,
            r: "pl(3, 'whole')\n\
                par(fig = c(0, 0.5, 0, 1)); pl(4, 'left')\n\
                par(fig = c(0.5, 1, 0, 1), new = TRUE); pl(5, 'right')",
            draw: |d| {
                pl(d, 3, "whole");
                d.par().fig([0.0, 0.5, 0.0, 1.0]).draw();
                pl(d, 4, "left");
                d.par().fig([0.5, 1.0, 0.0, 1.0]).draw();
                d.par().new(true).draw();
                pl(d, 5, "right");
            },
        },
    ]
}

#[test]
fn multipage_pdf_matches_r() {
    if !common::r_available() {
        return;
    }
    let cases = cases();
    let page = (PAGE_W, PAGE_H);
    let r_cases: Vec<_> = cases.iter().map(|c| (c.name, c.r)).collect();
    let r = r::run_r("multipage", page, R_PRELUDE, &r_cases);
    for (c, r_pdf) in cases.iter().zip(&r.pdfs) {
        let (ours_pdf, ()) = r::ours(page, c.draw);
        let name = c.name;
        let (ours, theirs) = (pdf::parse(&ours_pdf), pdf::parse(r_pdf));
        assert_eq!(theirs.pages.len(), c.pages, "{name}: R's page count");
        assert_eq!(ours.pages.len(), c.pages, "{name}: our page count");
        let text = String::from_utf8_lossy(&ours_pdf);
        assert!(
            text.contains(&format!("/Count {}", c.pages)),
            "{name}: /Count"
        );
        pdf::assert_same_pdf(name, ours, theirs, &pdf::Compare::default());
    }
}

#[test]
fn compressed_pages_inflate_to_the_uncompressed_ones() {
    let draw = |compress: bool| {
        let mut dev = Device::memory("pdf")
            .width(PAGE_W)
            .height(PAGE_H)
            .compress(compress)
            .open()
            .unwrap();
        pl(&mut dev, 3, "one");
        pl(&mut dev, 4, "two");
        dev.finish()
    };
    let (plain, packed) = (draw(false), draw(true));
    let packed_text = String::from_utf8_lossy(&packed);
    assert!(packed_text.contains("/Count 2"));
    assert_eq!(packed_text.matches("/FlateDecode").count(), 2);
    // Each compressed stream inflates to the matching uncompressed one.
    let find = |hay: &[u8], needle: &[u8], from: usize| {
        hay[from..]
            .windows(needle.len())
            .position(|w| w == needle)
            .map(|i| from + i)
    };
    let streams = |pdf: &[u8]| -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        let mut from = 0;
        while let Some(i) = find(pdf, b"/Length ", from) {
            let digits: String = pdf[i + 8..]
                .iter()
                .take_while(|c| c.is_ascii_digit())
                .map(|&c| c as char)
                .collect();
            let len: usize = digits.parse().unwrap();
            let start = find(pdf, b"stream\n", i).unwrap() + 7;
            out.push(pdf[start..start + len].to_vec());
            from = start + len;
        }
        out
    };
    let (a, b) = (streams(&plain), streams(&packed));
    assert_eq!(a.len(), 2);
    for (p, z) in a.iter().zip(&b) {
        assert_eq!(&miniz_oxide::inflate::decompress_to_vec_zlib(z).unwrap(), p);
    }
}
