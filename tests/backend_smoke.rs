//! Exercises each backend directly (bypassing the not-yet-written
//! `device`/high-level plotting layers) with a handful of primitives, to
//! catch runtime panics/obviously wrong output early.

use rust_plots::afm::FontFace;
use rust_plots::backend::pdf::PdfBackend;
use rust_plots::backend::raster::RasterBackend;
use rust_plots::backend::svg::SvgBackend;
use rust_plots::backend::{Backend, Stroke, TextStyle};
use rust_plots::color::Color;
use rust_plots::geometry::{Point, Rect};

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
    let mut backend = PdfBackend::new(6.0, 4.0);
    exercise(&mut backend);
    let bytes = backend.finish();
    assert!(bytes.starts_with(b"%PDF-1.4"));
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("%%EOF"));
    assert!(text.contains("/Type /Catalog"));
    assert!(text.contains("BT"));
    assert!(text.contains("Tj"));
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
