// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! The `Backend` trait: a minimal drawing surface that the higher-level
//! graphics engine (`crate::device`) renders onto. Implemented for SVG
//! (`svg.rs`), PDF (`pdf.rs`), and PNG rasterization (`raster.rs`), and
//! [`AnyBackend`], which wraps any of the three behind one type.
//!
//! # Coordinate convention
//! All coordinates passed to a `Backend` are in **inches**, origin at the
//! bottom-left of the page, y increasing upward. This is PDF's native
//! page-coordinate convention, so the PDF backend only scales to points;
//! SVG and the raster backend also flip y internally.
//!
//! # Why text metrics aren't part of this trait
//! Glyph *width* and *height* calculations always go through
//! `crate::afm` (the same Helvetica AFM tables R's own `pdf()`/
//! `postscript()` devices use), regardless of backend. That keeps layout
//! math (label centering, legend sizing, tick offsets, ...) identical to
//! R no matter which backend ends up rasterizing the glyphs. Only the
//! actual glyph *painting* is backend-specific, via [`Backend::draw_text`].
//!
//! The one per-device quirk is the point size itself: see
//! [`Backend::effective_font_size`].

mod any;
mod compress;
pub mod null;
mod numfmt;
pub mod pdf;
pub mod raster;
pub mod svg;

pub use any::{AnyBackend, Format};

use crate::color::Color;
use crate::font::FontFace;
use crate::geometry::{Point, Rect};
use crate::lty::LineType;
use crate::numbers::Num;

/// How the ends of a line are drawn: round, butt (flat at the end) or
/// square (flat, extended by half the line width).
///
/// From a string (`"butt".parse::<LineEnd>()` or `TryFrom<&str>`): `"round"`,
/// `"butt"` or `"square"`, or their codes as strings, `"0"` to `"2"` (R takes
/// the codes only as numbers). Anything else is an error.
///
/// This is R's `par(lend = )`.
///
/// # Examples
///
/// ```
/// # use rs_graphics::prelude::*;
/// assert_eq!("butt".parse::<LineEnd>().unwrap(), LineEnd::Butt);
/// assert!("flat".parse::<LineEnd>().is_err());
/// ```
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum LineEnd {
    /// Rounded ends (the default).
    Round,
    /// Flat ends, at the end points.
    Butt,
    /// Flat ends, extended by half the line width.
    Square,
}

impl LineEnd {
    /// R's `lend` name or code, `None` otherwise; the public routes are
    /// `FromStr`/`TryFrom<&str>`.
    pub(crate) fn parse(spec: &str) -> Option<LineEnd> {
        match spec {
            "round" | "0" => Some(LineEnd::Round),
            "butt" | "1" => Some(LineEnd::Butt),
            "square" | "2" => Some(LineEnd::Square),
            _ => None,
        }
    }
}

crate::error::from_r_string!(LineEnd, "line end", "\"round\", \"butt\" or \"square\"");

/// How the corners of a line are drawn: round, mitred (pointed) or
/// bevelled (cut off).
///
/// From a string (`"mitre".parse::<LineJoin>()` or `TryFrom<&str>`): `"round"`,
/// `"mitre"` or `"bevel"`, or their codes as strings, `"0"` to `"2"` (R takes
/// the codes only as numbers). Anything else is an error.
///
/// This is R's `par(ljoin = )`.
///
/// # Examples
///
/// ```
/// # use rs_graphics::prelude::*;
/// assert_eq!("mitre".parse::<LineJoin>().unwrap(), LineJoin::Mitre);
/// ```
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum LineJoin {
    /// Rounded corners (the default).
    Round,
    /// Pointed corners, bevelled where longer than the mitre limit.
    Mitre,
    /// Corners cut off.
    Bevel,
}

impl LineJoin {
    /// R's `ljoin` name or code, `None` otherwise; the public routes are
    /// `FromStr`/`TryFrom<&str>`.
    pub(crate) fn parse(spec: &str) -> Option<LineJoin> {
        match spec {
            "round" | "0" => Some(LineJoin::Round),
            "mitre" | "1" => Some(LineJoin::Mitre),
            "bevel" | "2" => Some(LineJoin::Bevel),
            _ => None,
        }
    }
}

crate::error::from_r_string!(LineJoin, "line join", "\"round\", \"mitre\" or \"bevel\"");

