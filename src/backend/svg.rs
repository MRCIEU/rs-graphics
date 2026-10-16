//! SVG backend: a hand-rolled writer producing plain `<svg>` markup with
//! `<text>` elements (matching how R's own `svg()` Cairo-based device
//! emits text - real `<text>`, not paths), so a browser/viewer's own font
//! substitution renders glyphs, same as it would for R's SVG output.
//!
//! Internal user units are points (`1/72 inch`), matching PDF and R's
//! Cairo `svg()` device; the root `<svg>` element's `width`/`height`
//! attributes carry the physical inch size directly.

use std::fmt::Write as _;

use super::numfmt::{push_hex, push_trimmed};
use super::{Backend, LineEnd, LineJoin, Stroke, TextStyle};
use crate::color::Color;
use crate::geometry::{Point, Rect};
use crate::lty::LineType;

pub struct SvgBackend {
    width_in: f64,
    height_in: f64,
    body: String,
    clip_stack: Vec<Rect>,
    next_clip_id: u32,
}

impl SvgBackend {
    pub fn new(width_in: f64, height_in: f64) -> Self {
        SvgBackend {
            width_in,
            height_in,
            body: String::new(),
            clip_stack: Vec::new(),
            next_clip_id: 0,
        }
    }

    fn to_pt(&self, p: Point) -> (f64, f64) {
        (p.x * 72.0, (self.height_in - p.y) * 72.0)
    }

    fn width_pt(&self) -> f64 {
        self.width_in * 72.0
    }

    fn height_pt(&self) -> f64 {
        self.height_in * 72.0
    }

    /// Full SVG document as a `String`.
    pub fn finish(self) -> String {
        let mut out = String::new();
        let _ = writeln!(
            out,
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"no\"?>"
        );
        out.push_str("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"");
        push_trimmed(&mut out, self.width_in);
        out.push_str("in\" height=\"");
        push_trimmed(&mut out, self.height_in);
        out.push_str("in\" viewBox=\"0 0 ");
        push_trimmed(&mut out, self.width_pt());
        out.push(' ');
        push_trimmed(&mut out, self.height_pt());
        out.push_str("\">\n");
        out.push_str(&self.body);
        out.push_str("</svg>\n");
        out
    }

    /// Append `<tag points="x,y x,y ..."`.
    fn push_points(&mut self, tag: &str, points: &[Point]) {
        let height_in = self.height_in;
        let out = &mut self.body;
        out.push('<');
        out.push_str(tag);
        out.push_str(" points=\"");
        for (i, p) in points.iter().enumerate() {
            if i > 0 {
                out.push(' ');
            }
            push_trimmed(out, p.x * 72.0);
            out.push(',');
            push_trimmed(out, (height_in - p.y) * 72.0);
        }
        out.push('"');
    }
}

/// Append ` name="v"`.
fn push_attr(out: &mut String, name: &str, v: f64) {
    out.push(' ');
    out.push_str(name);
    out.push_str("=\"");
    push_trimmed(out, v);
    out.push('"');
}

/// Append ` name="#rrggbb"`.
fn push_color_attr(out: &mut String, name: &str, c: Color) {
    out.push(' ');
    out.push_str(name);
    out.push_str("=\"");
    push_hex(out, c);
    out.push('"');
}

fn push_stroke_attrs(out: &mut String, stroke: &Stroke) {
    push_color_attr(out, "stroke", stroke.color);
    push_attr(out, "stroke-width", stroke.width_pt());
    if stroke.color.a != 255 {
        push_attr(out, "stroke-opacity", stroke.color.a as f64 / 255.0);
    }
    let cap = match stroke.lend {
        LineEnd::Round => "round",
        LineEnd::Butt => "butt",
        LineEnd::Square => "square",
    };
    let join = match stroke.ljoin {
        LineJoin::Round => "round",
        LineJoin::Mitre => "miter",
        LineJoin::Bevel => "bevel",
    };
    out.push_str(" stroke-linecap=\"");
    out.push_str(cap);
    out.push_str("\" stroke-linejoin=\"");
    out.push_str(join);
    out.push('"');
    if matches!(stroke.ljoin, LineJoin::Mitre) {
        push_attr(out, "stroke-miterlimit", stroke.lmitre);
    }
    if let Some(dashes) = stroke
        .lty
        .dash_lengths_pt(stroke.lwd, stroke.lend == LineEnd::Round)
    {
        out.push_str(" stroke-dasharray=\"");
        for (i, &d) in dashes.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            push_trimmed(out, d);
        }
        out.push('"');
    }
}

fn push_fill_attrs(out: &mut String, fill: Option<Color>) {
    match fill {
        Some(c) => {
            push_color_attr(out, "fill", c);
            if c.a != 255 {
                push_attr(out, "fill-opacity", c.a as f64 / 255.0);
            }
        }
        None => out.push_str(" fill=\"none\""),
    }
}

