// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Points and rectangles in device inches: origin at the bottom-left of
//! the page, y increasing upward (PDF's own convention). The device and
//! backends work in these units; [`Device::plot_region`] and friends
//! return them, and [`Device::user_to_inches`] /
//! [`Device::inches_to_user`] convert to and from user coordinates.
//!
//! [`Device::plot_region`]: crate::device::Device::plot_region
//! [`Device::user_to_inches`]: crate::device::Device::user_to_inches
//! [`Device::inches_to_user`]: crate::device::Device::inches_to_user

/// A point in device inches, from the bottom-left corner of the page.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    /// The point `(x, y)`.
    pub const fn new(x: f64, y: f64) -> Self {
        Point { x, y }
    }
}

impl From<(f64, f64)> for Point {
    fn from((x, y): (f64, f64)) -> Self {
        Point::new(x, y)
    }
}

/// An axis-aligned rectangle in inches, `(x0, y0)` the bottom-left
/// corner and `(x1, y1)` the top-right corner (`x1 >= x0`, `y1 >= y0`).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Rect {
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
}

impl Rect {
    /// The rectangle with corners `(x0, y0)` and `(x1, y1)`, in either
    /// order (they are sorted).
    pub fn new(x0: f64, y0: f64, x1: f64, y1: f64) -> Self {
        Rect {
            x0: x0.min(x1),
            y0: y0.min(y1),
            x1: x0.max(x1),
            y1: y0.max(y1),
        }
    }

    /// `x1 - x0`.
    pub fn width(&self) -> f64 {
        self.x1 - self.x0
    }

    /// `y1 - y0`.
    pub fn height(&self) -> f64 {
        self.y1 - self.y0
    }
}