/// Everything a [`Backend`] needs to draw a line: colour, width, line type,
/// ends, corners and mitre limit.
#[derive(Clone, Debug)]
pub struct Stroke {
    /// Line colour.
    pub color: Color,
    /// Line width in R's units: 1 is 1/96 inch, 0.75 points (see
    /// [`width_pt`](Stroke::width_pt)).
    pub lwd: f64,
    /// Line type: solid, dashed, ....
    pub lty: LineType,
    /// How the line ends are drawn.
    pub lend: LineEnd,
    /// How the corners are drawn.
    pub ljoin: LineJoin,
    /// Mitre limit for mitred corners (at least 1).
    pub lmitre: f64,
}

impl Stroke {
    /// Creates a solid stroke of width `lwd` (R's units) with round ends and
    /// corners and a mitre limit of 10: R's defaults.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// use rs_graphics::backend::Stroke;
    /// let s = Stroke::simple(Color::BLACK, 2);
    /// assert_eq!(s.width_pt(), 1.5);
    /// ```
    pub fn simple(color: Color, lwd: impl Num) -> Self {
        Stroke {
            color,
            lwd: lwd.to_f64(),
            lty: LineType::SOLID,
            lend: LineEnd::Round,
            ljoin: LineJoin::Round,
            lmitre: 10.0,
        }
    }

    /// Returns the line width in points: `lwd * 0.75`, at least 0.01.
    pub fn width_pt(&self) -> f64 {
        (self.lwd * 0.75).max(0.01)
    }

    /// Returns the dash pattern in points (alternately drawn and skipped
    /// lengths, starting with a drawn one), or `None` for a solid line.
    ///
    /// Each length is the line type's, in `lwd` units, times the line width
    /// in points. Unless the line ends are butt, drawn lengths are one `lwd`
    /// shorter and gaps one `lwd` longer, so that the round or square ends
    /// don't fill the gaps (lengths below 0 become 0). A pattern whose
    /// lengths are all 0.01pt or less is solid. Blank lines are `None` too:
    /// a backend draws nothing for them (see [`LineType::BLANK`]).
    ///
    /// This is how R's `pdf()` device (`PDFSetLineTexture()`) draws `lty`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// # use rs_graphics::backend::{LineEnd, Stroke};
    /// let mut s = Stroke::simple(Color::BLACK, 1);
    /// assert_eq!(s.dash_pt(), None);
    /// s.lty = LineType::DASHED; // 4 on, 4 off; round ends
    /// assert_eq!(s.dash_pt(), Some(vec![2.25, 3.75]));
    /// s.lend = LineEnd::Butt;
    /// assert_eq!(s.dash_pt(), Some(vec![3.0, 3.0]));
    /// ```
    pub fn dash_pt(&self) -> Option<Vec<f64>> {
        self.lty.dash_lengths_pt(self.lwd, self.lend == LineEnd::Butt)
    }
}


/// How a [`Backend`] paints a run of text: colour, font face, size and
/// rotation.
///
/// There is deliberately no justification here: the device works out where
/// each string starts, using the same font metrics for every backend, and
/// passes [`Backend::draw_text`] the left end of its baseline. A backend
/// only paints a run of text from that point, optionally rotated about it,
/// which keeps text placement identical across PDF, SVG and PNG.
#[derive(Clone, Debug)]
pub struct TextStyle {
    /// Text colour.
    pub color: Color,
    /// Font face: plain, bold, italic, bold italic or Symbol.
    pub face: FontFace,
    /// Point size to paint at: `par(ps) * cex` after
    /// [`Backend::effective_font_size`] (so whole points on PDF).
    pub size_pt: f64,
    /// Rotation in degrees, counterclockwise about the anchor point,
    /// matching R's `srt` text-rotation convention.
    pub rotation_deg: f64,
}

impl TextStyle {
    /// Creates an unrotated text style.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// use rs_graphics::backend::TextStyle;
    /// let style = TextStyle::new(Color::BLACK, FontFace::Bold, 12);
    /// assert_eq!(style.rotation_deg, 0.0);
    /// ```
    pub fn new(color: Color, face: FontFace, size_pt: impl Num) -> Self {
        let size_pt = size_pt.to_f64();
        TextStyle {
            color,
            face,
            size_pt,
            rotation_deg: 0.0,
        }
    }
}

