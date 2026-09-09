//! Minimal hand-rolled single-page PDF writer, using the 4 standard
//! Type1 Helvetica faces (`Helvetica`, `-Bold`, `-Oblique`,
//! `-BoldOblique`) that every PDF viewer/renderer bundles - so, like R's
//! own `pdf()` device default, no font embedding is needed at all, and
//! text metrics/rendering come from the exact same Adobe AFM data this
//! crate already uses for layout (`crate::afm`).
//!
//! PDF's native page coordinate system is already "inches (well, points)
//! from the bottom-left, y-up", matching this crate's internal `Backend`
//! convention exactly (see `backend/mod.rs`), so this backend applies no
//! coordinate flip - only a inches-to-points scale (`1 in = 72 pt`).

use std::fmt::Write as _;

use super::{Backend, LineEnd, LineJoin, Stroke, TextStyle};
use crate::afm::FontFace;
use crate::color::Color;
use crate::geometry::{Point, Rect};
use crate::lty::LineType;

pub struct PdfBackend {
    width_in: f64,
    height_in: f64,
    content: String,
    clip_depth: u32,
    last_fill: Option<Color>,
    last_stroke_color: Option<Color>,
}

impl PdfBackend {
    pub fn new(width_in: f64, height_in: f64) -> Self {
        PdfBackend {
            width_in,
            height_in,
            content: String::new(),
            clip_depth: 0,
            last_fill: None,
            last_stroke_color: None,
        }
    }

    fn pt(&self, p: Point) -> (f64, f64) {
        (p.x * 72.0, p.y * 72.0)
    }

    fn set_fill(&mut self, c: Color) {
        if self.last_fill != Some(c) {
            let _ = writeln!(
                self.content,
                "{:.4} {:.4} {:.4} rg",
                c.r as f64 / 255.0,
                c.g as f64 / 255.0,
                c.b as f64 / 255.0
            );
            self.last_fill = Some(c);
        }
    }

    fn set_stroke_color(&mut self, c: Color) {
        if self.last_stroke_color != Some(c) {
            let _ = writeln!(
                self.content,
                "{:.4} {:.4} {:.4} RG",
                c.r as f64 / 255.0,
                c.g as f64 / 255.0,
                c.b as f64 / 255.0
            );
            self.last_stroke_color = Some(c);
        }
    }

    fn set_stroke_style(&mut self, stroke: &Stroke) {
        self.set_stroke_color(stroke.color);
        let _ = writeln!(self.content, "{:.3} w", stroke.width_pt());
        let cap = match stroke.lend {
            LineEnd::Butt => 0,
            LineEnd::Round => 1,
            LineEnd::Square => 2,
        };
        let join = match stroke.ljoin {
            LineJoin::Mitre => 0,
            LineJoin::Round => 1,
            LineJoin::Bevel => 2,
        };
        let _ = writeln!(self.content, "{cap} J {join} j {:.2} M", stroke.lmitre);
        match stroke
            .lty
            .dash_lengths_pt(stroke.lwd, stroke.lend == LineEnd::Round)
        {
            Some(dashes) => {
                let list = dashes
                    .iter()
                    .map(|d| format!("{d:.3}"))
                    .collect::<Vec<_>>()
                    .join(" ");
                let _ = writeln!(self.content, "[{list}] 0 d");
            }
            None => {
                let _ = writeln!(self.content, "[] 0 d");
            }
        }
    }

    fn path_moveto_lineto(&mut self, points: &[Point], close: bool) {
        for (i, p) in points.iter().enumerate() {
            let (x, y) = self.pt(*p);
            if i == 0 {
                let _ = writeln!(self.content, "{x:.3} {y:.3} m");
            } else {
                let _ = writeln!(self.content, "{x:.3} {y:.3} l");
            }
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
        let _ = writeln!(self.content, "{:.3} {:.3} m", cx + r, cy);
        let _ = writeln!(
            self.content,
            "{:.3} {:.3} {:.3} {:.3} {:.3} {:.3} c",
            cx + r,
            cy + k,
            cx + k,
            cy + r,
            cx,
            cy + r
        );
        let _ = writeln!(
            self.content,
            "{:.3} {:.3} {:.3} {:.3} {:.3} {:.3} c",
            cx - k,
            cy + r,
            cx - r,
            cy + k,
            cx - r,
            cy
        );
        let _ = writeln!(
            self.content,
            "{:.3} {:.3} {:.3} {:.3} {:.3} {:.3} c",
            cx - r,
            cy - k,
            cx - k,
            cy - r,
            cx,
            cy - r
        );
        let _ = writeln!(
            self.content,
            "{:.3} {:.3} {:.3} {:.3} {:.3} {:.3} c",
            cx + k,
            cy - r,
            cx + r,
            cy - k,
            cx + r,
            cy
        );
        self.content.push_str("h\n");
    }

    fn font_resource_name(face: FontFace) -> &'static str {
        match face {
            FontFace::Plain => "/F1",
            FontFace::Bold => "/F2",
            FontFace::Italic => "/F3",
            FontFace::BoldItalic => "/F4",
        }
    }

