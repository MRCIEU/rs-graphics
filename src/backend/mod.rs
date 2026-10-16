//! The `Backend` trait: a minimal drawing surface that the higher-level
//! graphics engine (`crate::device`) renders onto. Implemented for SVG
//! (`svg.rs`), PDF (`pdf.rs`), and PNG rasterization (`raster.rs`).
//!
//! # Coordinate convention
//! All coordinates passed to a `Backend` are in **inches**, origin at the
//! bottom-left of the page, y increasing upward. This is PDF's native
//! page-coordinate convention, so the PDF backend needs no transform at
//! all; SVG and the raster backend flip y internally.
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

mod compress;
mod numfmt;
pub mod pdf;
pub mod raster;
pub mod svg;

use crate::color::Color;
use crate::geometry::{Point, Rect};
use crate::lty::LineType;

/// Cap style for strokes (`par(lend=)`).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum LineEnd {
    Round,
    Butt,
    Square,
}

impl LineEnd {
    pub fn parse(spec: &str) -> Option<LineEnd> {
        match spec {
            "round" | "0" => Some(LineEnd::Round),
            "butt" | "1" => Some(LineEnd::Butt),
            "square" | "2" => Some(LineEnd::Square),
            _ => None,
        }
    }
}

/// Join style for strokes (`par(ljoin=)`).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum LineJoin {
    Round,
    Mitre,
    Bevel,
}

impl LineJoin {
    pub fn parse(spec: &str) -> Option<LineJoin> {
        match spec {
            "round" | "0" => Some(LineJoin::Round),
            "mitre" | "1" => Some(LineJoin::Mitre),
            "bevel" | "2" => Some(LineJoin::Bevel),
            _ => None,
        }
    }
}

/// Everything needed to stroke a path, mirroring R's graphics context
/// (`R_GE_gcontext`) stroke-relevant fields.
#[derive(Clone, Debug)]
pub struct Stroke {
    pub color: Color,
    /// R's `lwd`, in R's native line-width units (`1 lwd == 0.75 pt`, see
    /// `PostScriptSetLineWidth`/`PDF_SetLineStyle` in `devPS.c`).
    pub lwd: f64,
    pub lty: LineType,
    pub lend: LineEnd,
    pub ljoin: LineJoin,
    pub lmitre: f64,
}

impl Stroke {
    pub fn simple(color: Color, lwd: f64) -> Self {
        Stroke {
            color,
            lwd,
            lty: LineType::Pattern(&[]),
            lend: LineEnd::Round,
            ljoin: LineJoin::Round,
            lmitre: 10.0,
        }
    }

    /// Stroke width in points (`lwd * 0.75`, R's convention).
    pub fn width_pt(&self) -> f64 {
        (self.lwd * 0.75).max(0.01)
    }
}

/// The four standard font faces, matching `crate::afm::FontFace`.
pub use crate::afm::FontFace;

/// Text-painting parameters. Note there is deliberately no
/// horizontal/vertical justification field here: the `crate::device`
/// layer always resolves R's `adj`/`hadj`/`vadj`/`pos` justification
/// itself (using `crate::afm` metrics, the same tables used for layout
/// everywhere else) and passes [`Backend::draw_text`] the already-
/// resolved baseline-left origin. That keeps *all* text-layout math
/// backend-independent; a `Backend` only ever paints a left-anchored,
/// baseline-anchored run of text, optionally rotated about that anchor.
#[derive(Clone, Debug)]
pub struct TextStyle {
    pub color: Color,
    pub face: FontFace,
    /// Point size actually used, i.e. `par(ps) * cex`.
    pub size_pt: f64,
    /// Rotation in degrees, counterclockwise about the anchor point,
    /// matching R's `srt` text-rotation convention.
    pub rotation_deg: f64,
}

impl TextStyle {
    pub fn new(color: Color, face: FontFace, size_pt: f64) -> Self {
        TextStyle {
            color,
            face,
            size_pt,
            rotation_deg: 0.0,
        }
    }
}

/// A minimal 2D vector drawing surface, in inches with a bottom-left
/// origin. See the module docs for the coordinate convention.
pub trait Backend {
    /// Page/canvas size in inches.
    fn size_inches(&self) -> (f64, f64);

    /// Push a clip rectangle (intersected with any currently active
    /// clip). Must be paired with [`Backend::pop_clip`].
    fn push_clip(&mut self, rect: Rect);
    fn pop_clip(&mut self);

    fn draw_line(&mut self, p0: Point, p1: Point, stroke: &Stroke);
    fn draw_polyline(&mut self, points: &[Point], stroke: &Stroke);
    /// Draw a closed polygon. `fill = None` means no fill; `stroke =
    /// None` means no border.
    fn draw_polygon(&mut self, points: &[Point], fill: Option<Color>, stroke: Option<&Stroke>);
    fn draw_rect(&mut self, rect: Rect, fill: Option<Color>, stroke: Option<&Stroke>);
    fn draw_circle(
        &mut self,
        center: Point,
        radius: f64,
        fill: Option<Color>,
        stroke: Option<&Stroke>,
    );

    /// Draw `text` with its baseline starting at `origin` (i.e. `origin`
    /// is where the left edge of the first glyph sits on the text
    /// baseline), then rotated `rotation_deg` counterclockwise about
    /// `origin`. Callers (the `crate::device` layer) have already
    /// resolved justification/`adj` using `crate::afm` metrics, so
    /// backends only need to paint glyphs starting exactly here.
    fn draw_text(&mut self, origin: Point, text: &str, style: &TextStyle);

    /// Size of one device unit in inches (R's `pch = "."` is at least one
    /// unit across). `1/72` for vector devices (R's `pdf()`/`svg()` use
    /// big points); a pixel for raster devices.
    fn device_unit_inches(&self) -> f64 {
        1.0 / 72.0
    }

    /// The point size this device actually sets text at, given the size
    /// R would request (`par(ps) * cex`). The `crate::device` layer routes
    /// *every* text size through this - for both layout (string widths,
    /// ascent/"M" height) and drawing - mirroring how R's own devices
    /// apply their quirks inside `strWidth`/`metricInfo`/`text`.
    ///
    /// The default is the identity, which matches R's cairo and Quartz
    /// devices (`svg()`, `png()`). R's `pdf()`/`postscript()` devices round
    /// to a whole number of points, see [`pdf::PdfBackend`].
    fn effective_font_size(&self, requested_pt: f64) -> f64 {
        requested_pt
    }

    /// Whether R's counterpart device has a transparent `par("bg")` (R's
    /// `pdf()` does; `png()`/`svg()` default to white). `legend()`'s box
    /// fill defaults to `par("bg")`, so on such a device it is *not* filled
    /// by default and plot content shows through.
    fn transparent_default_bg(&self) -> bool {
        false
    }
}
