// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! The PDF backend, [`PdfBackend`]: a small hand-written PDF writer.
//!
//! It uses the four standard Helvetica faces (`Helvetica`, `-Bold`,
//! `-Oblique`, `-BoldOblique`), plus `Symbol` when plotmath needs it, which
//! every PDF viewer has, so no fonts are embedded (as in R's `pdf()`), and
//! text is measured with the same Adobe font metrics used for layout
//! everywhere else (`crate::afm`).
//!
//! PDF's native page coordinate system is already "inches (well, points)
//! from the bottom-left, y-up", matching this crate's internal `Backend`
//! convention exactly (see `backend/mod.rs`), so this backend applies no
//! coordinate flip - only a inches-to-points scale (`1 in = 72 pt`).
//!
//! Like R's `pdf()`, each new page the device starts (each `plot_new()`
//! on a one-figure page, a multi-figure page wrapping round, `par(fig = )`
//! without `new = TRUE`) is a new PDF page, sharing the font objects.
//! Like R's `pdf()` (`compress = TRUE`), each page's content stream is
//! Flate-compressed by default; see
//! [`DeviceBuilder::compress`](crate::device::DeviceBuilder::compress).
//!
//! Semi-transparent colours work as in R's `pdf()` (PDF 1.4 transparency):
//! each alpha used gets an `ExtGState` resource, `/ca` for fills and text
//! or `/CA` for strokes, selected with `gs` when the alpha changes. Output
//! with only opaque colours has no such resources and is unchanged.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use super::compress::{DEFLATE_LEVEL, zlib_compress};
use super::numfmt::push_fixed;
use super::{Backend, LineEnd, LineJoin, Stroke, TextStyle};
use crate::afm;
use crate::font::FontFace;
use crate::color::Color;
use crate::geometry::{Point, Rect};
use crate::lty::LineType;
use crate::numbers::Num;

/// A backend writing PDF, using the standard Helvetica (and Symbol) fonts
/// that every PDF viewer has, so no fonts are embedded.
///
/// Pages are kept as they are finished, so one file holds them all. Text
/// sizes are rounded to whole points and the page is transparent by
/// default, as in R's `pdf()` device. Devices from
/// [`Device::pdf`](crate::device::Device::pdf) and `Device::memory("pdf")`
/// draw on one. See the [module docs](self).
///
/// # Examples
///
/// ```
/// # use rs_graphics::prelude::*;
/// let mut dev = Device::memory("pdf").width(6).height(4).bg("white").open().unwrap();
/// dev.plot().x([1, 2, 3]).draw();
/// assert!(dev.finish().starts_with(b"%PDF"));
/// ```
pub struct PdfBackend {
    width_in: f64,
    height_in: f64,
    content: String,
    clip_depth: u32,
    last_fill: Option<Color>,
    last_stroke_color: Option<Color>,
    last_line: LineState,
    /// The fill and stroke alpha in effect (`None`: unknown, after a `Q`
    /// once some alpha has been set). PDF's default is opaque.
    last_fill_alpha: Option<u8>,
    last_stroke_alpha: Option<u8>,
    /// The fill (`/ca`) and stroke (`/CA`) alphas used anywhere, each an
    /// `ExtGState` resource (`/Ga<alpha>`, `/GA<alpha>`); empty for opaque
    /// output, which then has no such resources.
    fill_alphas: BTreeSet<u8>,
    stroke_alphas: BTreeSet<u8>,
    compress: bool,
    background: Color,
    /// Finished pages' content streams; `content` is the current page.
    pages: Vec<String>,
    /// Whether a page has been started (by the first `new_page()`).
    started: bool,
    /// Whether any text used the Symbol font, which then gets a font
    /// resource (`/F5`); other output is unchanged.
    uses_symbol: bool,
}

/// The line-style parameters last written to the content stream (`None`:
/// unknown), so each is written only when it changes, as R's `pdf()` does.
#[derive(Default)]
struct LineState {
    width: Option<f64>,
    cap: Option<u8>,
    join: Option<u8>,
    mitre: Option<f64>,
    dash: Option<Vec<f64>>,
}

