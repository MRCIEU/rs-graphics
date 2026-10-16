//! Exercises each backend directly (bypassing the not-yet-written
//! `device`/high-level plotting layers) with a handful of primitives, to
//! catch runtime panics/obviously wrong output early.

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
    // clip open/close should balance.
    assert_eq!(
        svg.matches("<g clip-path").count(),
        svg.matches("</g>").count()
    );
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
