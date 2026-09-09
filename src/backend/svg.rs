//! SVG backend: a hand-rolled writer producing plain `<svg>` markup with
//! `<text>` elements (matching how R's own `svg()` Cairo-based device
//! emits text - real `<text>`, not paths), so a browser/viewer's own font
//! substitution renders glyphs, same as it would for R's SVG output.
//!
//! Internal user units are points (`1/72 inch`), matching PDF and R's
//! Cairo `svg()` device; the root `<svg>` element's `width`/`height`
//! attributes carry the physical inch size directly.

use std::fmt::Write as _;

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
        let _ = writeln!(
            out,
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}in\" height=\"{}in\" viewBox=\"0 0 {} {}\">",
            fmt_num(self.width_in),
            fmt_num(self.height_in),
            fmt_num(self.width_pt()),
            fmt_num(self.height_pt())
        );
        out.push_str(&self.body);
        out.push_str("</svg>\n");
        out
    }

    fn stroke_attrs(&self, out: &mut String, stroke: &Stroke) {
        let _ = write!(
            out,
            " stroke=\"{}\" stroke-width=\"{}\"",
            stroke.color.to_hex(),
            fmt_num(stroke.width_pt())
        );
        if stroke.color.a != 255 {
            let _ = write!(
                out,
                " stroke-opacity=\"{}\"",
                fmt_num(stroke.color.a as f64 / 255.0)
            );
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
        let _ = write!(out, " stroke-linecap=\"{cap}\" stroke-linejoin=\"{join}\"");
        if matches!(stroke.ljoin, LineJoin::Mitre) {
            let _ = write!(out, " stroke-miterlimit=\"{}\"", fmt_num(stroke.lmitre));
        }
        if let Some(dashes) = stroke
            .lty
            .dash_lengths_pt(stroke.lwd, stroke.lend == LineEnd::Round)
        {
            let list = dashes
                .iter()
                .map(|d| fmt_num(*d))
                .collect::<Vec<_>>()
                .join(",");
            let _ = write!(out, " stroke-dasharray=\"{list}\"");
        }
    }

    fn fill_attr(out: &mut String, fill: Option<Color>) {
        match fill {
            Some(c) => {
                let _ = write!(out, " fill=\"{}\"", c.to_hex());
                if c.a != 255 {
                    let _ = write!(out, " fill-opacity=\"{}\"", fmt_num(c.a as f64 / 255.0));
                }
            }
            None => out.push_str(" fill=\"none\""),
        }
    }

    fn border_attrs(&self, out: &mut String, stroke: Option<&Stroke>) {
        match stroke {
            Some(s) if !matches!(s.lty, LineType::Blank) => self.stroke_attrs(out, s),
            _ => out.push_str(" stroke=\"none\""),
        }
    }
}

fn fmt_num(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        let s = format!("{v:.4}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
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
        let _ = writeln!(
            self.body,
            "<clipPath id=\"clip{id}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"/></clipPath>",
            fmt_num(x0),
            fmt_num(y0),
            fmt_num(w),
            fmt_num(h)
        );
        let _ = writeln!(self.body, "<g clip-path=\"url(#clip{id})\">");
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
        let mut attrs = String::new();
        self.stroke_attrs(&mut attrs, stroke);
        let _ = writeln!(
            self.body,
            "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\"{attrs}/>",
            fmt_num(x1),
            fmt_num(y1),
            fmt_num(x2),
            fmt_num(y2)
        );
    }

    fn draw_polyline(&mut self, points: &[Point], stroke: &Stroke) {
        if matches!(stroke.lty, LineType::Blank) || points.len() < 2 {
            return;
        }
        let pts = points
            .iter()
            .map(|p| {
                let (x, y) = self.to_pt(*p);
                format!("{},{}", fmt_num(x), fmt_num(y))
            })
            .collect::<Vec<_>>()
            .join(" ");
        let mut attrs = String::new();
        attrs.push_str(" fill=\"none\"");
        self.stroke_attrs(&mut attrs, stroke);
        let _ = writeln!(self.body, "<polyline points=\"{pts}\"{attrs}/>");
    }

    fn draw_polygon(&mut self, points: &[Point], fill: Option<Color>, stroke: Option<&Stroke>) {
        if points.len() < 2 {
            return;
        }
        let pts = points
            .iter()
            .map(|p| {
                let (x, y) = self.to_pt(*p);
                format!("{},{}", fmt_num(x), fmt_num(y))
            })
            .collect::<Vec<_>>()
            .join(" ");
        let mut attrs = String::new();
        Self::fill_attr(&mut attrs, fill);
        self.border_attrs(&mut attrs, stroke);
        let _ = writeln!(self.body, "<polygon points=\"{pts}\"{attrs}/>");
    }

    fn draw_rect(&mut self, rect: Rect, fill: Option<Color>, stroke: Option<&Stroke>) {
        let (x0, y0) = self.to_pt(Point::new(rect.x0, rect.y1));
        let w = rect.width() * 72.0;
        let h = rect.height() * 72.0;
        let mut attrs = String::new();
        Self::fill_attr(&mut attrs, fill);
        self.border_attrs(&mut attrs, stroke);
        let _ = writeln!(
            self.body,
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"{attrs}/>",
            fmt_num(x0),
            fmt_num(y0),
            fmt_num(w),
            fmt_num(h)
        );
    }

    fn draw_circle(
        &mut self,
        center: Point,
        radius: f64,
        fill: Option<Color>,
        stroke: Option<&Stroke>,
    ) {
        let (cx, cy) = self.to_pt(center);
        let r = radius * 72.0;
        let mut attrs = String::new();
        Self::fill_attr(&mut attrs, fill);
        self.border_attrs(&mut attrs, stroke);
        let _ = writeln!(
            self.body,
            "<circle cx=\"{}\" cy=\"{}\" r=\"{}\"{attrs}/>",
            fmt_num(cx),
            fmt_num(cy),
            fmt_num(r)
        );
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
        let mut transform = String::new();
        if style.rotation_deg != 0.0 {
            let _ = write!(
                transform,
                " transform=\"rotate({} {} {})\"",
                fmt_num(-style.rotation_deg),
                fmt_num(x),
                fmt_num(y)
            );
        }
        let _ = writeln!(
            self.body,
            "<text x=\"{}\" y=\"{}\" text-anchor=\"start\" font-family=\"Helvetica, Arial, sans-serif\" font-size=\"{}\" font-style=\"{style_str}\" font-weight=\"{weight}\" fill=\"{}\"{transform}>{}</text>",
            fmt_num(x),
            fmt_num(y),
            fmt_num(style.size_pt),
            style.color.to_hex(),
            escape_xml(text)
        );
    }
}