/// A drawing surface for a [`Device`](crate::device::Device): PDF, SVG or
/// PNG output.
///
/// Coordinates are in inches from the page's bottom-left corner, y
/// upwards (see the [module docs](self)). The crate's backends are
/// [`PdfBackend`](pdf::PdfBackend), [`SvgBackend`](svg::SvgBackend),
/// [`RasterBackend`](raster::RasterBackend) and [`AnyBackend`], made by
/// the device builder ([`Device::pdf`](crate::device::Device::pdf),
/// [`Device::memory`](crate::device::Device::memory), ...); code that works
/// with any of them takes a `B: Backend`. To draw somewhere else, implement
/// this trait and pass your backend to
/// [`Device::new`](crate::device::Device::new).
///
/// # Examples
///
/// A function drawing on any device:
///
/// ```
/// # use rs_graphics::prelude::*;
/// fn draw<B: Backend>(dev: &mut Device<B>) {
///     dev.plot().x([1, 2, 3]).draw();
/// }
/// let mut pdf = Device::memory("pdf").width(5).height(5).open().unwrap();
/// draw(&mut pdf);
/// let mut svg = Device::memory("svg").width(5).height(5).open().unwrap();
/// draw(&mut svg);
/// ```
pub trait Backend {
    /// Returns the page size in inches, `(width, height)`.
    fn size_inches(&self) -> (f64, f64);

    /// Starts clipping drawing to `rect`, intersected with any clip already
    /// active. Must be paired with [`Backend::pop_clip`].
    fn push_clip(&mut self, rect: Rect);
    /// Ends the clip started by the last [`Backend::push_clip`].
    fn pop_clip(&mut self);

    /// Draws a straight line from `p0` to `p1`.
    fn draw_line(&mut self, p0: Point, p1: Point, stroke: &Stroke);
    /// Draws an open line through `points`, in order.
    fn draw_polyline(&mut self, points: &[Point], stroke: &Stroke);
    /// Draws a closed polygon through `points`. `fill = None`: no fill;
    /// `stroke = None`: no border.
    fn draw_polygon(&mut self, points: &[Point], fill: Option<Color>, stroke: Option<&Stroke>);
    /// Draws a rectangle. `fill = None`: no fill; `stroke = None`: no
    /// border.
    fn draw_rect(&mut self, rect: Rect, fill: Option<Color>, stroke: Option<&Stroke>);
    /// Draws a circle of `radius` inches around `center`. `fill = None`: no
    /// fill; `stroke = None`: no border.
    fn draw_circle(
        &mut self,
        center: Point,
        radius: f64,
        fill: Option<Color>,
        stroke: Option<&Stroke>,
    );

    /// Paints `text` with the left end of its baseline at `origin`, rotated
    /// by `style.rotation_deg` anticlockwise about `origin`.
    ///
    /// The device has already worked out justification, so a backend only
    /// paints glyphs from this point.
    fn draw_text(&mut self, origin: Point, text: &str, style: &TextStyle);

    /// Returns the size of one device unit in inches: 1/72 for PDF and SVG
    /// (points), one pixel for PNG.
    ///
    /// A `pch = "."` symbol is at least one unit across, as in R.
    fn device_unit_inches(&self) -> f64;

    /// Returns the point size this device actually sets text at, given the
    /// requested size (`par(ps) * cex`).
    ///
    /// The device sends every text size through this, for measuring as well
    /// as drawing. It's the requested size for SVG and PNG, and rounded to a
    /// whole number of points for PDF.
    ///
    /// R's `pdf()` device rounds this way; its cairo and Quartz devices
    /// (`svg()`, `png()`) don't.
    fn effective_font_size(&self, requested_pt: f64) -> f64;

    /// Returns the background colour [`par(bg)`] starts as: white for SVG, the
    /// canvas colour for PNG, and transparent for PDF unless the device was
    /// opened with a [`bg`](crate::device::DeviceBuilder::bg).
    ///
    /// New pages and default `legend()` boxes are filled with [`par(bg)`].
    ///
    /// [`par(bg)`]: crate::par::Par::bg
    fn default_bg(&self) -> Color;

    /// Starts a new page.
    ///
    /// PDF keeps the finished page and starts another; SVG and PNG, which hold
    /// one page, discard what was drawn (the PNG canvas goes back to its
    /// background colour).
    fn new_page(&mut self);

    /// Returns the output so far as a finished file (what [`into_bytes`]
    /// would give), leaving the backend as newly made with the same options.
    ///
    /// This is how a device writes one file per page.
    ///
    /// [`into_bytes`]: Backend::into_bytes
    fn take_page(&mut self) -> Vec<u8>;

    /// Finishes the output and returns it: PDF or PNG bytes, or the SVG
    /// document as UTF-8.
    ///
    /// The same as each backend's own `finish()`, for code generic over
    /// `B: Backend`.
    fn into_bytes(self) -> Vec<u8>
    where
        Self: Sized;
}

#[cfg(test)]
mod smoke_tests;
