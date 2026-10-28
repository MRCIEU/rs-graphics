// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! The null backend, [`NullBackend`]: draws nothing and writes nothing, but
//! measures as the PDF backend does. Open one with
//! [`Device::null`](crate::device::Device::null).

use super::{Backend, Stroke, TextStyle};
use crate::color::Color;
use crate::geometry::{Point, Rect};
use crate::numbers::Num;

/// A backend that draws nothing and produces no output, with the PDF
/// backend's measurements: text sizes rounded to whole points, a device
/// unit of 1/72 inch, and a transparent background unless one is given.
///
/// Layouts, return values (`legend()`, `axis()`, `strwidth()`) and errors
/// (margins too large for the page) are those a PDF device of the same size
/// would give, without the cost of writing anything. Open it with
/// [`Device::null`](crate::device::Device::null) rather than directly.
///
/// This is R's `pdf(NULL)`.
///
/// # Examples
///
/// ```
/// use rs_graphics::backend::null::NullBackend;
/// # use rs_graphics::prelude::*;
/// let mut dev = Device::new(NullBackend::new(7, 5), Par::default());
/// dev.plot().x([1, 2, 3]).draw();
/// assert!(dev.finish().is_empty());
/// ```
#[derive(Clone, Debug)]
pub struct NullBackend {
    width_in: f64,
    height_in: f64,
    background: Color,
}

impl NullBackend {
    /// Creates a null backend for a page of `width_in` by `height_in`
    /// inches, with a transparent background (as PDF's).
    pub fn new(width_in: impl Num, height_in: impl Num) -> Self {
        NullBackend {
            width_in: width_in.to_f64(),
            height_in: height_in.to_f64(),
            background: Color::TRANSPARENT,
        }
    }

    /// Sets the page background: what colour 0 ([`par(bg)`]) is and what a
    /// legend box is filled with. Nothing is painted.
    ///
    /// [`par(bg)`]: crate::par::Par::bg
    pub fn with_background(mut self, color: Color) -> Self {
        self.background = color;
        self
    }
}

impl Backend for NullBackend {
    fn size_inches(&self) -> (f64, f64) {
        (self.width_in, self.height_in)
    }

    fn push_clip(&mut self, _rect: Rect) {}

    fn pop_clip(&mut self) {}

    fn draw_line(&mut self, _p0: Point, _p1: Point, _stroke: &Stroke) {}

    fn draw_polyline(&mut self, _points: &[Point], _stroke: &Stroke) {}

    fn draw_polygon(&mut self, _points: &[Point], _fill: Option<Color>, _stroke: Option<&Stroke>) {}

    fn draw_rect(&mut self, _rect: Rect, _fill: Option<Color>, _stroke: Option<&Stroke>) {}

    fn draw_circle(
        &mut self,
        _center: Point,
        _radius: f64,
        _fill: Option<Color>,
        _stroke: Option<&Stroke>,
    ) {
    }

    fn draw_text(&mut self, _origin: Point, _text: &str, _style: &TextStyle) {}

    /// As R's `pdf()`: big points.
    fn device_unit_inches(&self) -> f64 {
        1.0 / 72.0
    }

    /// As R's `pdf()`: whole points.
    fn effective_font_size(&self, requested_pt: f64) -> f64 {
        super::pdf::pdf_font_size(requested_pt)
    }

    fn default_bg(&self) -> Color {
        self.background
    }

    fn new_page(&mut self) {}

    /// Nothing: there is no output.
    fn take_page(&mut self) -> Vec<u8> {
        Vec::new()
    }

    /// Nothing: there is no output.
    fn into_bytes(self) -> Vec<u8> {
        Vec::new()
    }
}
