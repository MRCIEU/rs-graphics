// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! SVG backend: a hand-rolled writer producing plain `<svg>` markup with
//! real `<text>` elements, so a browser/viewer's own font substitution
//! renders glyphs. (R's Cairo-based `svg()` device instead writes glyph
//! outlines as paths.)
//!
//! Internal user units are points (`1/72 inch`), matching PDF and R's
//! Cairo `svg()` device; the root `<svg>` element's `width`/`height`
//! attributes carry the physical inch size directly.
//!
//! Consecutive shapes with the same style (fill and stroke attributes) are
//! written inside one `<g>` carrying that style, which they inherit, rather
//! than repeating it on each: in a scatter plot the style is most of every
//! point's markup. A shape whose style differs from its neighbours' keeps
//! its own attributes.

use std::fmt::Write as _;

use super::compress::{gzip_compress, DEFLATE_LEVEL};
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
    /// The shape being written: its markup up to its style attributes, and
    /// those attributes (each with a leading space).
    geom: String,
    style: String,
    /// The last shape, held back until we know whether the next one shares
    /// its style.
    pending: Option<(String, String)>,
    /// The style of the open `<g>` of same-style shapes, if any.
    group_style: Option<String>,
}

impl SvgBackend {
    pub fn new(width_in: impl Into<f64>, height_in: impl Into<f64>) -> Self {
        let (width_in, height_in) = (width_in.into(), height_in.into());
        SvgBackend {
            width_in,
            height_in,
            body: String::new(),
            clip_stack: Vec::new(),
            next_clip_id: 0,
            geom: String::new(),
            style: String::new(),
            pending: None,
            group_style: None,
        }
    }

    /// Write the shape in `geom` + `style`: into the open group if it has
    /// the same style, else into a new group with the held-back shape if
    /// that has the same style, else hold it back.
    fn emit_shape(&mut self) {
        if let Some(group) = &self.group_style {
            if *group == self.style {
                self.body.push_str(&self.geom);
                self.body.push_str("/>\n");
                return;
            }
            self.body.push_str("</g>\n");
            self.group_style = None;
        }
        if let Some((geom, style)) = self.pending.take() {
            if style == self.style {
                self.body.push_str("<g");
                self.body.push_str(&style);
                self.body.push_str(">\n");
                self.body.push_str(&geom);
                self.body.push_str("/>\n");
                self.body.push_str(&self.geom);
                self.body.push_str("/>\n");
                self.group_style = Some(style);
                return;
            }
            self.write_whole(&geom, &style);
        }
        self.pending = Some((self.geom.clone(), self.style.clone()));
    }

    fn write_whole(&mut self, geom: &str, style: &str) {
        self.body.push_str(geom);
        self.body.push_str(style);
        self.body.push_str("/>\n");
    }

