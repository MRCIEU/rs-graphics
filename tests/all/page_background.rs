// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! The page background follows the device, as R's `par("bg")` does: none
//! on PDF (R's `pdf()` is transparent) unless `with_background` is used,
//! white on SVG and the canvas colour on PNG; and a new page is a new PDF
//! page but replaces the SVG one.

use rs_graphics::backend::pdf::PdfBackend;
use rs_graphics::backend::raster::RasterBackend;
use rs_graphics::backend::svg::SvgBackend;
use rs_graphics::backend::Backend;
use rs_graphics::color::{Color, ColorSpec};
use rs_graphics::device::Device;
use rs_graphics::par::Par;
use rs_graphics::prelude::*;

/// A full-page rectangle at 6 x 4 inches.
const PAGE_RE: &str = "0.000 0.000 432.000 288.000 re";

fn pdf(backend: PdfBackend) -> String {
    let mut dev = Device::new(backend.with_compression(false), Par::default());
    dev.plot_new();
    String::from_utf8_lossy(&dev.into_backend().finish()).into_owned()
}

#[test]
fn pdf_page_is_transparent_by_default() {
    assert!(!pdf(PdfBackend::new(6.0, 4.0)).contains(PAGE_RE));
    let white = pdf(PdfBackend::new(6.0, 4.0).with_background(Color::WHITE));
    assert!(white.contains(PAGE_RE), "{white}");
}

#[test]
fn par_bg_set_by_the_caller_is_kept() {
    let par = Par {
        bg: Color::rgb(255, 255, 0),
        ..Par::default()
    };
    let mut dev = Device::new(PdfBackend::new(6.0, 4.0).with_compression(false), par);
    dev.plot_new();
    let out = String::from_utf8_lossy(&dev.into_backend().finish()).into_owned();
    assert!(out.contains(PAGE_RE));
}

#[test]
fn svg_and_png_pages_are_filled() {
    let mut dev = Device::new(SvgBackend::new(6.0, 4.0), Par::default());
    dev.plot_new();
    let svg = dev.into_backend().finish();
    assert!(svg.contains("fill=\"#FFFFFF\"") || svg.contains("fill=\"#ffffff\""), "{svg}");

    let grey = Color::rgb(200, 200, 200);
    let dev = Device::new(RasterBackend::new(1.0, 1.0, 10.0, grey), Par::default());
    assert_eq!(dev.current_par().bg, grey);
}

/// Draws a recognisable mark on page one, wraps to page two, draws another.
fn two_pages<B: Backend>(backend: B) -> B {
    let mut dev = Device::new(backend, Par::default());
    dev.plot_new();
    dev.plot_window().xlim(0.0, 1.0).ylim(0.0, 1.0).draw();
    dev.points()
        .x([0.5])
        .y([0.5])
        .with(
            &XyOptions::default()
                .pch(vec![Some(0)])
                .col(vec![ColorSpec::Index(2)]),
        )
        .draw();
    dev.plot_new();
    dev.plot_window().xlim(0.0, 1.0).ylim(0.0, 1.0).draw();
    dev.points()
        .x([0.5])
        .y([0.5])
        .with(
            &XyOptions::default()
                .pch(vec![Some(0)])
                .col(vec![ColorSpec::Index(4)]),
        )
        .draw();
    dev.into_backend()
}

#[test]
fn a_new_page_is_a_second_pdf_page_and_replaces_the_svg_one() {
    let pdf = two_pages(PdfBackend::new(6.0, 4.0).with_compression(false)).finish();
    let pdf = String::from_utf8_lossy(&pdf);
    assert!(pdf.contains("/Count 2"), "{pdf}");
    // Palette 2 (#DF536B) on page one, palette 4 (#2297E6) on page two.
    let first = pdf.find("0.8745 0.3255 0.4196 RG").expect("page one's mark");
    let second = pdf.find("0.1333 0.5922 0.9020 RG").expect("page two's mark");
    assert!(first < pdf.find("5 0 obj").unwrap() && second > pdf.find("6 0 obj").unwrap());

    // SVG holds one page: the first is discarded.
    let svg = two_pages(SvgBackend::new(6.0, 4.0)).finish().to_uppercase();
    assert!(!svg.contains("#DF536B"));
    assert!(svg.contains("#2297E6"));
}
