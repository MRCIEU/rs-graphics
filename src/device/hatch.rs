// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Polygons as R's `polygon()` draws them, including its shading lines
//! (`density`/`angle`): a port of `polygon.R` (`polygon.fullhatch()` and
//! `polygon.onehatch()`, by Kevin Buhr). `rect()`, `barplot()` and
//! `legend()` boxes go through here too, as in R, where they call
//! `polygon()` when shading.

use super::Device;
use super::primitives::pick;
use crate::backend::{Backend, Stroke};
use crate::color::ColorSpec;
use crate::geometry::Point;
use crate::lty::LineType;
use crate::setters::pick_na;

/// Whether R shades: some `density` is given and not `NA` or negative
/// (`polygon()`/`rect()` treat a `density` that is all `NA` or negative
/// as not given). R tests this with `is.numeric(density)`, so a logical
/// `density = NA` takes the shading path anyway, filling with `col`
/// (`par(fg)` by default) and outlining in it (an R bug, not copied:
/// `None`s are R's `NA_real_`).
pub(crate) fn shading(density: &[Option<f64>]) -> bool {
    density.iter().any(|d| d.is_some_and(|d| d >= 0.0))
}

/// A run of finite points, in data units and in device inches.
type Run = (Vec<(f64, f64)>, Vec<Point>);

/// Per-polygon styles, each recycled over the polygons (an empty slice is
/// R's default; see [`Device::polygons`]).
pub(crate) struct ShapeStyle<'a> {
    pub col: &'a [ColorSpec],
    pub border: &'a [ColorSpec],
    pub lty: &'a [Option<LineType>],
    pub lwd: &'a [Option<f64>],
    pub density: &'a [Option<f64>],
    pub angle: &'a [f64],
    /// R's `fillOddEven`: shade where the border goes round an odd number
    /// of times, rather than a non-zero number.
    pub odd_even: bool,
}

impl<B: Backend> Device<B> {
    /// Draws the polygons in `x`/`y` (data units; a non-finite point ends
    /// one polygon), without setting a clip.
    ///
    /// Without shading (see [`shading`]) this is `C_polygon`: each polygon
    /// of at least two points is filled with `col` (empty: none) and
    /// outlined with `border` (empty: `par(fg)`), `lty` and `lwd`, in turn.
    ///
    /// With shading, as `polygon.R`: `col` is the colour of the shading
    /// lines (empty or `None` elements: `par(fg)`) and `border` defaults to
    /// it. Each polygon in turn is shaded with lines (`density` > 0, lines
    /// per inch, at `angle` degrees, default 45), left empty (`density` 0)
    /// or filled with its `col` (`density` `None` or negative); then all
    /// borders are drawn. Shading is skipped on log axes (R warns
    /// "cannot hatch with logarithmic scale active").
    pub(crate) fn polygons(&mut self, x: &[f64], y: &[f64], s: &ShapeStyle) {
        // Runs of finite points, in data units and in inches.
        let mut runs: Vec<Run> = Vec::new();
        let mut run = (Vec::new(), Vec::new());
        let n = x.len().min(y.len());
        for i in 0..=n {
            let p = (i < n).then(|| self.user_to_inches(x[i], y[i]));
            if let Some(p) = p.filter(|p| p.x.is_finite() && p.y.is_finite()) {
                run.0.push((x[i], y[i]));
                run.1.push(p);
            } else if !run.1.is_empty() {
                runs.push(std::mem::take(&mut run));
            }
        }
        let fg = ColorSpec::Color(self.par.fg);
        let (lty, lwd) = (self.par.lty, self.par.lwd);
        let shade = shading(s.density);

        // `col`/`border` as `C_polygon` gets them: with shading, `NA`
        // colours are `par(fg)` and the border defaults to them.
        let col = |i: usize| match pick(s.col, i, ColorSpec::None) {
            ColorSpec::None if shade => fg,
            c => c,
        };
        let border = |i: usize| {
            if shade && s.border.is_empty() {
                col(i)
            } else {
                pick(s.border, i, fg)
            }
        };

        if shade {
            // R's loop counts every run of at least one point.
            for (i, (pts, inches)) in runs.iter().enumerate() {
                match pick_na(s.density, i, f64::NAN).filter(|d| !d.is_nan()) {
                    Some(d) if d > 0.0 => {
                        if self.xlog || self.ylog {
                            continue;
                        }
                        let (Some(c), Some(lwd), Some(lty)) = (
                            self.resolve(col(i)),
                            pick_na(s.lwd, i, lwd),
                            pick_na(s.lty, i, lty),
                        ) else {
                            continue;
                        };
                        let stroke = self.stroke_of(c, lwd, lty);
                        let angle = pick(s.angle, i, 45.0);
                        self.hatch(pts, d, angle, s.odd_even, &stroke);
                    }
                    Some(0.0) => {}
                    _ => {
                        if inches.len() >= 2 {
                            let fill = self.resolve(col(i));
                            self.backend.draw_polygon(inches, fill, None);
                        }
                    }
                }
            }
        }

        // `C_polygon`'s polygons have at least two points; shorter runs
        // don't use up a style. With shading, R's last call draws only
        // the borders.
        for (num, (_, inches)) in runs.iter().filter(|r| r.1.len() >= 2).enumerate() {
            let fill = if shade { None } else { self.resolve(col(num)) };
            // An `NA` `lty` or `lwd` leaves out the border; the fill is drawn.
            let line = pick_na(s.lwd, num, lwd).zip(pick_na(s.lty, num, lty));
            let stroke = self
                .resolve(border(num))
                .zip(line)
                .map(|(c, (lwd, lty))| self.stroke_of(c, lwd, lty));
            self.backend.draw_polygon(inches, fill, stroke.as_ref());
        }
    }