impl PdfBackend {
    /// Creates a PDF backend for a `width_in` x `height_in` inch page, with a
    /// transparent background and compressed content.
    pub(crate) fn new(width_in: impl Num, height_in: impl Num) -> Self {
        let (width_in, height_in) = (width_in.to_f64(), height_in.to_f64());
        PdfBackend {
            width_in,
            height_in,
            content: String::new(),
            clip_depth: 0,
            last_fill: None,
            last_stroke_color: None,
            last_line: LineState::default(),
            last_fill_alpha: Some(255),
            last_stroke_alpha: Some(255),
            fill_alphas: BTreeSet::new(),
            stroke_alphas: BTreeSet::new(),
            compress: true,
            background: Color::TRANSPARENT,
            pages: Vec::new(),
            started: false,
            uses_symbol: false,
        }
    }

    /// Sets the page background; transparent by default.
    ///
    /// This is R's `pdf(bg = )`.
    pub(crate) fn with_background(mut self, background: Color) -> Self {
        self.background = background;
        self
    }

    /// Sets whether the page content is compressed; `true` by default.
    ///
    /// Uncompressed output is much larger, but its drawing operators can be
    /// read as text, as the tests comparing it with R's output do.
    ///
    /// This is R's `pdf(compress = )`.
    pub(crate) fn with_compression(mut self, compress: bool) -> Self {
        self.compress = compress;
        self
    }

    fn pt(&self, p: Point) -> (f64, f64) {
        (p.x * 72.0, p.y * 72.0)
    }

    /// Write the operator line `nums op`, each number with `prec` decimals.
    fn op(&mut self, nums: &[f64], prec: usize, op: &str) {
        for &v in nums {
            push_fixed(&mut self.content, v, prec);
            self.content.push(' ');
        }
        self.content.push_str(op);
        self.content.push('\n');
    }

    /// Sets the fill colour, and its alpha through an `ExtGState` (`/ca`),
    /// as R's `pdf()` does for semi-transparent colours.
    fn set_fill(&mut self, c: Color) {
        if self.last_fill_alpha != Some(c.a) {
            self.fill_alphas.insert(c.a);
            let _ = writeln!(self.content, "/Ga{} gs", c.a);
            self.last_fill_alpha = Some(c.a);
        }
        let rgb_only = c.with_alpha(255);
        if self.last_fill != Some(rgb_only) {
            let rgb = [c.r, c.g, c.b].map(|v| v as f64 / 255.0);
            self.op(&rgb, 4, "rg");
            self.last_fill = Some(rgb_only);
        }
    }

    /// Sets the stroke colour, and its alpha through an `ExtGState`
    /// (`/CA`).
    fn set_stroke_color(&mut self, c: Color) {
        if self.last_stroke_alpha != Some(c.a) {
            self.stroke_alphas.insert(c.a);
            let _ = writeln!(self.content, "/GA{} gs", c.a);
            self.last_stroke_alpha = Some(c.a);
        }
        let rgb_only = c.with_alpha(255);
        if self.last_stroke_color != Some(rgb_only) {
            let rgb = [c.r, c.g, c.b].map(|v| v as f64 / 255.0);
            self.op(&rgb, 4, "RG");
            self.last_stroke_color = Some(rgb_only);
        }
    }

