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
mod numfmt;
pub mod pdf;
pub mod raster;
pub mod svg;

pub use any::{AnyBackend, Format};

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

crate::error::from_str_via_parse!(LineEnd, "line end");

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

crate::error::from_str_via_parse!(LineJoin, "line join");

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
            lty: LineType::SOLID,
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
    /// Point size to paint at: `par(ps) * cex` after
    /// [`Backend::effective_font_size`] (so whole points on PDF).
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

    /// The device's background colour, which `par("bg")` starts as: white
    /// (R's `svg()`), the canvas colour for [`raster::RasterBackend`]
    /// (R's `png(bg = )`), or transparent for [`pdf::PdfBackend`] unless
    /// set with its `with_background` (R's `pdf(bg = )`). New pages are
    /// filled with it, and `legend()` boxes by default.
    fn default_bg(&self) -> Color {
        Color::WHITE
    }

    /// Start a new page. The PDF backend keeps the finished page and starts
    /// another (one PDF page per R page); SVG, which holds a single page,
    /// discards what was drawn; elsewhere this does nothing, and the new
    /// page's background is painted over the old one instead.
    fn new_page(&mut self) {}

    /// The finished file: PDF or PNG bytes, or the SVG document as UTF-8.
    /// The same as each backend's own `finish()`, but available to code
    /// generic over `B: Backend`.
    fn into_bytes(self) -> Vec<u8>
    where
        Self: Sized;
}