fn push_border_attrs(out: &mut String, stroke: Option<&Stroke>) {
    match stroke {
        Some(s) if !matches!(s.lty, LineType::Blank) => push_stroke_attrs(out, s),
        _ => out.push_str(" stroke=\"none\""),
    }
}

fn escape_xml(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

impl Backend for SvgBackend {
    fn size_inches(&self) -> (f64, f64) {
        (self.width_in, self.height_in)
    }

    fn push_clip(&mut self, rect: Rect) {
        self.clip_stack.push(rect);
        let id = self.next_clip_id;
        self.next_clip_id += 1;
        let (x0, y0) = self.to_pt(Point::new(rect.x0, rect.y1));
        let w = rect.width() * 72.0;
        let h = rect.height() * 72.0;
        let out = &mut self.body;
        let _ = write!(out, "<clipPath id=\"clip{id}\"><rect");
        push_attr(out, "x", x0);
        push_attr(out, "y", y0);
        push_attr(out, "width", w);
        push_attr(out, "height", h);
        let _ = writeln!(out, "/></clipPath>");
        let _ = writeln!(out, "<g clip-path=\"url(#clip{id})\">");
    }

    fn pop_clip(&mut self) {
        if self.clip_stack.pop().is_some() {
            self.body.push_str("</g>\n");
        }
    }

    fn draw_line(&mut self, p0: Point, p1: Point, stroke: &Stroke) {
        if matches!(stroke.lty, LineType::Blank) {
            return;
        }
        let (x1, y1) = self.to_pt(p0);
        let (x2, y2) = self.to_pt(p1);
        let out = &mut self.body;
        out.push_str("<line");
        push_attr(out, "x1", x1);
        push_attr(out, "y1", y1);
        push_attr(out, "x2", x2);
        push_attr(out, "y2", y2);
        push_stroke_attrs(out, stroke);
        out.push_str("/>\n");
    }

    fn draw_polyline(&mut self, points: &[Point], stroke: &Stroke) {
        if matches!(stroke.lty, LineType::Blank) || points.len() < 2 {
            return;
        }
        self.push_points("polyline", points);
        let out = &mut self.body;
        out.push_str(" fill=\"none\"");
        push_stroke_attrs(out, stroke);
        out.push_str("/>\n");
    }

    fn draw_polygon(&mut self, points: &[Point], fill: Option<Color>, stroke: Option<&Stroke>) {
        if points.len() < 2 {
            return;
        }
        self.push_points("polygon", points);
        let out = &mut self.body;
        push_fill_attrs(out, fill);
        push_border_attrs(out, stroke);
        out.push_str("/>\n");
    }

    fn draw_rect(&mut self, rect: Rect, fill: Option<Color>, stroke: Option<&Stroke>) {
        let (x0, y0) = self.to_pt(Point::new(rect.x0, rect.y1));
        let w = rect.width() * 72.0;
        let h = rect.height() * 72.0;
        let out = &mut self.body;
        out.push_str("<rect");
        push_attr(out, "x", x0);
        push_attr(out, "y", y0);
        push_attr(out, "width", w);
        push_attr(out, "height", h);
        push_fill_attrs(out, fill);
        push_border_attrs(out, stroke);
        out.push_str("/>\n");
    }

    fn draw_circle(
        &mut self,
        center: Point,
        radius: f64,
        fill: Option<Color>,
        stroke: Option<&Stroke>,
    ) {
        let (cx, cy) = self.to_pt(center);
        let out = &mut self.body;
        out.push_str("<circle");
        push_attr(out, "cx", cx);
        push_attr(out, "cy", cy);
        push_attr(out, "r", radius * 72.0);
        push_fill_attrs(out, fill);
        push_border_attrs(out, stroke);
        out.push_str("/>\n");
    }

    fn draw_text(&mut self, origin: Point, text: &str, style: &TextStyle) {
        if text.is_empty() {
            return;
        }
        let (x, y) = self.to_pt(origin);
        let style_str = match style.face {
            crate::afm::FontFace::Italic | crate::afm::FontFace::BoldItalic => "italic",
            _ => "normal",
        };
        let weight = match style.face {
            crate::afm::FontFace::Bold | crate::afm::FontFace::BoldItalic => "bold",
            _ => "normal",
        };
        let out = &mut self.body;
        out.push_str("<text");
        push_attr(out, "x", x);
        push_attr(out, "y", y);
        out.push_str(" text-anchor=\"start\" font-family=\"Helvetica, Arial, sans-serif\"");
        push_attr(out, "font-size", style.size_pt);
        let _ = write!(out, " font-style=\"{style_str}\" font-weight=\"{weight}\"");
        push_color_attr(out, "fill", style.color);
        if style.rotation_deg != 0.0 {
            out.push_str(" transform=\"rotate(");
            push_trimmed(out, -style.rotation_deg);
            out.push(' ');
            push_trimmed(out, x);
            out.push(' ');
            push_trimmed(out, y);
            out.push_str(")\"");
        }
        out.push('>');
        out.push_str(&escape_xml(text));
        out.push_str("</text>\n");
    }
}
