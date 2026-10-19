// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! `abline()`: a port of `C_abline` (`plot.c`).
//!
//! Lines span `getxlimits`/`getylimits` (the plot region, or the figure
//! region with `par(xpd = TRUE)`) and are clipped to the `par(xpd)` region.
//! On log axes `h`/`v` are data values, and the `a`/`b` line is straight on
//! the page (`untf = FALSE`) or `y = a + b * x` in data units (`untf =
//! TRUE`).

use crate::backend::Backend;
use crate::color::ColorSpec;
use crate::geometry::Point;
use crate::lty::LineType;

use super::Device;

/// The arguments of R's `abline()` (`a`/`b`, `coef` and, through its
/// coefficients, `reg` all go in `coef`), with R's
/// defaults. Empty `col`/`lty`/`lwd` mean `par("col")`/`par("lty")`/
/// `par("lwd")`; otherwise they are recycled over the lines in drawing
/// order: the `a`/`b` line, then each `h`, then each `v`.
#[derive(Clone, Debug, Default)]
pub struct AblineOptions {
    /// Intercept and slope of a line (`a`, `b`); `None`: no such line.
    pub coef: Option<(f64, f64)>,
    /// `untf = TRUE`: on log axes, draw `y = a + b * x` in data units (a
    /// curve) rather than a straight line in log units. Default `false`.
    pub untf: bool,
    /// Horizontal lines at these y values; empty: none.
    pub h: Vec<f64>,
    /// Vertical lines at these x values; empty: none.
    pub v: Vec<f64>,
    /// Colours, recycled over the lines; empty: `par(col)`.
    pub col: Vec<ColorSpec>,
    /// Line types, recycled over the lines; empty: `par(lty)`.
    pub lty: Vec<LineType>,
    /// Line widths, recycled over the lines; empty: `par(lwd)`.
    pub lwd: Vec<f64>,
}

crate::setters::setters!(AblineOptions => [AblineCall] {
    coef: pair f64,
    untf: value bool,
    h: values f64,
    v: values f64,
    col: values ColorSpec,
    lty: values LineType,
    lwd: values f64,
});

impl<B: Backend> Device<B> {
    /// Draws R's `abline()`; the public entry is
    /// [`Graphics::abline`](crate::graphics::Graphics::abline), whose docs
    /// describe it.
    pub(crate) fn abline_impl(&mut self, opts: &AblineOptions) {
        if let Some((a, b)) = opts.coef {
            assert!(
                a.is_finite() && b.is_finite(),
                "abline: 'a' and 'b' must be finite"
            );
        }
        let mut nlines = 0;
        // The style of the `n`th line: `col`, `lty` and `lwd` recycled.
        let style = |dev: &Self, n: usize| {
            let pick = |len: usize| n % len.max(1);
            let col = match opts.col.len() {
                0 => ColorSpec::Color(dev.par.col),
                l => opts.col[pick(l)],
            };
            let lty = match opts.lty.len() {
                0 => dev.par.lty,
                l => opts.lty[pick(l)],
            };
            let lwd = match opts.lwd.len() {
                0 => dev.par.lwd,
                l => opts.lwd[pick(l)],
            };
            dev.resolve(col).map(|c| dev.stroke_of(c, lwd, lty))
        };
        self.push_clip_for(self.par.xpd);
        let (x0, x1) = self.xpd_limits(true);
        let (y0, y1) = self.xpd_limits(false);
        if let Some((a, b)) = opts.coef {
            if let Some(stroke) = style(self, 0) {
                if opts.untf && (self.xlog || self.ylog) {
                    if let Some(pts) = self.untf_points(a, b) {
                        self.backend.draw_polyline(&pts, &stroke);
                    }
                } else {
                    // In `usr` units the line is `a + b * x` whether or not
                    // either axis is logarithmic.
                    let from = self.usr_to_inches(x0, a + b * x0);
                    let to = self.usr_to_inches(x1, a + b * x1);
                    self.draw_line_if_finite(from, to, &stroke);
                }
            }
            nlines += 1;
        }
        for &h in &opts.h {
            if let Some(stroke) = style(self, nlines) {
                let h = if self.ylog { h.log10() } else { h };
                let (from, to) = (self.usr_to_inches(x0, h), self.usr_to_inches(x1, h));
                self.draw_line_if_finite(from, to, &stroke);
            }
            nlines += 1;
        }
        for &v in &opts.v {
            if let Some(stroke) = style(self, nlines) {
                let v = if self.xlog { v.log10() } else { v };
                let (from, to) = (self.usr_to_inches(v, y0), self.usr_to_inches(v, y1));
                self.draw_line_if_finite(from, to, &stroke);
            }
            nlines += 1;
        }
        self.pop_clip();
    }

    /// The `untf = TRUE` curve `y = a + b * x` through 101 points (equally
    /// spaced in `log10(x)` on a log x axis), with the ends where x or y is
    /// <= 0 on a log axis dropped; `None` if nothing is left or a point
    /// can't be placed.
    fn untf_points(&self, a: f64, b: f64) -> Option<Vec<Point>> {
        const NS: usize = 100;
        let (l0, l1) = self.xpd_limits(true);
        let data = |v: f64| if self.xlog { 10f64.powf(v) } else { v };
        let (mut x0, x1) = (data(l0), data(l1));
        let mut xx = vec![0.0; NS + 1];
        if self.xlog {
            x0 = x0.max(1.01 * (x1 / f64::MAX));
            let f = (x1 / x0).powf(1.0 / NS as f64);
            xx[0] = x0;
            for i in 1..NS {
                xx[i] = xx[i - 1] * f;
            }
        } else {
            let step = (x1 - x0) / NS as f64;
            for (i, x) in xx.iter_mut().enumerate().take(NS) {
                *x = x0 + i as f64 * step;
            }
        }
        xx[NS] = x1;
        let yy: Vec<f64> = xx.iter().map(|&x| a + x * b).collect();
        let positive = |i: usize| (!self.xlog || xx[i] > 0.0) && (!self.ylog || yy[i] > 0.0);
        let (mut start, mut stop) = (0, NS);
        while start <= NS && !positive(start) {
            start += 1;
        }
        while stop > 0 && !positive(stop) {
            stop -= 1;
        }
        if start > stop {
            return None;
        }
        let pts: Vec<Point> = (start..=stop)
            .map(|i| self.user_to_inches(xx[i], yy[i]))
            .collect();
        pts.iter()
            .all(|p| p.x.is_finite() && p.y.is_finite())
            .then_some(pts)
    }
}
