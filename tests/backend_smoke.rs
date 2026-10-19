// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Exercises each backend directly (bypassing `Device`) with a handful of
//! primitives, to catch runtime panics/obviously wrong output early. Also
//! checks PDF compression, `.svgz` output, the PNG `pHYs` chunk, and that
//! PNG output doesn't depend on the thread count, and that `AnyBackend`
//! and the `Device` output helpers give the wrapped backend's bytes.

use rs_graphics::afm::FontFace;
use rs_graphics::backend::pdf::PdfBackend;
use rs_graphics::backend::raster::RasterBackend;
use rs_graphics::backend::svg::SvgBackend;
use rs_graphics::backend::{Backend, Stroke, TextStyle};
use rs_graphics::color::Color;
use rs_graphics::geometry::{Point, Rect};

fn exercise(b: &mut dyn Backend) {
    let stroke = Stroke::simple(Color::BLACK, 1.0);
    b.draw_rect(
        Rect::new(0.5, 0.5, 5.5, 3.5),
        Some(Color::WHITE),
        Some(&stroke),
    );
    b.push_clip(Rect::new(0.5, 0.5, 5.5, 3.5));
    b.draw_line(Point::new(1.0, 1.0), Point::new(5.0, 3.0), &stroke);
    b.draw_polyline(
        &[
            Point::new(1.0, 1.5),
            Point::new(2.0, 2.5),
            Point::new(3.0, 1.2),
            Point::new(4.0, 2.8),
        ],
        &stroke,
    );
    b.draw_polygon(
        &[
            Point::new(2.0, 2.0),
            Point::new(2.5, 2.5),
            Point::new(3.0, 2.0),
        ],
        Some(Color::rgb(0xDF, 0x53, 0x6B)),
        Some(&stroke),
    );
    b.draw_circle(
        Point::new(4.5, 1.0),
        0.3,
        Some(Color::rgb(0x22, 0x97, 0xE6)),
        Some(&stroke),
    );
    let text_style = TextStyle::new(Color::BLACK, FontFace::Plain, 12.0);
    b.draw_text(Point::new(1.0, 3.0), "Hello, rust-plots!", &text_style);
    let rotated = TextStyle {
        rotation_deg: 90.0,
        ..TextStyle::new(Color::BLACK, FontFace::Bold, 10.0)
    };
    b.draw_text(Point::new(0.6, 1.0), "y axis", &rotated);
    b.pop_clip();
}

#[test]
fn svg_backend_produces_well_formed_output() {
    let mut backend = SvgBackend::new(6.0, 4.0);
    exercise(&mut backend);
    let svg = backend.finish();
    assert!(svg.starts_with("<?xml"));
    assert!(svg.contains("<svg"));
    assert!(svg.contains("</svg>"));
    assert!(svg.contains("<text"));
    assert!(svg.contains("Hello, rust-plots!"));
    // Group (clip and same-style) open/close should balance.
    assert_eq!(svg.matches("<g ").count(), svg.matches("</g>").count());
}

#[test]
fn svgz_is_the_gzipped_svg() {
    let mut plain = SvgBackend::new(6.0, 4.0);
    let mut compressed = SvgBackend::new(6.0, 4.0);
    exercise(&mut plain);
    exercise(&mut compressed);
    let (svg, svgz) = (plain.finish(), compressed.finish_svgz());
    assert_eq!(&svgz[..3], &[0x1F, 0x8B, 8], "gzip magic and deflate");
    assert!(svgz.len() < svg.len());
    // Inflate the deflate data between the 10-byte header and the 8-byte
    // trailer (CRC-32 and length).
    let body = &svgz[10..svgz.len() - 8];
    let inflated = miniz_oxide::inflate::decompress_to_vec(body).expect("valid deflate");
    assert_eq!(inflated, svg.as_bytes());
    let len = u32::from_le_bytes(svgz[svgz.len() - 4..].try_into().unwrap());
    assert_eq!(len as usize, svg.len());
}

#[test]
fn pdf_backend_produces_well_formed_output() {
    let mut backend = PdfBackend::new(6.0, 4.0).with_compression(false);
    exercise(&mut backend);
    let bytes = backend.finish();
    assert!(bytes.starts_with(b"%PDF-1.4"));
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("%%EOF"));
    assert!(text.contains("/Type /Catalog"));
    assert!(text.contains("BT"));
    assert!(text.contains("Tj"));
}

