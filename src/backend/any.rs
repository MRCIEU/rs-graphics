// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! [`AnyBackend`]: one type for all three output formats, chosen at run
//! time (e.g. from a command-line option), so drawing code needs no
//! generic `B`. [`Device::format`](crate::device::Device::format) and
//! [`Device::pdf`](crate::device::Device::pdf) & co. open devices on it.

use super::null::NullBackend;
use super::pdf::PdfBackend;
use super::raster::RasterBackend;
use super::svg::SvgBackend;
use super::{Backend, Stroke, TextStyle};
use crate::color::Color;
use crate::geometry::{Point, Rect};

/// An output format: PDF, SVG or PNG.
///
/// Strings work too (`"pdf"`, `"svg"`, `"png"`, ignoring case), so
/// `Device::format("png", file)` works.
///
/// # Examples
///
/// ```
/// # use rs_graphics::prelude::*;
/// assert_eq!("PNG".parse::<Format>().unwrap(), Format::Png);
/// assert!("jpeg".parse::<Format>().is_err());
/// ```
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Format {
    /// PDF, a vector format.
    Pdf,
    /// SVG, a vector format for the web.
    Svg,
    /// PNG, a raster (pixel) format.
    Png,
}

impl Format {
    /// `"pdf"`, `"svg"` or `"png"`, ignoring case; `None` otherwise. The
    /// public routes are `FromStr`/`TryFrom<&str>`.
    pub(crate) fn parse(name: &str) -> Option<Format> {
        match name.to_ascii_lowercase().as_str() {
            "pdf" => Some(Format::Pdf),
            "svg" => Some(Format::Svg),
            "png" => Some(Format::Png),
            _ => None,
        }
    }
}

crate::error::from_r_string!(Format, "output format", "\"pdf\", \"svg\" or \"png\"");

/// A PDF, SVG or PNG backend behind one type, for a format chosen at run
/// time.
///
/// Every [`Backend`] method is forwarded, so the output is byte for byte
/// that of the wrapped backend. Every device the builder opens
/// ([`Device::pdf`](crate::device::Device::pdf),
/// [`Device::format`](crate::device::Device::format),
/// [`Device::memory`](crate::device::Device::memory), ...) is a
/// `Device<AnyBackend>`, so code taking one works with any format.
///
/// # Examples
///
/// ```
/// # use rs_graphics::prelude::*;
/// fn scatter(dev: &mut Device<AnyBackend>) {
///     dev.plot().x([1, 2, 3]).draw();
/// }
/// let mut dev = Device::memory("png").res(150).threads(2).open().unwrap();
/// scatter(&mut dev);
/// assert_eq!(dev.into_backend().format(), Some(Format::Png));
/// ```
pub enum AnyBackend {
    /// A PDF backend.
    Pdf(PdfBackend),
    /// An SVG backend.
    Svg(SvgBackend),
    /// A PNG backend.
    Png(RasterBackend),
    /// The null backend: no output, PDF's measurements (see
    /// [`Device::null`](crate::device::Device::null)).
    Null(NullBackend),
}

impl AnyBackend {
    /// Returns the wrapped backend's format, or `None` for the null
    /// backend, which has no output.
    pub fn format(&self) -> Option<Format> {
        match self {
            AnyBackend::Pdf(_) => Some(Format::Pdf),
            AnyBackend::Svg(_) => Some(Format::Svg),
            AnyBackend::Png(_) => Some(Format::Png),
            AnyBackend::Null(_) => None,
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

impl From<NullBackend> for AnyBackend {
    fn from(b: NullBackend) -> Self {
        AnyBackend::Null(b)
    }
}

/// Calls `$call` on whichever backend is wrapped.
macro_rules! forward {
    ($self:expr, $b:ident => $call:expr) => {
        match $self {
            AnyBackend::Pdf($b) => $call,
            AnyBackend::Svg($b) => $call,
            AnyBackend::Png($b) => $call,
            AnyBackend::Null($b) => $call,
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
    fn take_page(&mut self) -> Vec<u8> {
        forward!(self, b => b.take_page())
    }
    fn into_bytes(self) -> Vec<u8> {
        forward!(self, b => b.into_bytes())
    }
}