    fn base_font_name(face: FontFace) -> &'static str {
        match face {
            FontFace::Plain => "Helvetica",
            FontFace::Bold => "Helvetica-Bold",
            FontFace::Italic => "Helvetica-Oblique",
            FontFace::BoldItalic => "Helvetica-BoldOblique",
        }
    }

    fn escape_pdf_string(s: &str) -> String {
        let mut out = String::with_capacity(s.len());
        for c in s.chars() {
            match c {
                '(' => out.push_str("\\("),
                ')' => out.push_str("\\)"),
                '\\' => out.push_str("\\\\"),
                _ => out.push(c),
            }
        }
        out
    }

    /// Serialize the completed single-page PDF as bytes.
    pub fn finish(self) -> Vec<u8> {
        let w_pt = self.width_in * 72.0;
        let h_pt = self.height_in * 72.0;

        let mut objects: Vec<String> = Vec::new();
        // 1: Catalog, 2: Pages, 3: Page, 4: Contents, 5-8: Fonts.
        objects.push("<< /Type /Catalog /Pages 2 0 R >>".to_string());
        objects.push("<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string());
        objects.push(format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {w_pt:.3} {h_pt:.3}] \
             /Resources << /Font << /F1 5 0 R /F2 6 0 R /F3 7 0 R /F4 8 0 R >> >> \
             /Contents 4 0 R >>"
        ));
        let stream = &self.content;
        objects.push(format!(
            "<< /Length {} >>\nstream\n{}endstream",
            stream.as_bytes().len(),
            stream
        ));
        for face in [
            FontFace::Plain,
            FontFace::Bold,
            FontFace::Italic,
            FontFace::BoldItalic,
        ] {
            objects.push(format!(
                "<< /Type /Font /Subtype /Type1 /BaseFont /{} /Encoding /WinAnsiEncoding >>",
                Self::base_font_name(face)
            ));
        }

        let mut out = Vec::new();
        out.extend_from_slice(b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n");
        let mut offsets = Vec::with_capacity(objects.len());
        for (i, obj) in objects.iter().enumerate() {
            offsets.push(out.len());
            let header = format!("{} 0 obj\n", i + 1);
            out.extend_from_slice(header.as_bytes());
            out.extend_from_slice(obj.as_bytes());
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

impl Backend for PdfBackend {
    fn size_inches(&self) -> (f64, f64) {
        (self.width_in, self.height_in)
    }

    fn push_clip(&mut self, rect: Rect) {
        self.content.push_str("q\n");
        let (x0, y0) = self.pt(Point::new(rect.x0, rect.y0));
        let w = rect.width() * 72.0;
        let h = rect.height() * 72.0;
        let _ = writeln!(self.content, "{x0:.3} {y0:.3} {w:.3} {h:.3} re W n");
        self.clip_depth += 1;
        // Restoring `q`/`Q` also resets the fill/stroke color cache
        // assumptions, since PDF graphics state (including color) is
        // saved/restored by q/Q too.
        self.last_fill = None;
        self.last_stroke_color = None;
    }

    fn pop_clip(&mut self) {
        if self.clip_depth > 0 {
            self.content.push_str("Q\n");
            self.clip_depth -= 1;
            self.last_fill = None;
            self.last_stroke_color = None;
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
        let _ = writeln!(self.content, "{x0:.3} {y0:.3} {w:.3} {h:.3} re");
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
        let (x, y) = self.pt(origin);
        let theta = style.rotation_deg.to_radians();
        let (c, s) = (theta.cos(), theta.sin());
        self.content.push_str("BT\n");
        let _ = writeln!(
            self.content,
            "{} {:.2} Tf",
            Self::font_resource_name(style.face),
            style.size_pt
        );
        let _ = writeln!(
            self.content,
            "{c:.6} {s:.6} {:.6} {c:.6} {x:.3} {y:.3} Tm",
            -s
        );
        let _ = writeln!(self.content, "({}) Tj", Self::escape_pdf_string(text));
        self.content.push_str("ET\n");
    }
}