/// The bytes of the (only) content stream of `pdf`, and its dictionary.
fn content_stream(pdf: &[u8]) -> (String, &[u8]) {
    let find = |needle: &[u8], from: usize| {
        from + pdf[from..]
            .windows(needle.len())
            .position(|w| w == needle)
            .expect("found")
    };
    let dict_start = find(b"4 0 obj\n", 0) + 8;
    let data_start = find(b"stream\n", dict_start) + 7;
    let dict = String::from_utf8_lossy(&pdf[dict_start..data_start]).into_owned();
    let len_at = dict.find("/Length ").expect("a /Length") + 8;
    let len: usize = dict[len_at..]
        .split(|c: char| !c.is_ascii_digit())
        .next()
        .unwrap()
        .parse()
        .unwrap();
    (dict, &pdf[data_start..data_start + len])
}

#[test]
fn pdf_backend_compresses_content_by_default() {
    let mut plain = PdfBackend::new(6.0, 4.0).with_compression(false);
    let mut compressed = PdfBackend::new(6.0, 4.0);
    exercise(&mut plain);
    exercise(&mut compressed);
    let (plain, compressed) = (plain.finish(), compressed.finish());
    assert!(compressed.len() < plain.len());
    let text = String::from_utf8_lossy(&compressed);
    assert!(text.contains("%%EOF") && text.contains("endstream"));

    let (dict, data) = content_stream(&compressed);
    assert!(dict.contains("/Filter /FlateDecode"), "{dict}");
    let inflated = miniz_oxide::inflate::decompress_to_vec_zlib(data).expect("valid zlib data");
    let (plain_dict, plain_data) = content_stream(&plain);
    assert!(!plain_dict.contains("/Filter"));
    assert_eq!(inflated, plain_data);
}

