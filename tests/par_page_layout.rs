// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! `dev.par().mfrow(..)` / `.mfcol(..)` (no R needed): the same output as
//! `Device::mfrow`/`mfcol` (themselves cross-checked against R in
//! `layout_matches_r.rs`), applied before the call's other settings so a
//! `cex`/`mex` in the same call wins, and kept by `.with()`.

use rs_graphics::prelude::*;

fn panels(dev: &mut Device<PdfBackend>) {
    for i in 0..3 {
        let y = [1.0, 3.0 + i as f64, 2.0];
        dev.plot(&[1.0, 2.0, 3.0], &y).main("Panel").draw();
    }
    dev.mtext("Page", 3).outer(true).draw();
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
fn par_mfrow_matches_device_mfrow() {
    let direct = render(|dev| {
        dev.mfrow(2, 2);
        dev.par.oma = Sides::new(0.0, 0.0, 2.0, 0.0);
    });
    let via_par = render(|dev| dev.par().mfrow(2, 2).oma([0.0, 0.0, 2.0, 0.0]).draw());
    assert_eq!(direct, via_par);
    // Setter order doesn't matter.
    let reordered = render(|dev| dev.par().oma([0.0, 0.0, 2.0, 0.0]).mfrow(2, 2).draw());
    assert_eq!(direct, reordered);
}

#[test]
fn par_mfcol_matches_device_mfcol() {
    let direct = render(|dev| dev.mfcol(2, 2));
    let via_par = render(|dev| dev.par().mfcol(2, 2).draw());
    assert_eq!(direct, via_par);
    // The last of mfrow/mfcol in a call wins.
    let last = render(|dev| dev.par().mfrow(3, 1).mfcol(2, 2).draw());
    assert_eq!(direct, last);
}

#[test]
fn settings_in_the_same_call_override_the_grid_defaults() {
    let mut dev = Device::pdf(8.0, 6.0);
    dev.par().mfrow(2, 2).draw();
    assert_eq!(dev.par.cex, 0.83);
    dev.par().cex(0.5).mex(1.5).mfrow(2, 2).draw();
    assert_eq!((dev.par.cex, dev.par.mex), (0.5, 1.5));
    dev.par().mfrow(3, 2).cex(0.9).draw();
    assert_eq!(dev.par.cex, 0.9);
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
    let mut dev = Device::pdf(8.0, 6.0);
    dev.par().mfrow(0, 2).draw();
}