    /// Write out the held-back shape and close any open group, before
    /// anything else goes into the document.
    fn flush_shapes(&mut self) {
        if self.group_style.take().is_some() {
            self.body.push_str("</g>\n");
        }
        if let Some((geom, style)) = self.pending.take() {
            self.write_whole(&geom, &style);
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

    /// The SVG document, gzip-compressed as for a `.svgz` file: several
    /// times smaller (about 4-6x for big scatter plots) and compressed on
    /// all cores. Inkscape, Illustrator and the like open `.svgz` files, but
    /// browsers only display them when served with `Content-Encoding: gzip`,
    /// so [`SvgBackend::finish`]'s plain SVG is the default.
    pub fn finish_svgz(self) -> Vec<u8> {
        gzip_compress(self.finish().as_bytes(), DEFLATE_LEVEL)
    }

    /// Full SVG document as a `String`.
    pub fn finish(mut self) -> String {
        self.flush_shapes();
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

    /// Start `geom` with `<tag points="x,y x,y ..."`.
    fn push_points(&mut self, tag: &str, points: &[Point]) {
        let height_in = self.height_in;
        let out = &mut self.geom;
        out.clear();
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
        .dash_lengths_pt(stroke.lwd, stroke.lend == LineEnd::Butt)
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
    fn into_bytes(self) -> Vec<u8> {
        self.finish().into_bytes()
    }

    fn size_inches(&self) -> (f64, f64) {
        (self.width_in, self.height_in)
    }

    fn new_page(&mut self) {
        self.body.clear();
        self.pending = None;
        self.group_style = None;
        // Their `<g>`s went with the old body.
        self.clip_stack.clear();
    }

    fn push_clip(&mut self, rect: Rect) {
        self.flush_shapes();
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
        self.flush_shapes();
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
        let out = &mut self.geom;
        out.clear();
        out.push_str("<line");
        push_attr(out, "x1", x1);
        push_attr(out, "y1", y1);
        push_attr(out, "x2", x2);
        push_attr(out, "y2", y2);
        self.style.clear();
        push_stroke_attrs(&mut self.style, stroke);
        self.emit_shape();
    }

    fn draw_polyline(&mut self, points: &[Point], stroke: &Stroke) {
        if matches!(stroke.lty, LineType::Blank) || points.len() < 2 {
            return;
        }
        self.push_points("polyline", points);
        self.style.clear();
        self.style.push_str(" fill=\"none\"");
        push_stroke_attrs(&mut self.style, stroke);
        self.emit_shape();
    }

    fn draw_polygon(&mut self, points: &[Point], fill: Option<Color>, stroke: Option<&Stroke>) {
        if points.len() < 2 {
            return;
        }
        self.push_points("polygon", points);
        self.style.clear();
        push_fill_attrs(&mut self.style, fill);
        push_border_attrs(&mut self.style, stroke);
        self.emit_shape();
    }

    fn draw_rect(&mut self, rect: Rect, fill: Option<Color>, stroke: Option<&Stroke>) {
        let (x0, y0) = self.to_pt(Point::new(rect.x0, rect.y1));
        let w = rect.width() * 72.0;
        let h = rect.height() * 72.0;
        let out = &mut self.geom;
        out.clear();
        out.push_str("<rect");
        push_attr(out, "x", x0);
        push_attr(out, "y", y0);
        push_attr(out, "width", w);
        push_attr(out, "height", h);
        self.style.clear();
        push_fill_attrs(&mut self.style, fill);
        push_border_attrs(&mut self.style, stroke);
        self.emit_shape();
    }

    fn draw_circle(
        &mut self,
        center: Point,
        radius: f64,
        fill: Option<Color>,
        stroke: Option<&Stroke>,
    ) {
        let (cx, cy) = self.to_pt(center);
        let out = &mut self.geom;
        out.clear();
        out.push_str("<circle");
        push_attr(out, "cx", cx);
        push_attr(out, "cy", cy);
        push_attr(out, "r", radius * 72.0);
        self.style.clear();
        push_fill_attrs(&mut self.style, fill);
        push_border_attrs(&mut self.style, stroke);
        self.emit_shape();
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
        self.flush_shapes();
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
        if style.face == crate::afm::FontFace::Symbol {
            let text: String = text.chars().map(crate::afm::symbol_to_unicode).collect();
            out.push_str(&escape_xml(&text));
        } else {
            out.push_str(&escape_xml(text));
        }
        out.push_str("</text>\n");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(svg: &str) -> Vec<&str> {
        let start = svg.find("viewBox").unwrap();
        let lines: Vec<&str> = svg[start..].lines().skip(1).collect();
        lines[..lines.len() - 1].to_vec() // drop `</svg>`
    }

    /// Clips open when a page is discarded don't leave a stray `</g>`.
    #[test]
    fn new_page_drops_open_clips() {
        let mut b = SvgBackend::new(1.0, 1.0);
        b.push_clip(Rect::new(0.0, 0.0, 1.0, 1.0));
        b.new_page();
        b.pop_clip();
        b.draw_circle(Point::new(0.5, 0.5), 0.1, Some(Color::rgb(255, 0, 0)), None);
        let svg = b.finish();
        assert_eq!(svg.matches("<g").count(), svg.matches("</g>").count(), "{svg}");
    }

    /// Runs of same-style shapes share a `<g>`; a shape whose style differs
    /// from both neighbours keeps its own; text and clips end a run.
    #[test]
    fn same_style_shapes_are_grouped() {
        let red = Color::rgb(255, 0, 0);
        let blue = Color::rgb(0, 0, 255);
        let mut b = SvgBackend::new(1.0, 1.0);
        let dot = |b: &mut SvgBackend, x: f64, c: Color| {
            b.draw_circle(Point::new(x, 0.5), 0.0125, Some(c), None);
        };
        dot(&mut b, 0.125, red);
        dot(&mut b, 0.25, red);
        dot(&mut b, 0.375, red);
        dot(&mut b, 0.5, blue); // alone
        dot(&mut b, 0.625, red);
        b.draw_text(
            Point::new(0.0, 0.0),
            "t",
            &TextStyle::new(red, crate::afm::FontFace::Plain, 10.0),
        );
        dot(&mut b, 0.75, red); // the text ended the run
        b.push_clip(Rect::new(0.0, 0.0, 1.0, 1.0));
        dot(&mut b, 0.875, red);
        dot(&mut b, 0.9375, red);
        b.pop_clip();
        let svg = b.finish();
        let style = r##" fill="#ff0000" stroke="none""##;
        assert_eq!(
            body(&svg),
            vec![
                format!("<g{style}>").as_str(),
                r#"<circle cx="9" cy="36" r="0.9"/>"#,
                r#"<circle cx="18" cy="36" r="0.9"/>"#,
                r#"<circle cx="27" cy="36" r="0.9"/>"#,
                "</g>",
                r##"<circle cx="36" cy="36" r="0.9" fill="#0000ff" stroke="none"/>"##,
                format!(r#"<circle cx="45" cy="36" r="0.9"{style}/>"#).as_str(),
                concat!(
                    r##"<text x="0" y="72" text-anchor="start" font-family="Helvetica, Arial, sans-serif""##,
                    r##" font-size="10" font-style="normal" font-weight="normal" fill="#ff0000">t</text>"##
                ),
                format!(r#"<circle cx="54" cy="36" r="0.9"{style}/>"#).as_str(),
                r#"<clipPath id="clip0"><rect x="0" y="0" width="72" height="72"/></clipPath>"#,
                r#"<g clip-path="url(#clip0)">"#,
                format!("<g{style}>").as_str(),
                r#"<circle cx="63" cy="36" r="0.9"/>"#,
                r#"<circle cx="67.5" cy="36" r="0.9"/>"#,
                "</g>",
                "</g>",
            ]
        );
    }
}