#[test]
fn raster_backend_produces_valid_png_of_expected_size() {
    let mut backend = RasterBackend::new(6.0, 4.0, 96.0, Color::WHITE);
    exercise(&mut backend);
    let png_bytes = backend.finish();
    assert_eq!(
        &png_bytes[0..8],
        &[0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n']
    );

    // A pHYs chunk records 96 dpi (3780 pixels per metre) right after IHDR.
    assert_eq!(&png_bytes[37..41], b"pHYs");
    assert_eq!(&png_bytes[41..50], &[0, 0, 0x0E, 0xC4, 0, 0, 0x0E, 0xC4, 1]);

    let decoded = tiny_skia::Pixmap::decode_png(&png_bytes).expect("valid png");
    assert_eq!(decoded.width(), (6.0f64 * 96.0).round() as u32);
    assert_eq!(decoded.height(), (4.0f64 * 96.0).round() as u32);

    // Somewhere inside the clipped rect we drew a blue-ish filled circle;
    // confirm at least one non-background pixel exists near its center.
    let cx = (4.5 * 96.0) as u32;
    let cy_from_top = ((4.0 - 1.0) * 96.0) as u32;
    let px = decoded.pixel(cx, cy_from_top).expect("in bounds");
    assert!(
        px.red() != 255 || px.green() != 255 || px.blue() != 255,
        "expected non-white pixel at circle center, got {px:?}"
    );
}

/// Draws `exercise()` plus enough shapes crossing tile boundaries to be
/// painted over several flushes' worth of tiles, with `threads` threads.
fn raster_with_threads(threads: usize) -> tiny_skia::Pixmap {
    let mut b = RasterBackend::new(6.0, 4.0, 96.0, Color::WHITE).with_threads(threads);
    exercise(&mut b);
    b.push_clip(Rect::new(0.7, 0.6, 5.3, 3.4));
    let stroke = Stroke::simple(Color::rgb(200, 0, 0), 2.0);
    for i in 0..2000 {
        let t = i as f64 * 0.618_034;
        let p = Point::new(0.2 + 5.6 * t.fract(), 0.2 + 3.6 * (t * 7.3).fract());
        b.draw_circle(
            p,
            0.02 + 0.03 * (t * 3.1).fract(),
            Some(Color::rgb(0, 0, 160)),
            Some(&stroke),
        );
    }
    b.pop_clip();
    tiny_skia::Pixmap::decode_png(&b.finish()).expect("valid png")
}

#[test]
fn raster_output_is_independent_of_thread_count() {
    let one = raster_with_threads(1);
    let two = raster_with_threads(2);
    let seven = raster_with_threads(7);
    assert!(two.data() == seven.data(), "2 and 7 threads differ");
    // Tiles only change anti-aliasing rounding at shape edges.
    let max_diff = one
        .data()
        .iter()
        .zip(two.data())
        .map(|(a, b)| a.abs_diff(*b))
        .max()
        .unwrap();
    assert!(
        max_diff <= 32,
        "1 vs 2 threads: max channel difference {max_diff}"
    );
}

/// Two pages through `Device`: text (PDF's size rounding), a legend box
/// (the default background), a new page and tiny `pch = "."` points
/// (PNG's device unit).
fn draw_two_pages<B: Backend>(dev: &mut rs_graphics::device::Device<B>) {
    use rs_graphics::prelude::*;
    let x = [1.0, 2.5, 4.0];
    let opts = PlotOptions {
        pch: vec![21],
        cex: vec![1.3],
        main: Some(Label::math("alpha^2")),
        ..PlotOptions::default()
    };
    plot(dev, &x, &x, &opts);
    let entries = [LegendEntry::point("a", "red", 46)];
    dev.legend(
        LegendPos::Keyword(LegendKeyword::TopLeft),
        &entries,
        &LegendOptions::default(),
    );
    // `pch = "."` small enough that its one-device-unit minimum applies.
    let dots = PlotOptions {
        pch: vec![46],
        cex: vec![0.5],
        ..PlotOptions::default()
    };
    plot(dev, &x, &[3.0, 1.0, 2.0], &dots);
}

#[test]
fn any_backend_output_is_the_wrapped_backends() {
    use rs_graphics::backend::{AnyBackend, Format};
    use rs_graphics::device::Device;
    use rs_graphics::par::Par;
    let render = |b: AnyBackend| {
        let mut dev = Device::new(b, Par::default());
        draw_two_pages(&mut dev);
        dev.finish()
    };
    let mut pdf = Device::pdf(5.0, 4.0);
    draw_two_pages(&mut pdf);
    assert_eq!(
        render(AnyBackend::new(Format::Pdf, 5.0, 4.0, 72.0)),
        pdf.finish()
    );

    let mut svg = Device::svg(5.0, 4.0);
    draw_two_pages(&mut svg);
    let svg = svg.into_backend().finish();
    assert_eq!(
        render(AnyBackend::new(Format::Svg, 5.0, 4.0, 72.0)),
        svg.into_bytes()
    );

    // Not 72 dpi, where a pixel is the default device unit (1/72 in).
    let mut png = Device::png(5.0, 4.0, 150.0);
    draw_two_pages(&mut png);
    assert_eq!(
        render(AnyBackend::new(Format::Png, 5.0, 4.0, 150.0)),
        png.finish()
    );

    let bg = Color::rgb(255, 255, 224);
    let custom = || PdfBackend::new(5.0, 4.0).with_background(bg);
    let mut direct = Device::new(custom(), Par::default());
    draw_two_pages(&mut direct);
    assert_eq!(render(AnyBackend::from(custom())), direct.finish());
}

#[test]
fn format_follows_the_file_extension() {
    use rs_graphics::backend::{AnyBackend, Format};
    assert_eq!(Format::from_path("a/b/plot.pdf"), Some(Format::Pdf));
    assert_eq!(Format::from_path("plot.SVG"), Some(Format::Svg));
    assert_eq!(Format::from_path("plot.v2.png"), Some(Format::Png));
    assert_eq!(Format::from_path("plot.jpg"), None);
    assert_eq!(Format::from_path("plot"), None);
    assert_eq!("Png".parse::<Format>(), Ok(Format::Png));
    let err = AnyBackend::for_path("plot.jpg", 1.0, 1.0, 72.0)
        .err()
        .unwrap();
    assert_eq!(err.value, "plot.jpg");
    assert_eq!(
        AnyBackend::for_path("x.svg", 1.0, 1.0, 72.0)
            .unwrap()
            .format(),
        Format::Svg
    );
}

#[test]
fn device_save_writes_the_finished_file() {
    use rs_graphics::device::Device;
    let dir = std::env::temp_dir().join("rs_graphics_device_save");
    std::fs::create_dir_all(&dir).unwrap();
    for name in ["out.pdf", "out.svg", "out.png"] {
        let path = dir.join(name);
        let mut a = Device::for_path(&path, 4.0, 3.0, 72.0).unwrap();
        let mut b = Device::for_path(&path, 4.0, 3.0, 72.0).unwrap();
        draw_two_pages(&mut a);
        draw_two_pages(&mut b);
        a.save(&path).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b.finish(), "{name}");
    }
}
