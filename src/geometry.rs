// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Points and rectangles in device inches.
//!
//! Device inches have their origin at the bottom-left of the page, with y
//! increasing upward (PDF's own convention). The device and backends work
//! in these units; [`Device::plot_region`] and friends return them, and
//! [`Device::user_to_inches`] / [`Device::inches_to_user`] convert to and
//! from user coordinates.
//!
//! [`Device::plot_region`]: crate::device::Device::plot_region
//! [`Device::user_to_inches`]: crate::device::Device::user_to_inches
//! [`Device::inches_to_user`]: crate::device::Device::inches_to_user

/// A point in device inches, from the bottom-left corner of the page.
///
/// Converts from an `(x, y)` tuple.
///
/// # Examples
///
/// ```
/// # use rs_graphics::prelude::*;
/// let p: Point = (1.5, 2.0).into();
/// assert_eq!(p, Point::new(1.5, 2.0));
/// assert_eq!(p.y, 2.0);
/// ```
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Point {
    /// Inches from the left edge of the page.
    pub x: f64,
    /// Inches from the bottom edge of the page.
    pub y: f64,
}

impl Point {
    /// Returns the point `(x, y)`, in inches.
    pub const fn new(x: f64, y: f64) -> Self {
        Point { x, y }
    }
}

impl From<(f64, f64)> for Point {
    fn from((x, y): (f64, f64)) -> Self {
        Point::new(x, y)
    }
}

/// An axis-aligned rectangle in device inches.
///
/// `(x0, y0)` is the bottom-left corner and `(x1, y1)` the top-right corner
/// (`x1 >= x0`, `y1 >= y0`; [`Rect::new`] sorts them).
///
/// # Examples
///
/// ```
/// # use rs_graphics::prelude::*;
/// # let mut dev = Device::new(SvgBackend::new(7, 7), Par::default());
/// dev.plot_new();
/// let plot: Rect = dev.plot_region();
/// assert!(plot.width() > 0.0 && plot.x1 <= 7.0);
/// ```
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Rect {
    /// Left edge.
    pub x0: f64,
    /// Bottom edge.
    pub y0: f64,
    /// Right edge.
    pub x1: f64,
    /// Top edge.
    pub y1: f64,
}

impl Rect {
    /// Returns the rectangle with corners `(x0, y0)` and `(x1, y1)`, given
    /// in either order (they are sorted).
    ///
    /// # Examples
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// let r = Rect::new(3.0, 2.0, 1.0, 4.0);
    /// assert_eq!((r.x0, r.y0, r.x1, r.y1), (1.0, 2.0, 3.0, 4.0));
    /// assert_eq!((r.width(), r.height()), (2.0, 2.0));
    /// ```
    pub fn new(x0: f64, y0: f64, x1: f64, y1: f64) -> Self {
        Rect {
            x0: x0.min(x1),
            y0: y0.min(y1),
            x1: x0.max(x1),
            y1: y0.max(y1),
        }
    }

    /// Returns the width, `x1 - x0`.
    pub fn width(&self) -> f64 {
        self.x1 - self.x0
    }

    /// Returns the height, `y1 - y0`.
    pub fn height(&self) -> f64 {
        self.y1 - self.y0
    }
}