    /// Write whichever of `stroke`'s colour and line-style parameters
    /// differ from those last written.
    fn set_stroke_style(&mut self, stroke: &Stroke) {
        self.set_stroke_color(stroke.color);
        let width = stroke.width_pt();
        if self.last_line.width != Some(width) {
            self.op(&[width], 3, "w");
            self.last_line.width = Some(width);
        }
        let cap = match stroke.lend {
            LineEnd::Butt => 0,
            LineEnd::Round => 1,
            LineEnd::Square => 2,
        };
        if self.last_line.cap != Some(cap) {
            let _ = writeln!(self.content, "{cap} J");
            self.last_line.cap = Some(cap);
        }
        let join = match stroke.ljoin {
            LineJoin::Mitre => 0,
            LineJoin::Round => 1,
            LineJoin::Bevel => 2,
        };
        if self.last_line.join != Some(join) {
            let _ = writeln!(self.content, "{join} j");
            self.last_line.join = Some(join);
        }
        if self.last_line.mitre != Some(stroke.lmitre) {
            self.op(&[stroke.lmitre], 2, "M");
            self.last_line.mitre = Some(stroke.lmitre);
        }
        let dash = stroke
            .lty
            .dash_lengths_pt(stroke.lwd, stroke.lend == LineEnd::Butt)
            .unwrap_or_default();
        if self.last_line.dash.as_ref() != Some(&dash) {
            self.content.push('[');
            for (i, &d) in dash.iter().enumerate() {
                if i > 0 {
                    self.content.push(' ');
                }
                push_fixed(&mut self.content, d, 3);
            }
            self.content.push_str("] 0 d\n");
            self.last_line.dash = Some(dash);
        }
    }

    /// Forget the colours and line style last written: `Q` restores the
    /// graphics state saved by `q`, so they may no longer be in effect.
    fn forget_state(&mut self) {
        self.last_fill = None;
        self.last_stroke_color = None;
        self.last_line = LineState::default();
        // Opaque unless some alpha has been set (it may be in effect again).
        let opaque = |used: &BTreeSet<u8>| used.is_empty().then_some(255);
        self.last_fill_alpha = opaque(&self.fill_alphas);
        self.last_stroke_alpha = opaque(&self.stroke_alphas);
    }

    fn path_moveto_lineto(&mut self, points: &[Point], close: bool) {
        for (i, p) in points.iter().enumerate() {
            let (x, y) = self.pt(*p);
            self.op(&[x, y], 3, if i == 0 { "m" } else { "l" });
        }
        if close {
            self.content.push_str("h\n");
        }
    }

    fn paint_path(&mut self, fill: Option<Color>, stroke: Option<&Stroke>) {
        let do_fill = fill.is_some();
        let do_stroke = stroke.map_or(false, |s| !matches!(s.lty, LineType::Blank));
        if let Some(c) = fill {
            self.set_fill(c);
        }
        if let Some(s) = stroke {
            if do_stroke {
                self.set_stroke_style(s);
            }
        }
        let op = match (do_fill, do_stroke) {
            (true, true) => "B",
            (true, false) => "f",
            (false, true) => "S",
            (false, false) => "n",
        };
        self.content.push_str(op);
        self.content.push('\n');
    }

    /// Approximate a circle with 4 cubic Bezier arcs (the standard
    /// kappa = 0.5522847498 magic constant).
    fn circle_path(&mut self, center: Point, radius: f64) {
        const K: f64 = 0.552_284_749_8;
        let (cx, cy) = self.pt(center);
        let r = radius * 72.0;
        let k = K * r;
        self.op(&[cx + r, cy], 3, "m");
        self.op(&[cx + r, cy + k, cx + k, cy + r, cx, cy + r], 3, "c");
        self.op(&[cx - k, cy + r, cx - r, cy + k, cx - r, cy], 3, "c");
        self.op(&[cx - r, cy - k, cx - k, cy - r, cx, cy - r], 3, "c");
        self.op(&[cx + k, cy - r, cx + r, cy - k, cx + r, cy], 3, "c");
        self.content.push_str("h\n");
    }