    /// `polygon.fullhatch()`: shading lines `density` per inch at `angle`
    /// degrees across the polygon `pts` (`usr` units, linear axes), placed
    /// so that, extended, they would pass through the `usr` origin.
    fn hatch(
        &mut self,
        pts: &[(f64, f64)],
        density: f64,
        angle: f64,
        odd_even: bool,
        stroke: &Stroke,
    ) {
        let mut x: Vec<f64> = pts.iter().map(|p| p.0).collect();
        let mut y: Vec<f64> = pts.iter().map(|p| p.1).collect();
        x.push(x[0]);
        y.push(y[0]);
        let mut angle = angle.rem_euclid(180.0);
        let (ux0, ux1, uy0, uy1) = self.usr;
        // `usr` units per inch; flipped axes flip the angle.
        let mut upi = [
            (ux1 - ux0) / self.plt.width(),
            (uy1 - uy0) / self.plt.height(),
        ];
        if upi[0] < 0.0 {
            angle = 180.0 - angle;
        }
        if upi[1] < 0.0 {
            angle = 180.0 - angle;
        }
        upi = [upi[0].abs(), upi[1].abs()];
        let rad = angle / 180.0 * std::f64::consts::PI;
        let xd = rad.cos() * upi[0];
        let yd = rad.sin() * upi[1];
        let min = |v: &[f64]| v.iter().copied().fold(f64::INFINITY, f64::min);
        let max = |v: &[f64]| v.iter().copied().fold(f64::NEG_INFINITY, f64::max);

        if !(45.0..=135.0).contains(&angle) {
            // Lines stepped vertically; `first_x`/`last_x` are where the
            // bounding box is first and last hit as y increases.
            let (first_x, last_x) = if angle < 45.0 {
                (max(&x), min(&x))
            } else {
                (min(&x), max(&x))
            };
            let y_shift = upi[1] / density / rad.cos().abs();
            if !(y_shift > 0.0 && y_shift.is_finite()) {
                return;
            }
            let mut y0 = ((min(&y) - first_x * yd / xd) / y_shift).floor() * y_shift;
            let y_end = max(&y) - last_x * yd / xd;
            while y0 < y_end {
                self.hatch_line(&x, &y, (0.0, y0), (xd, yd), odd_even, stroke);
                y0 += y_shift;
            }
        } else {
            let (first_y, last_y) = if angle < 90.0 {
                (max(&y), min(&y))
            } else {
                (min(&y), max(&y))
            };
            let x_shift = upi[0] / density / rad.sin().abs();
            if !(x_shift > 0.0 && x_shift.is_finite()) {
                return;
            }
            let mut x0 = ((min(&x) - first_y * xd / yd) / x_shift).floor() * x_shift;
            let x_end = max(&x) - last_y * xd / yd;
            while x0 < x_end {
                self.hatch_line(&x, &y, (x0, 0.0), (xd, yd), odd_even, stroke);
                x0 += x_shift;
            }
        }
    }

    /// `polygon.onehatch()`: the parts of the line `(x0, y0) + t (xd, yd)`
    /// inside the closed polygon `x`/`y` (non-zero winding, or with
    /// `odd_even` an odd number of crossings).
    fn hatch_line(
        &mut self,
        x: &[f64],
        y: &[f64],
        (x0, y0): (f64, f64),
        (xd, yd): (f64, f64),
        odd_even: bool,
        stroke: &Stroke,
    ) {
        // Which side of the line each point is on, and where (and which
        // way) each edge crosses it.
        let half: Vec<i32> = (0..x.len())
            .map(|i| (xd * (y[i] - y0) - yd * (x[i] - x0) <= 0.0) as i32)
            .collect();
        let mut t = Vec::new();
        let mut cross = Vec::new();
        for i in 0..x.len() - 1 {
            let c = half[i + 1] - half[i];
            if c != 0 {
                let (x1, y1, x2, y2) = (x[i], y[i], x[i + 1], y[i + 1]);
                t.push(
                    ((x1 - x0) * (y2 - y1) - (y1 - y0) * (x2 - x1))
                        / (xd * (y2 - y1) - yd * (x2 - x1)),
                );
                cross.push(c);
            }
        }
        if t.is_empty() {
            return;
        }
        // `order(t)`: stable.
        let mut o: Vec<usize> = (0..t.len()).collect();
        o.sort_by(|&a, &b| t[a].partial_cmp(&t[b]).unwrap_or(std::cmp::Ordering::Equal));
        let mut crossings = 0;
        for k in 0..o.len() - 1 {
            crossings += cross[o[k]];
            // `if (fillOddEven) crossings <- crossings %% 2`.
            let inside = if odd_even {
                crossings.rem_euclid(2)
            } else {
                crossings
            };
            if inside == 0 {
                continue;
            }
            let (ta, tb) = (t[o[k]], t[o[k + 1]]);
            let p0 = self.usr_to_inches(x0 + xd * ta, y0 + yd * ta);
            let p1 = self.usr_to_inches(x0 + xd * tb, y0 + yd * tb);
            if [p0.x, p0.y, p1.x, p1.y].iter().all(|v| v.is_finite()) {
                self.backend.draw_line(p0, p1, stroke);
            }
        }
    }
}
