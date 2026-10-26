// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! `abline()`: a port of `C_abline` (`plot.c`).
//!
//! Lines span `getxlimits`/`getylimits` (the plot region, the figure
//! region with `xpd = TRUE`, or the device with `xpd = NA`; inline or
//! from `par(xpd)`) and are clipped to the same region.
//! On log axes `h`/`v` are data values, and the `a`/`b` line is straight on
//! the page (`untf = FALSE`) or `y = a + b * x` in data units (`untf =
//! TRUE`).

use crate::backend::{Backend, LineEnd, LineJoin};
use crate::color::ColorSpec;
use crate::geometry::Point;
use crate::lty::LineType;
use crate::par::Xpd;
use crate::setters::pick_na;

use super::Device;

crate::setters::options! {
    /// The arguments of R's `abline()`, with R's defaults. Empty
    /// `col`/`lty`/`lwd` mean `par("col")`/`par("lty")`/`par("lwd")`;
    /// otherwise they are recycled over the lines in drawing order: each
    /// `a`/`b` line, then each `h`, then each `v`.
    ///
    /// Unlike R, `a` and `b` take several values (one line per pair), so every
    /// kind of line can be given several times. R's `coef = c(a, b)` (and
    /// `reg = fit`, through its coefficients) is `.a(..).b(..)`.
    #[derive(Clone, Debug, Default)]
    pub struct AblineOptions => [AblineCall] {
        /// Intercepts of the lines `y = a + b * x`, paired with `b` (either
        /// may have a single value, used for every line). Empty (with `b`):
        /// no such lines.
        pub a: Vec<f64> => values f64,
        /// Slopes of the lines `y = a + b * x`, paired with `a`.
        pub b: Vec<f64> => values f64,
        /// On log axes, draws each `y = a + b * x` in data units (a curve)
        /// rather than as a straight line in log units. Default `false`.
        pub untf: bool => value bool,
        /// Horizontal lines at these y values. Empty: none.
        pub h: Vec<f64> => values f64,
        /// Vertical lines at these x values. Empty: none.
        pub v: Vec<f64> => values f64,
        /// Line colours, recycled over the lines. Empty: `par(col)`.
        pub col: Vec<ColorSpec> => values ColorSpec,
        /// Line types, recycled over the lines. Empty: `par(lty)`; `None`:
        /// no line.
        pub lty: Vec<Option<LineType>> => na_values LineType,
        /// Line widths, recycled over the lines. Empty: `par(lwd)`; `None`:
        /// no line.
        pub lwd: Vec<Option<f64>> => na_values f64,
        /// How far the lines run and where they are clipped: the plot region
        /// for [`Xpd::Plot`], the figure region for [`Xpd::Figure`], the
        /// whole device for [`Xpd::Device`] (the setter also takes
        /// `false`/`true`). `None`: `par(xpd)`; overrides it for this call
        /// only.
        pub xpd: Option<Xpd> => option Xpd,
        /// How line ends are drawn: `"round"`, `"butt"` or `"square"`.
        /// `None`: `par(lend)`; overrides it for this call only.
        pub lend: Option<LineEnd> => option LineEnd,
        /// How line corners are joined: `"round"`, `"mitre"` or `"bevel"`.
        /// `None`: `par(ljoin)`; overrides it for this call only.
        pub ljoin: Option<LineJoin> => option LineJoin,
        /// Mitre limit of mitred joins (at least 1). `None`: `par(lmitre)`;
        /// overrides it for this call only.
        pub lmitre: Option<f64> => option f64,
    }
}

impl AblineOptions {
    /// The `a`/`b` lines' intercepts and slopes, `a` and `b` paired (a
    /// single value pairs with each of the other's).
    ///
    /// # Panics
    /// If only one of `a` and `b` is given (R: "invalid a=, b=
    /// specification"). R only errors for `a` alone and silently draws
    /// nothing for `b` alone; that looks like an oversight in `C_abline`,
    /// so both are errors here. If both have several values but not the
    /// same number ("'a' and 'b' lengths differ"), or a value isn't finite
    /// (R: "'a' and 'b' must be finite").
    fn lines(&self) -> Vec<(f64, f64)> {
        let (a, b) = (&self.a, &self.b);
        if a.is_empty() && b.is_empty() {
            return Vec::new();
        }
        assert!(
            !a.is_empty() && !b.is_empty(),
            "invalid a=, b= specification"
        );
        assert!(
            a.len() == b.len() || a.len() == 1 || b.len() == 1,
            "abline: 'a' and 'b' lengths differ"
        );
        let n = a.len().max(b.len());
        let lines: Vec<(f64, f64)> = (0..n).map(|i| (a[i % a.len()], b[i % b.len()])).collect();
        assert!(
            lines.iter().all(|(a, b)| a.is_finite() && b.is_finite()),
            "abline: 'a' and 'b' must be finite"
        );
        lines
    }
}

impl<B: Backend> Device<B> {
    /// Draws R's `abline()`; the public entry is
    /// [`Graphics::abline`](crate::graphics::Graphics::abline), whose docs
    /// describe it.
    pub(crate) fn abline_impl(&mut self, opts: &AblineOptions) {
        let mut dev = self.line_pars(opts.lend, opts.ljoin, opts.lmitre);
        dev.abline_draw(opts);
    }

    fn abline_draw(&mut self, opts: &AblineOptions) {
        let lines = opts.lines();
        let mut nlines = 0;
        // The style of the `n`th line: `col`, `lty` and `lwd` recycled.
        let style = |dev: &Self, n: usize| {
            let pick = |len: usize| n % len.max(1);
            let col = match opts.col.len() {
                0 => ColorSpec::Color(dev.par.col),
                l => opts.col[pick(l)],
            };
            // An `NA` colour, `lty` or `lwd` (`None`) skips the line.
            let lty = pick_na(&opts.lty, n, dev.par.lty)?;
            let lwd = pick_na(&opts.lwd, n, dev.par.lwd)?;
            dev.resolve(col).map(|c| dev.stroke_of(c, lwd, lty))
        };
        let xpd = opts.xpd.unwrap_or(self.par.xpd);
        self.push_clip_for(xpd);
        let (x0, x1) = self.xpd_limits(true, xpd);
        let (y0, y1) = self.xpd_limits(false, xpd);
        for &(a, b) in &lines {
            if let Some(stroke) = style(self, nlines) {
                if opts.untf && (self.xlog || self.ylog) {
                    if let Some(pts) = self.untf_points(a, b, xpd) {
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
    fn untf_points(&self, a: f64, b: f64, xpd: Xpd) -> Option<Vec<Point>> {
        const NS: usize = 100;
        let (l0, l1) = self.xpd_limits(true, xpd);
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