    fn font_resource_name(face: FontFace) -> &'static str {
        match face {
            FontFace::Plain => "/F1",
            FontFace::Bold => "/F2",
            FontFace::Italic => "/F3",
            FontFace::BoldItalic => "/F4",
            FontFace::Symbol => "/F5",
        }
    }

    fn base_font_name(face: FontFace) -> &'static str {
        match face {
            FontFace::Plain => "Helvetica",
            FontFace::Bold => "Helvetica-Bold",
            FontFace::Italic => "Helvetica-Oblique",
            FontFace::BoldItalic => "Helvetica-BoldOblique",
            FontFace::Symbol => "Symbol",
        }
    }

    /// A PDF string literal body for `s` in the fonts' WinAnsiEncoding.
    /// Like R's `pdf()` (whose default ISOLatin1 encoding agrees with WinAnsi
    /// above 0x9F), Latin-1 characters are written as their single byte, as
    /// an octal escape so the content stream stays ASCII. Characters with no
    /// byte in the encoding are drawn as `?`.
    ///
    /// With `symbol`, `s` is in the Symbol font's own encoding (one `char`
    /// per code), so every code up to 255 is written as its byte.
    fn escape_pdf_string(s: &str, symbol: bool) -> String {
        let mut out = String::with_capacity(s.len());
        for c in s.chars() {
            match c {
                '(' => out.push_str("\\("),
                ')' => out.push_str("\\)"),
                '\\' => out.push_str("\\\\"),
                ' '..='~' => out.push(c),
                '\u{a0}'..='\u{ff}' => out.push_str(&format!("\\{:03o}", c as u32)),
                '\u{80}'..='\u{9f}' if symbol => out.push_str(&format!("\\{:03o}", c as u32)),
                _ => out.push('?'),
            }
        }
        out
    }

    /// Finishes the PDF and returns its bytes, with every page started,
    /// including the current one.
    pub(crate) fn finish(self) -> Vec<u8> {
        let w_pt = self.width_in * 72.0;
        let h_pt = self.height_in * 72.0;

        let mut pages = self.pages;
        pages.push(self.content);
        let n_pages = pages.len();

        // 1: Catalog, 2: Pages, then a Page and its Contents per page
        // (3 and 4 for one page), then the four Helvetica fonts and, if
        // used, Symbol.
        let font0 = 3 + 2 * n_pages;
        let mut objects: Vec<Vec<u8>> = Vec::new();
        objects.push(b"<< /Type /Catalog /Pages 2 0 R >>".to_vec());
        let kids: Vec<String> = (0..n_pages).map(|i| format!("{} 0 R", 3 + 2 * i)).collect();
        objects.push(
            format!(
                "<< /Type /Pages /Kids [{}] /Count {n_pages} >>",
                kids.join(" ")
            )
            .into_bytes(),
        );
        let symbol_res = if self.uses_symbol {
            format!(" /F5 {} 0 R", font0 + 4)
        } else {
            String::new()
        };
        // The alphas' `ExtGState`s come after the fonts, one object each,
        // listed in every page's resources (none for opaque output).
        let mut gs_obj = font0 + 4 + usize::from(self.uses_symbol);
        let mut gs_res = String::new();
        let mut gs_objects: Vec<Vec<u8>> = Vec::new();
        for (key, prefix, alphas) in [
            ("ca", "Ga", &self.fill_alphas),
            ("CA", "GA", &self.stroke_alphas),
        ] {
            for &a in alphas {
                let _ = write!(gs_res, " /{prefix}{a} {gs_obj} 0 R");
                gs_objects.push(
                    format!("<< /Type /ExtGState /{key} {:.3} >>", a as f64 / 255.0).into_bytes(),
                );
                gs_obj += 1;
            }
        }
        if !gs_res.is_empty() {
            gs_res = format!(" /ExtGState <<{gs_res} >>");
        }
        for (i, content) in pages.into_iter().enumerate() {
            objects.push(
                format!(
                    "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {w_pt:.3} {h_pt:.3}] \
                     /Resources << /Font << /F1 {} 0 R /F2 {} 0 R /F3 {} 0 R /F4 {} 0 R{symbol_res} >>{gs_res} >> \
                     /Contents {} 0 R >>",
                    font0,
                    font0 + 1,
                    font0 + 2,
                    font0 + 3,
                    4 + 2 * i
                )
                .into_bytes(),
            );
            let (stream, filter) = if self.compress {
                let data = zlib_compress(content.as_bytes(), DEFLATE_LEVEL);
                (data, " /Filter /FlateDecode")
            } else {
                (content.into_bytes(), "")
            };
            let mut contents =
                format!("<< /Length {}{filter} >>\nstream\n", stream.len()).into_bytes();
            contents.extend_from_slice(&stream);
            // Flate data may not end in a newline; a separator added before
            // `endstream` isn't counted in `/Length`.
            if stream.last() != Some(&b'\n') {
                contents.push(b'\n');
            }
            contents.extend_from_slice(b"endstream");
            objects.push(contents);
        }
        for face in [
            FontFace::Plain,
            FontFace::Bold,
            FontFace::Italic,
            FontFace::BoldItalic,
        ] {
            objects.push(
                format!(
                    "<< /Type /Font /Subtype /Type1 /BaseFont /{} /Encoding /WinAnsiEncoding >>",
                    Self::base_font_name(face)
                )
                .into_bytes(),
            );
        }
        if self.uses_symbol {
            // A standard 14 font with its own built-in encoding.
            objects.push(b"<< /Type /Font /Subtype /Type1 /BaseFont /Symbol >>".to_vec());
        }
        objects.extend(gs_objects);

        let mut out = Vec::new();
        out.extend_from_slice(b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n");
        let mut offsets = Vec::with_capacity(objects.len());
        for (i, obj) in objects.iter().enumerate() {
            offsets.push(out.len());
            let header = format!("{} 0 obj\n", i + 1);
            out.extend_from_slice(header.as_bytes());
            out.extend_from_slice(obj);
            out.extend_from_slice(b"\nendobj\n");
        }
        let xref_offset = out.len();
        let n = objects.len() + 1;
        out.extend_from_slice(format!("xref\n0 {n}\n").as_bytes());
        out.extend_from_slice(b"0000000000 65535 f \n");
        for off in &offsets {
            out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
        }
        out.extend_from_slice(
            format!("trailer\n<< /Size {n} /Root 1 0 R >>\nstartxref\n{xref_offset}\n%%EOF\n")
                .as_bytes(),
        );
        out
    }
}

