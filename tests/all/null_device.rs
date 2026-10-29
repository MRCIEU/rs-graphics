// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! `Device::null()` (R's `pdf(NULL)`): the same values and errors as a PDF
//! device of the same size, nothing drawn, no file. (No R needed.)

use rs_graphics::prelude::*;

/// What a device works out for a few calls: the legend's box, an axis's
/// ticks, a label's width, the coordinates and the plot region.
fn measure<B: Backend>(dev: &mut Device<B>) -> String {
    dev.par().mfrow(1, 2).cex(1.3).draw();
    dev.plot()
        .x([1, 10, 100])
        .y([3, 1, 2])
        .xlog(true)
        .main("Main")
        .draw();
    let legend = dev
        .legend()
        .pos("topright")
        .labels(["first", "second label"])
        .pch(19)
        .title("A title")
        .draw();
    let ticks = dev.axis().side(3).draw();
    let width = dev.strwidth().s("Some text").cex(1.7).units("inches").get();
    let height = dev.strheight().s("x^2").units("figure").get();
    dev.hist().x([1.0, 2.0, 2.5, 3.0, 7.5]).draw();
    format!(
        "{legend:?}\n{ticks:?}\n{width}\n{height}\n{:?}\n{:?}\n{:?}",
        dev.usr(),
        dev.plot_region(),
        dev.current_par().bg
    )
}

#[test]
fn measures_as_a_pdf_device() {
    let mut pdf = Device::memory("pdf").width(8).height(5).open().unwrap();
    let mut null = Device::null().width(8).height(5).open().unwrap();
    assert_eq!(measure(&mut null), measure(&mut pdf));
    // PDF rounds text to whole points; an SVG device doesn't, so a legend
    // at `cex(1.3)` comes out differently there.
    let mut svg = Device::memory("svg").width(8).height(5).open().unwrap();
    assert_ne!(measure(&mut null), measure(&mut svg));
}

#[test]
fn fails_where_a_pdf_device_fails() {
    let mut pdf = Device::memory("pdf").width(1.5).height(1.5).open().unwrap();
    let mut null = Device::null().width(1.5).height(1.5).open().unwrap();
    let e = null.try_plot_new().unwrap_err();
    assert_eq!(e, pdf.try_plot_new().unwrap_err());
    assert_eq!(e.kind(), ErrorKind::DoesNotFit);
}

#[test]
fn draws_and_writes_nothing() {
    let mut dev = Device::null().open().unwrap();
    assert_eq!(dev.din(), (7.0, 7.0));
    for _ in 0..3 {
        dev.plot().x(1..=1000).draw();
    }
    assert_eq!(dev.into_backend().format(), None);

    let mut dev = Device::null().open().unwrap();
    dev.plot().x([1, 2]).draw();
    assert!(dev.finish().is_empty());
    let dev = Device::null().open().unwrap();
    assert!(dev.close().is_ok(), "closing is R's dev.off()");
}

#[test]
fn takes_pdfs_options() {
    let dev = Device::null()
        .width(10)
        .height(5)
        .units("cm")
        .pointsize(9)
        .bg("ivory")
        .open()
        .unwrap();
    let (w, h) = dev.din();
    assert!((w - 10.0 / 2.54).abs() < 1e-12 && (h - 5.0 / 2.54).abs() < 1e-12);
    assert_eq!(dev.start_ps(), 9.0);
    assert_eq!(dev.current_par().bg, Color::try_from("ivory").unwrap());
    // Transparent by default, as PDF's.
    let dev = Device::null().open().unwrap();
    assert_eq!(dev.current_par().bg.a, 0);
    // Pixels need a PNG device.
    let e = Device::null().units("px").open().err().unwrap();
    assert_eq!(e.kind(), std::io::ErrorKind::InvalidInput);
}
