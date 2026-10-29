// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks `par(pty = "s")`, `par(plt = )` and `par(fig = )` against a
//! real R `pdf()` device's uncompressed output: every page's painted paths
//! and text items in drawing order (see `common::pdf::assert_same`).
//! Skipped if `Rscript` isn't available. Also, without R: `par(fig = )`
//! without `par(new = TRUE)` starts a new page, and invalid `fig` values
//! panic.

use crate::common;
use crate::common::{pdf, r};

use rs_graphics::backend::AnyBackend;
use rs_graphics::device::Device;
use rs_graphics::highlevel::PlotOptions;
use rs_graphics::par::{ParOptions, Pty, Sides};
use rs_graphics::graphics::Graphics;

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
    size: (f64, f64),
    r: &'static str,
    setup: fn(ParOptions) -> ParOptions,
    draw: fn(&mut Dev),
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "pty_wide",
            size: (8.0, 5.0),
            r: "par(pty = 's'); pl(10, 'Square')",
            setup: |p| p.pty(Pty::Square),
            draw: |d| pl(d, 10, "Square"),
        },
        Case {
            name: "pty_tall",
            size: (4.0, 7.0),
            r: "par(pty = 's'); pl(10, 'Square')",
            setup: |p| p.pty(Pty::Square),
            draw: |d| pl(d, 10, "Square"),
        },
        Case {
            name: "plt",
            size: (7.0, 6.0),
            r: "par(plt = c(0.2, 0.9, 0.3, 0.8)); pl(10, 'plt')",
            setup: |p| p.plt(Some([0.2, 0.9, 0.3, 0.8])),
            draw: |d| pl(d, 10, "plt"),
        },
        Case {
            name: "pty_then_plt",
            size: (7.0, 6.0),
            r: "par(pty = 's', plt = c(0.1, 0.95, 0.2, 0.9)); pl(10, 'plt wins')",
            setup: |p| p.pty(Pty::Square).plt(Some([0.1, 0.95, 0.2, 0.9])),
            draw: |d| pl(d, 10, "plt wins"),
        },
        Case {
            name: "fig_new",
            size: (8.0, 6.0),
            r: "par(fig = c(0, 0.6, 0, 1)); pl(10, 'left')\n\
                par(fig = c(0.6, 1, 0.3, 1), new = TRUE); pl(5, 'right')",
            setup: |p| p,
            draw: |d| {
                d.par().fig([0.0, 0.6, 0.0, 1.0]).draw();
                pl(d, 10, "left");
                d.par().fig([0.6, 1.0, 0.3, 1.0]).draw();
                d.par().new(true).draw();
                pl(d, 5, "right");
            },
        },
        Case {
            name: "fig_oma",
            size: (7.0, 6.0),
            r: "par(oma = c(2, 3, 1, 1), fig = c(0.1, 0.9, 0.2, 0.9)); pl(10, 'oma')",
            setup: |p| p.oma(Sides::new(2.0, 3.0, 1.0, 1.0)),
            draw: |d| {
                d.par().fig([0.1, 0.9, 0.2, 0.9]).draw();
                pl(d, 10, "oma");
            },
        },
        Case {
            name: "fig_after_mfrow",
            size: (8.0, 6.0),
            r: "par(mfrow = c(2, 2)); pl(10, 'A')\n\
                par(fig = c(0.5, 1, 0, 0.5), new = TRUE); pl(5, 'inset')",
            setup: |p| p,
            draw: |d| {
                d.par().mfrow(2, 2).draw();
                pl(d, 10, "A");
                d.par().fig([0.5, 1.0, 0.0, 0.5]).draw();
                d.par().new(true).draw();
                pl(d, 5, "inset");
            },
        },
    ]
}

#[test]
fn regions_match_r() {
    if !common::r_available() {
        return;
    }
    let cases = cases();
    let r_cases: Vec<_> = cases.iter().map(|c| (c.name, c.size, c.r)).collect();
    let r = r::run_r_sized("regions", R_PRELUDE, &r_cases);
    for (c, r_pdf) in cases.iter().zip(&r.pdfs) {
        let par = (c.setup)(ParOptions::default());
        let (ours_pdf, ()) = r::ours_with(c.size, par, c.draw);
        let name = c.name;
        let theirs = pdf::parse(r_pdf);
        assert!(
            theirs.pages.iter().any(|p| !p.paths.is_empty()),
            "{name}: nothing from R"
        );
        pdf::assert_same_pdf(
            name,
            pdf::parse(&ours_pdf),
            theirs,
            &pdf::Compare::default(),
        );
    }
}

#[test]
fn fig_without_new_starts_a_new_page() {
    let mut dev = Device::memory("pdf")
        .width(6.0)
        .height(4.0)
        .compress(false)
        .open()
        .unwrap();
    pl(&mut dev, 10, "first");
    dev.par().fig([0.5, 1.0, 0.5, 1.0]).draw();
    pl(&mut dev, 3, "second");
    // Two pages: "first" on page one, "second" on page two.
    let pages = pdf::parse(&dev.finish()).pages;
    assert_eq!(pages.len(), 2);
    let has = |k: usize, s: &str| pages[k].texts.iter().any(|t| t.string == s);
    assert!(has(0, "first") && !has(0, "second"));
    assert!(has(1, "second") && !has(1, "first"));
}

#[test]
#[should_panic(expected = "invalid value specified for graphical parameter \"fig\"")]
fn fig_must_be_ordered_fractions() {
    let mut dev = Device::memory("pdf").width(6.0).height(4.0).open().unwrap();
    dev.par().fig([0.5, 0.2, 0.0, 1.0]).draw();
}