/// The font size R's `pdf()` sets and measures text in: `requested_pt`
/// rounded to whole points (`floor(x + 0.5)`; see
/// [`PdfBackend`]'s `effective_font_size`). Shared with the null device,
/// which measures as `pdf()` does.
pub(crate) fn pdf_font_size(requested_pt: f64) -> f64 {
    (requested_pt + 0.5).floor()
}

impl Backend for PdfBackend {
    fn into_bytes(self) -> Vec<u8> {
        self.finish()
    }

    fn size_inches(&self) -> (f64, f64) {
        (self.width_in, self.height_in)
    }

    /// R's `pdf()` device (`PDF_Text`, `PDF_StrWidth`, `PDF_MetricInfo` in
    /// `devPS.c`) uses `floor(cex * ps + 0.5)` points for drawing *and*
    /// for all string metrics - so e.g. `cex.main = 1.2` at 12pt is set in
    /// 14pt (not 14.4pt) and laid out with 14pt widths. Text whose rounded
    /// size is 0 is not drawn at all (callers check for a size `<= 0`).
    fn effective_font_size(&self, requested_pt: f64) -> f64 {
        pdf_font_size(requested_pt)
    }

    fn default_bg(&self) -> Color {
        self.background
    }

    /// R's `pdf()` works in big points.
    fn device_unit_inches(&self) -> f64 {
        1.0 / 72.0
    }

    fn take_page(&mut self) -> Vec<u8> {
        let fresh = PdfBackend::new(self.width_in, self.height_in)
            .with_background(self.background)
            .with_compression(self.compress);
        std::mem::replace(self, fresh).finish()
    }

    fn new_page(&mut self) {
        let page = std::mem::take(&mut self.content);
        if self.started {
            self.pages.push(page);
        }
        self.started = true;
        self.forget_state();
        // A new page starts in the default (opaque) graphics state.
        self.last_fill_alpha = Some(255);
        self.last_stroke_alpha = Some(255);
    }

    fn push_clip(&mut self, rect: Rect) {
        self.content.push_str("q\n");
        let (x0, y0) = self.pt(Point::new(rect.x0, rect.y0));
        let w = rect.width() * 72.0;
        let h = rect.height() * 72.0;
        self.op(&[x0, y0, w, h], 3, "re W n");
        self.clip_depth += 1;
        // `q`/`Q` save and restore the colours and line style too.
        self.forget_state();
    }

