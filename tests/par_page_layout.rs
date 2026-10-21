// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! The page layout arguments of a device's `par()` call (no R needed;
//! each is cross-checked against R elsewhere, e.g. `layout_matches_r.rs`
//! and `regions_matches_r.rs`): one call gives the same output as separate
//! calls, the layout is applied before the call's other settings (so a
//! `cex`/`mex` in the same call wins) in the order grid, `fig`, `fin`,
//! `mfg`, then `new` last (so `.fig(..).new(true)` adds to the page), and
//! `.with()` keeps it.

use rs_graphics::prelude::*;

fn panels(dev: &mut Device<PdfBackend>) {
    for i in 0..3 {
        let y = [1.0, 3.0 + i as f64, 2.0];
        dev.plot().x(&[1.0, 2.0, 3.0]).y(&y).main("Panel").draw();
    }
    dev.mtext().text("Page").side(3).outer(true).draw();
}

fn render(setup: impl Fn(&mut Device<PdfBackend>)) -> Vec<u8> {
    let mut dev = Device::new(
        PdfBackend::new(8.0, 6.0).with_compression(false),
        Par::default(),
    );
    setup(&mut dev);
    panels(&mut dev);
    dev.finish()
}

#[test]
fn one_par_call_matches_separate_calls() {
    let direct = render(|dev| {
        dev.par().mfrow(2, 2).draw();
        dev.par().oma(Sides::new(0.0, 0.0, 2.0, 0.0)).draw();
    });
    let via_par = render(|dev| dev.par().mfrow(2, 2).oma([0.0, 0.0, 2.0, 0.0]).draw());
    assert_eq!(direct, via_par);
    // Setter order doesn't matter.
    let reordered = render(|dev| dev.par().oma([0.0, 0.0, 2.0, 0.0]).mfrow(2, 2).draw());
    assert_eq!(direct, reordered);
}

#[test]
fn last_grid_in_a_call_wins() {
    let direct = render(|dev| dev.par().mfcol(2, 2).draw());
    // The last of mfrow/mfcol in a call wins.
    let last = render(|dev| dev.par().mfrow(3, 1).mfcol(2, 2).draw());
    assert_eq!(direct, last);
}

#[test]
fn settings_in_the_same_call_override_the_grid_defaults() {
    let mut dev = Device::new(PdfBackend::new(8.0, 6.0), Par::default());
    dev.par().mfrow(2, 2).draw();
    assert_eq!(dev.current_par().cex, 0.83);
    dev.par().cex(0.5).mex(1.5).mfrow(2, 2).draw();
    assert_eq!((dev.current_par().cex, dev.current_par().mex), (0.5, 1.5));
    dev.par().mfrow(3, 2).cex(0.9).draw();
    assert_eq!(dev.current_par().cex, 0.9);
}

#[test]
fn with_keeps_the_grid() {
    let opts = ParOptions::default().oma([0.0, 0.0, 2.0, 0.0]);
    let direct = render(|dev| dev.par().mfrow(2, 2).oma([0.0, 0.0, 2.0, 0.0]).draw());
    let with = render(|dev| dev.par().mfrow(2, 2).with(&opts).draw());
    assert_eq!(direct, with);
}

#[test]
#[should_panic(expected = "at least one row and one column")]
fn zero_rows_panic_at_draw() {
    let mut dev = Device::new(PdfBackend::new(8.0, 6.0), Par::default());
    dev.par().mfrow(0, 2).draw();
}

/// `new` is applied after `fig`/`fin` and `mfg` after the grid, whatever
/// the setter order: the same as separate calls in R's usual order.
#[test]
fn page_layout_is_applied_in_order() {
    let draw = |setup: &dyn Fn(&mut Device<PdfBackend>)| {
        let mut dev = Device::new(
            PdfBackend::new(8.0, 6.0).with_compression(false),
            Par::default(),
        );
        dev.plot().x(&[1, 2]).y(&[1, 2]).draw();
        setup(&mut dev);
        dev.plot().x(&[1, 2]).y(&[2, 1]).draw();
        dev.finish()
    };
    let separate = draw(&|dev| {
        dev.par().fig([0.5, 1.0, 0.0, 0.5]).draw();
        dev.par().new(true).draw();
    });
    assert_eq!(
        separate,
        draw(&|dev| dev.par().new(true).fig([0.5, 1.0, 0.0, 0.5]).draw())
    );
    let separate = draw(&|dev| {
        dev.par().fin([3, 2]).draw();
        dev.par().new(true).draw();
    });
    assert_eq!(
        separate,
        draw(&|dev| dev.par().new(true).fin([3, 2]).draw())
    );
    let separate = draw(&|dev| {
        dev.par().mfrow(2, 2).draw();
        dev.par().mfg(2, 2).draw();
    });
    assert_eq!(
        separate,
        draw(&|dev| dev.par().mfg(2, 2).mfrow(2, 2).draw())
    );
}
