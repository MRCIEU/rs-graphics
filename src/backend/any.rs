//! [`AnyBackend`]: one type for all three output formats, chosen at run
//! time (e.g. from a file name), so drawing code needs no generic `B`.

use std::path::Path;

use super::pdf::PdfBackend;
use super::raster::RasterBackend;
use super::svg::SvgBackend;
use super::{Backend, Stroke, TextStyle};
use crate::color::Color;
use crate::error::ParseValueError;
use crate::geometry::{Point, Rect};

/// An output format.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Format {
    Pdf,
    Svg,
    Png,
}

impl Format {
    /// `"pdf"`, `"svg"` or `"png"`, ignoring case.
    pub fn parse(extension: &str) -> Option<Format> {
        match extension.to_ascii_lowercase().as_str() {
            "pdf" => Some(Format::Pdf),
            "svg" => Some(Format::Svg),
            "png" => Some(Format::Png),
            _ => None,
        }
    }

    /// The format named by `path`'s extension (`plot.PDF` is PDF).
    pub fn from_path(path: impl AsRef<Path>) -> Option<Format> {
        path.as_ref()
            .extension()
            .and_then(|e| e.to_str())
            .and_then(Format::parse)
    }
}

crate::error::from_str_via_parse!(Format, "output format");

/// A [`PdfBackend`], [`SvgBackend`] or [`RasterBackend`] (PNG) behind one
/// type. Every [`Backend`] method is forwarded, so output is byte for
/// byte that of the wrapped backend. To set backend options (a PDF
/// background, PNG threads, ...), build the backend and convert it with
/// `AnyBackend::from`.
pub enum AnyBackend {
    Pdf(PdfBackend),
    Svg(SvgBackend),
    Png(RasterBackend),
}

impl AnyBackend {
    /// A `width_in` x `height_in` inch page in `format`. `dpi` is used
    /// only for PNG, which gets a white background (R's `png()` default).
    pub fn new(format: Format, width_in: f64, height_in: f64, dpi: f64) -> Self {
        match format {
            Format::Pdf => AnyBackend::Pdf(PdfBackend::new(width_in, height_in)),
            Format::Svg => AnyBackend::Svg(SvgBackend::new(width_in, height_in)),
            Format::Png => {
                AnyBackend::Png(RasterBackend::new(width_in, height_in, dpi, Color::WHITE))
            }
        }
    }

    /// As [`AnyBackend::new`], with the format taken from `path`'s
    /// extension (`.pdf`, `.svg` or `.png`). Fails for any other
    /// extension.
    pub fn for_path(
        path: impl AsRef<Path>,
        width_in: f64,
        height_in: f64,
        dpi: f64,
    ) -> Result<Self, ParseValueError> {
        let path = path.as_ref();
        let format = Format::from_path(path).ok_or_else(|| {
            ParseValueError::new(
                "output format (use .pdf, .svg or .png)",
                &path.to_string_lossy(),
            )
        })?;
        Ok(AnyBackend::new(format, width_in, height_in, dpi))
    }

    /// The wrapped backend's format.
    pub fn format(&self) -> Format {
        match self {
            AnyBackend::Pdf(_) => Format::Pdf,
            AnyBackend::Svg(_) => Format::Svg,
            AnyBackend::Png(_) => Format::Png,
        }
    }
}

impl From<PdfBackend> for AnyBackend {
    fn from(b: PdfBackend) -> Self {
        AnyBackend::Pdf(b)
    }
}

impl From<SvgBackend> for AnyBackend {
    fn from(b: SvgBackend) -> Self {
        AnyBackend::Svg(b)
    }
}

impl From<RasterBackend> for AnyBackend {
    fn from(b: RasterBackend) -> Self {
        AnyBackend::Png(b)
    }
}

/// Calls `$call` on whichever backend is wrapped.
macro_rules! forward {
    ($self:expr, $b:ident => $call:expr) => {
        match $self {
            AnyBackend::Pdf($b) => $call,
            AnyBackend::Svg($b) => $call,
            AnyBackend::Png($b) => $call,
        }
    };
}

impl Backend for AnyBackend {
    fn size_inches(&self) -> (f64, f64) {
        forward!(self, b => b.size_inches())
    }
    fn push_clip(&mut self, rect: Rect) {
        forward!(self, b => b.push_clip(rect))
    }
    fn pop_clip(&mut self) {
        forward!(self, b => b.pop_clip())
    }
    fn draw_line(&mut self, p0: Point, p1: Point, stroke: &Stroke) {
        forward!(self, b => b.draw_line(p0, p1, stroke))
    }
    fn draw_polyline(&mut self, points: &[Point], stroke: &Stroke) {
        forward!(self, b => b.draw_polyline(points, stroke))
    }
    fn draw_polygon(&mut self, points: &[Point], fill: Option<Color>, stroke: Option<&Stroke>) {
        forward!(self, b => b.draw_polygon(points, fill, stroke))
    }
    fn draw_rect(&mut self, rect: Rect, fill: Option<Color>, stroke: Option<&Stroke>) {
        forward!(self, b => b.draw_rect(rect, fill, stroke))
    }
    fn draw_circle(
        &mut self,
        center: Point,
        radius: f64,
        fill: Option<Color>,
        stroke: Option<&Stroke>,
    ) {
        forward!(self, b => b.draw_circle(center, radius, fill, stroke))
    }
    fn draw_text(&mut self, origin: Point, text: &str, style: &TextStyle) {
        forward!(self, b => b.draw_text(origin, text, style))
    }
    fn device_unit_inches(&self) -> f64 {
        forward!(self, b => b.device_unit_inches())
    }
    fn effective_font_size(&self, requested_pt: f64) -> f64 {
        forward!(self, b => b.effective_font_size(requested_pt))
    }
    fn default_bg(&self) -> Color {
        forward!(self, b => b.default_bg())
    }
    fn new_page(&mut self) {
        forward!(self, b => b.new_page())
    }
    fn into_bytes(self) -> Vec<u8> {
        forward!(self, b => b.into_bytes())
    }
}