    fn pop_clip(&mut self) {
        if self.clip_depth > 0 {
            self.content.push_str("Q\n");
            self.clip_depth -= 1;
            self.forget_state();
        }
    }

    fn draw_line(&mut self, p0: Point, p1: Point, stroke: &Stroke) {
        if matches!(stroke.lty, LineType::Blank) {
            return;
        }
        self.path_moveto_lineto(&[p0, p1], false);
        self.paint_path(None, Some(stroke));
    }

    fn draw_polyline(&mut self, points: &[Point], stroke: &Stroke) {
        if matches!(stroke.lty, LineType::Blank) || points.len() < 2 {
            return;
        }
        self.path_moveto_lineto(points, false);
        self.paint_path(None, Some(stroke));
    }

    fn draw_polygon(&mut self, points: &[Point], fill: Option<Color>, stroke: Option<&Stroke>) {
        if points.len() < 2 {
            return;
        }
        self.path_moveto_lineto(points, true);
        self.paint_path(fill, stroke);
    }

    fn draw_rect(&mut self, rect: Rect, fill: Option<Color>, stroke: Option<&Stroke>) {
        let (x0, y0) = self.pt(Point::new(rect.x0, rect.y0));
        let w = rect.width() * 72.0;
        let h = rect.height() * 72.0;
        self.op(&[x0, y0, w, h], 3, "re");
        self.paint_path(fill, stroke);
    }

    fn draw_circle(
        &mut self,
        center: Point,
        radius: f64,
        fill: Option<Color>,
        stroke: Option<&Stroke>,
    ) {
        self.circle_path(center, radius);
        self.paint_path(fill, stroke);
    }

    fn draw_text(&mut self, origin: Point, text: &str, style: &TextStyle) {
        if text.is_empty() {
            return;
        }
        self.set_fill(style.color);
        let symbol = style.face == FontFace::Symbol;
        self.uses_symbol |= symbol;
        let (x, y) = self.pt(origin);
        let theta = style.rotation_deg.to_radians();
        let (c, s) = (theta.cos(), theta.sin());
        self.content.push_str("BT\n");
        self.content.push_str(Self::font_resource_name(style.face));
        self.content.push(' ');
        self.op(&[style.size_pt], 2, "Tf");
        for v in [c, s, -s, c] {
            push_fixed(&mut self.content, v, 6);
            self.content.push(' ');
        }
        self.op(&[x, y], 3, "Tm");
        // As R's `pdf()`: a string with kerning pairs is a `TJ` array, the
        // pieces separated by each kern (negated: positive numbers move the
        // next glyph left); otherwise a plain `Tj`.
        let kerns = afm::kerning_units(text, style.face);
        if kerns.iter().any(|&k| k != 0.0) {
            self.content.push_str("[(");
            let mut buf = [0u8; 4];
            for (i, ch) in text.chars().enumerate() {
                let piece = Self::escape_pdf_string(ch.encode_utf8(&mut buf), symbol);
                self.content.push_str(&piece);
                if let Some(&k) = kerns.get(i).filter(|&&k| k != 0.0) {
                    let _ = write!(self.content, ") {} (", -(k as i64));
                }
            }
            self.content.push_str(")] TJ\n");
        } else {
            let _ = writeln!(
                self.content,
                "({}) Tj",
                Self::escape_pdf_string(text, symbol)
            );
        }
        self.content.push_str("ET\n");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::svg::SvgBackend;

    #[test]
    fn pdf_rounds_font_size_like_r_but_svg_does_not() {
        let pdf = PdfBackend::new(6.0, 4.0);
        assert_eq!(pdf.effective_font_size(14.4), 14.0);
        assert_eq!(pdf.effective_font_size(10.5), 11.0);
        assert_eq!(pdf.effective_font_size(0.4), 0.0);
        let svg = SvgBackend::new(6.0, 4.0);
        assert_eq!(svg.effective_font_size(14.4), 14.4);
    }
}
