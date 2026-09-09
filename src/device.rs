//! The stateful graphics device: coordinate transforms (`plot.new()`/
//! `plot.window()`), clipping, and the primitive drawing operations
//! (`points`, `lines`, `segments`, `rect`, `polygon`, `text`, `box`,
//! `abline`, `axis`, `title`, `legend`, `arrows`).
//!
//! This is intentionally a single, fairly large module: nearly every
//! high-level plot (`plot()`, `hist()`, `barplot()`, ...) is built purely
//! out of the primitives here, mirroring how R's own `graphics.c` centers
//! everything around a handful of `GE*` engine calls.

use crate::afm::{self};
use crate::backend::{Backend, Stroke, TextStyle};
use crate::color::{Color, ColorSpec};
use crate::geometry::{Point, Rect};
use crate::lty::LineType;
use crate::par::{AxisStyle, Par};
use crate::pch;
use crate::pretty::ge_pretty;

/// Which axis/box side, in R's `side` convention: `1` = bottom, `2` =
/// left, `3` = top, `4` = right.
pub type Side = i32;

pub struct Device<B: Backend> {
    backend: B,
    pub par: Par,
    width_in: f64,
    height_in: f64,
    /// Figure region (device inches minus outer margins).
    fig: Rect,
    /// Plot region (figure region minus `mar` margins).
    plt: Rect,
    /// `(x0, x1, y0, y1)` in "user" coordinates - already log10-
    /// transformed if `xlog`/`ylog` is set, matching `par("usr")`.
    usr: (f64, f64, f64, f64),
    xlog: bool,
    ylog: bool,
}

/// A resolved style bundle for a stroked primitive; kept small so
/// call-sites can build one inline.
#[derive(Clone, Copy, Debug)]
pub struct LineStyle {
    pub col: ColorSpec,
    pub lwd: f64,
    pub lty: LineType,
}

impl LineStyle {
    pub fn new(col: ColorSpec, lwd: f64, lty: LineType) -> Self {
        LineStyle { col, lwd, lty }
    }
}

impl<B: Backend> Device<B> {
    pub fn new(backend: B, par: Par) -> Self {
        let (width_in, height_in) = backend.size_inches();
        let mut dev = Device {
            backend,
            par,
            width_in,
            height_in,
            fig: Rect::new(0.0, 0.0, width_in, height_in),
            plt: Rect::new(0.0, 0.0, width_in, height_in),
            usr: (0.0, 1.0, 0.0, 1.0),
            xlog: false,
            ylog: false,
        };
        dev.recompute_layout();
        dev
    }

    pub fn into_backend(self) -> B {
        self.backend
    }

    pub fn width_in(&self) -> f64 {
        self.width_in
    }

    pub fn height_in(&self) -> f64 {
        self.height_in
    }

    pub fn plot_region(&self) -> Rect {
        self.plt
    }

    pub fn usr(&self) -> (f64, f64, f64, f64) {
        self.usr
    }

    fn recompute_layout(&mut self) {
        let omi = self.par.omi();
        self.fig = Rect::new(
            omi.left,
            omi.bottom,
            self.width_in - omi.right,
            self.height_in - omi.top,
        );
        let mai = self.par.mai();
        self.plt = Rect::new(
            self.fig.x0 + mai.left,
            self.fig.y0 + mai.bottom,
            self.fig.x1 - mai.right,
            self.fig.y1 - mai.top,
        );
    }

    /// `plot.new()`: clears the device to `par("bg")` and (re)computes the
    /// figure/plot regions from the current margins.
    pub fn plot_new(&mut self) {
        self.recompute_layout();
        self.backend.draw_rect(
            Rect::new(0.0, 0.0, self.width_in, self.height_in),
            Some(self.par.bg),
            None,
        );
    }

    /// `plot.window(xlim, ylim, log)`: sets up the user-coordinate system.
    /// Applies R's default `xaxs = yaxs = "r"` 4%-range-expansion (see
    /// `GScale()` in `graphics.c`) unless the axis style is `"i"`.
    pub fn plot_window(&mut self, xlim: (f64, f64), ylim: (f64, f64), xlog: bool, ylog: bool) {
        self.xlog = xlog;
        self.ylog = ylog;
        let (x0, x1) = if xlog {
            (xlim.0.log10(), xlim.1.log10())
        } else {
            xlim
        };
        let (y0, y1) = if ylog {
            (ylim.0.log10(), ylim.1.log10())
        } else {
            ylim
        };
        let (ux0, ux1) = Self::expand_range(x0, x1, self.par.xaxs);
        let (uy0, uy1) = Self::expand_range(y0, y1, self.par.yaxs);
        self.usr = (ux0, ux1, uy0, uy1);
    }

    /// The default `xaxs = "r"` / `yaxs = "r"` range expansion (4% of the
    /// range on each side), matching `GScale()`'s `case 'r'` branch.
    /// `"i"` (exact) applies no expansion. This intentionally skips
    /// `GScale`'s extra edge-case handling for near-zero/non-finite
    /// ranges - see the crate README for the fidelity tradeoffs made
    /// here.
    fn expand_range(min: f64, max: f64, style: AxisStyle) -> (f64, f64) {
        match style {
            AxisStyle::Exact => (min, max),
            AxisStyle::Regular => {
                let temp = if min.abs().max(max.abs()) > 100.0 {
                    0.04 * max - 0.04 * min
                } else {
                    0.04 * (max - min)
                };
                (min - temp, max + temp)
            }
        }
    }

    /// Default tick locations R's `axis()` would pick for `side` when no
    /// `at=` is given: `pretty()` on the current `usr` range for that
    /// axis, with `n = 5` (R's `par("lab")[1:2]` default).
    pub fn default_ticks(&self, side: Side) -> Vec<f64> {
        let (lo, hi) = if side == 1 || side == 3 {
            (self.usr.0, self.usr.1)
        } else {
            (self.usr.2, self.usr.3)
        };
        let (lo, hi, ndiv) = ge_pretty(lo.min(hi), lo.max(hi), 5);
        (0..=ndiv)
            .map(|i| lo + i as f64 * (hi - lo) / ndiv as f64)
            .collect()
    }

    // ---------------------------------------------------------------
    // Coordinate transforms
    // ---------------------------------------------------------------

    /// Map a user-coordinate point to device inches.
    pub fn user_to_inches(&self, x: f64, y: f64) -> Point {
        let ux = if self.xlog { x.log10() } else { x };
        let uy = if self.ylog { y.log10() } else { y };
        let (x0, x1, y0, y1) = self.usr;
        let px = self.plt.x0 + (ux - x0) / (x1 - x0) * self.plt.width();
        let py = self.plt.y0 + (uy - y0) / (y1 - y0) * self.plt.height();
        Point::new(px, py)
    }

    fn line_height(&self) -> f64 {
        self.par.line_height_inches()
    }

    // ---------------------------------------------------------------
    // Clipping (matches R's `xpd`: FALSE clips to the plot region, TRUE
    // clips to the figure region - we treat R's `NA`, "clip to device",
    // the same as figure-region clipping, since we don't yet support
    // outer margins wide enough for that distinction to usually matter).
    // ---------------------------------------------------------------

    fn push_clip_for(&mut self, xpd: bool) {
        let rect = if xpd { self.fig } else { self.plt };
        self.backend.push_clip(rect);
    }

    fn pop_clip(&mut self) {
        self.backend.pop_clip();
    }

    // ---------------------------------------------------------------
    // Primitives
    // ---------------------------------------------------------------

    fn resolve(&self, spec: ColorSpec) -> Option<Color> {
        self.par.resolve_color(spec)
    }

    /// `points(x, y, pch=, col=, bg=, cex=)`.
    pub fn points(
        &mut self,
        xs: &[f64],
        ys: &[f64],
        pch: i32,
        col: ColorSpec,
        bg: ColorSpec,
        cex: f64,
    ) {
        let Some(col) = self.resolve(col) else { return };
        let bg = self.resolve(bg);
        let size = pch::symbol_size_inches(self.par.ps, self.par.cex * cex);
        self.push_clip_for(self.par.xpd);
        for (&x, &y) in xs.iter().zip(ys) {
            if !x.is_finite() || !y.is_finite() {
                continue;
            }
            let p = self.user_to_inches(x, y);
            self.draw_pch(pch, p, size, col, bg);
        }
        self.pop_clip();
    }

    fn draw_pch(&mut self, pch: i32, p: Point, size: f64, col: Color, bg: Option<Color>) {
        if let Some(geom) = pch::pch_geometry(pch, p.x, p.y, size) {
            let stroke = Stroke::simple(col, self.par.lwd);
            for shape in &geom.outline_only {
                self.paint_shape(shape, None, Some(&stroke));
            }
            for shape in &geom.filled_with_col {
                self.paint_shape(shape, Some(col), None);
            }
            for shape in &geom.filled_with_bg {
                self.paint_shape(shape, bg, Some(&stroke));
            }
            for &(a, b) in &geom.lines {
                self.backend
                    .draw_line(Point::new(a.0, a.1), Point::new(b.0, b.1), &stroke);
            }
        } else if pch == '.' as i32 {
            let xc = (cex_factor(size)).max(0.5) / 72.0;
            self.backend.draw_rect(
                Rect::new(p.x - xc, p.y - xc, p.x + xc, p.y + xc),
                Some(col),
                None,
            );
        } else if (32..=126).contains(&pch) {
            let ch = char::from_u32(pch as u32).unwrap_or('?');
            let face = self.par.font_face(self.par.font);
            let size_pt = self.par.ps * self.par.cex;
            let style = TextStyle::new(col, face, size_pt);
            let w = afm::str_width_inches(&ch.to_string(), face, size_pt, 1.0);
            let (asc, desc) = afm::ascent_descent_inches(face, size_pt, 1.0);
            let origin = resolve_text_origin(p, w, asc, desc, 0.5, 0.5, 0.0);
            self.backend.draw_text(origin, &ch.to_string(), &style);
        }
    }

    fn paint_shape(&mut self, shape: &pch::Shape, fill: Option<Color>, stroke: Option<&Stroke>) {
        match shape {
            pch::Shape::Circle { cx, cy, r } => {
                self.backend
                    .draw_circle(Point::new(*cx, *cy), *r, fill, stroke);
            }
            pch::Shape::Rect { x0, y0, x1, y1 } => {
                self.backend
                    .draw_rect(Rect::new(*x0, *y0, *x1, *y1), fill, stroke);
            }
            pch::Shape::Polygon(pts) => {
                let pts: Vec<Point> = pts.iter().map(|&(x, y)| Point::new(x, y)).collect();
                self.backend.draw_polygon(&pts, fill, stroke);
            }
        }
    }

    /// `lines(x, y, ...)`: connects points with a polyline, breaking into
    /// separate segments at non-finite (`NA`) coordinates, matching R.
    pub fn lines(&mut self, xs: &[f64], ys: &[f64], style: LineStyle) {
        let Some(col) = self.resolve(style.col) else {
            return;
        };
        let stroke = self.stroke_of(col, style.lwd, style.lty);
        self.push_clip_for(self.par.xpd);
        let mut run: Vec<Point> = Vec::new();
        for (&x, &y) in xs.iter().zip(ys) {
            if x.is_finite() && y.is_finite() {
                run.push(self.user_to_inches(x, y));
            } else if run.len() >= 2 {
                self.backend.draw_polyline(&run, &stroke);
                run.clear();
            } else {
                run.clear();
            }
        }
        if run.len() >= 2 {
            self.backend.draw_polyline(&run, &stroke);
        }
        self.pop_clip();
    }

    /// `segments(x0, y0, x1, y1, ...)`.
    pub fn segments(&mut self, x0: &[f64], y0: &[f64], x1: &[f64], y1: &[f64], style: LineStyle) {
        let Some(col) = self.resolve(style.col) else {
            return;
        };
        let stroke = self.stroke_of(col, style.lwd, style.lty);
        self.push_clip_for(self.par.xpd);
        for i in 0..x0.len().min(y0.len()).min(x1.len()).min(y1.len()) {
            if [x0[i], y0[i], x1[i], y1[i]].iter().all(|v| v.is_finite()) {
                let p0 = self.user_to_inches(x0[i], y0[i]);
                let p1 = self.user_to_inches(x1[i], y1[i]);
                self.backend.draw_line(p0, p1, &stroke);
            }
        }
        self.pop_clip();
    }

    /// `rect(xleft, ybottom, xright, ytop, ...)` in user coordinates.
    pub fn rect(
        &mut self,
        x0: f64,
        y0: f64,
        x1: f64,
        y1: f64,
        fill: ColorSpec,
        border: ColorSpec,
        lwd: f64,
        lty: LineType,
    ) {
        let fill_c = self.resolve(fill);
        let border_c = self.resolve(border);
        let stroke = border_c.map(|c| self.stroke_of(c, lwd, lty));
        self.push_clip_for(self.par.xpd);
        let p0 = self.user_to_inches(x0, y0);
        let p1 = self.user_to_inches(x1, y1);
        self.backend
            .draw_rect(Rect::new(p0.x, p0.y, p1.x, p1.y), fill_c, stroke.as_ref());
        self.pop_clip();
    }

    /// `rect()` variant taking a rectangle already in device inches
    /// (used internally for legends and margin decorations).
    pub fn rect_inches(&mut self, rect: Rect, fill: Option<Color>, stroke: Option<&Stroke>) {
        self.backend.draw_rect(rect, fill, stroke);
    }

    /// `polygon(x, y, ...)`.
    pub fn polygon(
        &mut self,
        xs: &[f64],
        ys: &[f64],
        fill: ColorSpec,
        border: ColorSpec,
        lwd: f64,
        lty: LineType,
    ) {
        let fill_c = self.resolve(fill);
        let border_c = self.resolve(border);
        let stroke = border_c.map(|c| self.stroke_of(c, lwd, lty));
        self.push_clip_for(self.par.xpd);
        let pts: Vec<Point> = xs
            .iter()
            .zip(ys)
            .map(|(&x, &y)| self.user_to_inches(x, y))
            .collect();
        self.backend.draw_polygon(&pts, fill_c, stroke.as_ref());
        self.pop_clip();
    }

    fn stroke_of(&self, color: Color, lwd: f64, lty: LineType) -> Stroke {
        Stroke {
            color,
            lwd,
            lty,
            lend: self.par.lend,
            ljoin: self.par.ljoin,
            lmitre: self.par.lmitre,
        }
    }

    /// `abline(a=, b=)` / `abline(h=)` / `abline(v=)`.
    pub fn abline(
        &mut self,
        a: Option<f64>,
        b: Option<f64>,
        h: Option<f64>,
        v: Option<f64>,
        col: ColorSpec,
        lwd: f64,
        lty: LineType,
    ) {
        let Some(color) = self.resolve(col) else {
            return;
        };
        let stroke = self.stroke_of(color, lwd, lty);
        self.push_clip_for(false); // abline always clips to the plot region
        let (x0, x1, y0, y1) = self.usr;
        if let Some(h) = h {
            let p0 = self.user_to_inches(
                if self.xlog { 10f64.powf(x0) } else { x0 },
                if self.ylog { 10f64.powf(h) } else { h },
            );
            let p1 = self.user_to_inches(
                if self.xlog { 10f64.powf(x1) } else { x1 },
                if self.ylog { 10f64.powf(h) } else { h },
            );
            self.backend.draw_line(p0, p1, &stroke);
        }
        if let Some(v) = v {
            let p0 = self.user_to_inches(
                if self.xlog { 10f64.powf(v) } else { v },
                if self.ylog { 10f64.powf(y0) } else { y0 },
            );
            let p1 = self.user_to_inches(
                if self.xlog { 10f64.powf(v) } else { v },
                if self.ylog { 10f64.powf(y1) } else { y1 },
            );
            self.backend.draw_line(p0, p1, &stroke);
        }
        if let (Some(a), Some(b)) = (a, b) {
            // Evaluate y = a + b*x at the two ends of the (possibly log)
            // user x-range, in the *original* (non-log) data space.
            let ux0 = if self.xlog { 10f64.powf(x0) } else { x0 };
            let ux1 = if self.xlog { 10f64.powf(x1) } else { x1 };
            let uy0 = a + b * ux0;
            let uy1 = a + b * ux1;
            let p0 = self.user_to_inches(ux0, uy0);
            let p1 = self.user_to_inches(ux1, uy1);
            self.backend.draw_line(p0, p1, &stroke);
        }
        self.pop_clip();
    }

    // ---------------------------------------------------------------
    // Text
    // ---------------------------------------------------------------

    /// Width/height (inches) `strwidth()`/`strheight()` would report for
    /// `text` under the current `par(font=, ps=)` (times `cex`).
    pub fn text_metrics(&self, text: &str, font: i32, cex: f64) -> (f64, f64) {
        let face = self.par.font_face(font);
        let size = self.par.ps * cex;
        (
            afm::str_width_inches(text, face, size, 1.0),
            afm::str_height_inches(face, size, 1.0),
        )
    }

    /// `text(x, y, labels, adj=, srt=, col=, cex=, font=)` in user
    /// coordinates.
    #[allow(clippy::too_many_arguments)]
    pub fn text(
        &mut self,
        x: f64,
        y: f64,
        label: &str,
        hadj: f64,
        vadj: f64,
        rotation_deg: f64,
        col: ColorSpec,
        cex: f64,
        font: i32,
    ) {
        let p = self.user_to_inches(x, y);
        self.text_at(p, label, hadj, vadj, rotation_deg, col, cex, font);
    }

    /// As [`Device::text`] but the anchor is already in device inches
    /// (used for axis labels, titles, legends, ...).
    #[allow(clippy::too_many_arguments)]
    pub fn text_at(
        &mut self,
        anchor: Point,
        label: &str,
        hadj: f64,
        vadj: f64,
        rotation_deg: f64,
        col: ColorSpec,
        cex: f64,
        font: i32,
    ) {
        let Some(color) = self.resolve(col) else {
            return;
        };
        if label.is_empty() {
            return;
        }
        let face = self.par.font_face(font);
        let size_pt = self.par.ps * self.par.cex * cex;
        let w = afm::str_width_inches(label, face, size_pt, 1.0);
        let (asc, desc) = afm::ascent_descent_inches(face, size_pt, 1.0);
        let origin = resolve_text_origin(anchor, w, asc, desc, hadj, vadj, rotation_deg);
        let style = TextStyle {
            rotation_deg,
            ..TextStyle::new(color, face, size_pt)
        };
        self.push_clip_for(self.par.xpd);
        self.backend.draw_text(origin, label, &style);
        self.pop_clip();
    }

    // ---------------------------------------------------------------
    // box() / axis() / title()
    // ---------------------------------------------------------------

    /// `box(which = "plot", col=, lwd=, lty=)`.
    pub fn box_plot(&mut self, col: ColorSpec, lwd: f64, lty: LineType) {
        let Some(color) = self.resolve(col) else {
            return;
        };
        let stroke = self.stroke_of(color, lwd, lty);
        self.backend.draw_rect(self.plt, None, Some(&stroke));
    }

    /// `axis(side, at=, labels=, ...)`. `at`/`labels` default to R's
    /// automatic tick placement (`pretty()` on the current axis range)
    /// and `format!("{}", v)`-style numeric labels.
    #[allow(clippy::too_many_arguments)]
    pub fn axis(
        &mut self,
        side: Side,
        at: Option<Vec<f64>>,
        labels: Option<Vec<String>>,
        col: ColorSpec,
        cex_axis: f64,
        font_axis: i32,
        las: i32,
    ) {
        let Some(color) = self.resolve(col) else {
            return;
        };
        // `axis()` always draws in the margins, outside the plot region,
        // so it ignores the user's `par(xpd)` and clips to the figure
        // region instead (matching R, which hardcodes this).
        let saved_xpd = self.par.xpd;
        self.par.xpd = true;
        let ticks = at.unwrap_or_else(|| self.default_ticks(side));
        let labels =
            labels.unwrap_or_else(|| ticks.iter().map(|v| format_axis_label(*v)).collect());
        let stroke = self.stroke_of(color, self.par.lwd, LineType::Pattern(&[]));
        let lh = self.line_height();
        let tick_len = self.par.tcl.abs() * lh; // R's `tcl` is negative-by-default = outward
        let label_offset = self.par.mgp[1] * lh;
        let size_pt = self.par.ps * self.par.cex * cex_axis;

        // Axis baseline itself.
        let axis_line = match side {
            1 => (
                Point::new(self.plt.x0, self.plt.y0),
                Point::new(self.plt.x1, self.plt.y0),
            ),
            3 => (
                Point::new(self.plt.x0, self.plt.y1),
                Point::new(self.plt.x1, self.plt.y1),
            ),
            2 => (
                Point::new(self.plt.x0, self.plt.y0),
                Point::new(self.plt.x0, self.plt.y1),
            ),
            _ => (
                Point::new(self.plt.x1, self.plt.y0),
                Point::new(self.plt.x1, self.plt.y1),
            ),
        };
        self.backend.draw_line(axis_line.0, axis_line.1, &stroke);

        for (v, label) in ticks.iter().zip(labels.iter()) {
            let along = if side == 1 || side == 3 {
                let uv = if self.xlog { 10f64.powf(*v) } else { *v };
                if uv < self.range_min(true) - 1e-9 || uv > self.range_max(true) + 1e-9 {
                    continue;
                }
                self.user_to_inches(uv, self.range_min(false)).x
            } else {
                let uv = if self.ylog { 10f64.powf(*v) } else { *v };
                if uv < self.range_min(false) - 1e-9 || uv > self.range_max(false) + 1e-9 {
                    continue;
                }
                self.user_to_inches(self.range_min(true), uv).y
            };

            let (tick_p0, tick_p1, label_anchor, hadj, vadj);
            match side {
                1 => {
                    tick_p0 = Point::new(along, self.plt.y0);
                    tick_p1 = Point::new(along, self.plt.y0 - tick_len);
                    label_anchor = Point::new(along, self.plt.y0 - label_offset);
                    hadj = 0.5;
                    vadj = 1.0;
                }
                3 => {
                    tick_p0 = Point::new(along, self.plt.y1);
                    tick_p1 = Point::new(along, self.plt.y1 + tick_len);
                    label_anchor = Point::new(along, self.plt.y1 + label_offset);
                    hadj = 0.5;
                    vadj = 0.0;
                }
                2 => {
                    tick_p0 = Point::new(self.plt.x0, along);
                    tick_p1 = Point::new(self.plt.x0 - tick_len, along);
                    label_anchor = Point::new(self.plt.x0 - label_offset, along);
                    hadj = 1.0;
                    vadj = 0.5;
                }
                _ => {
                    tick_p0 = Point::new(self.plt.x1, along);
                    tick_p1 = Point::new(self.plt.x1 + tick_len, along);
                    label_anchor = Point::new(self.plt.x1 + label_offset, along);
                    hadj = 0.0;
                    vadj = 0.5;
                }
            }
            self.backend.draw_line(tick_p0, tick_p1, &stroke);
            let rotation = if las == 2 || las == 3 { 90.0 } else { 0.0 };
            let (hadj, vadj) = if rotation != 0.0 {
                (vadj, hadj)
            } else {
                (hadj, vadj)
            };
            self.text_at(
                label_anchor,
                label,
                hadj,
                vadj,
                rotation,
                ColorSpec::Color(color),
                size_pt / self.par.ps / self.par.cex,
                font_axis,
            );
        }
        self.par.xpd = saved_xpd;
    }

    fn range_min(&self, is_x: bool) -> f64 {
        if is_x {
            let (x0, x1, ..) = self.usr;
            let lo = x0.min(x1);
            if self.xlog {
                10f64.powf(lo)
            } else {
                lo
            }
        } else {
            let (_, _, y0, y1) = self.usr;
            let lo = y0.min(y1);
            if self.ylog {
                10f64.powf(lo)
            } else {
                lo
            }
        }
    }

    fn range_max(&self, is_x: bool) -> f64 {
        if is_x {
            let (x0, x1, ..) = self.usr;
            let hi = x0.max(x1);
            if self.xlog {
                10f64.powf(hi)
            } else {
                hi
            }
        } else {
            let (_, _, y0, y1) = self.usr;
            let hi = y0.max(y1);
            if self.ylog {
                10f64.powf(hi)
            } else {
                hi
            }
        }
    }

    /// `title(main=, sub=, xlab=, ylab=)`.
    pub fn title(
        &mut self,
        main: Option<&str>,
        sub: Option<&str>,
        xlab: Option<&str>,
        ylab: Option<&str>,
    ) {
        let saved_xpd = self.par.xpd;
        self.par.xpd = true;
        let lh = self.line_height();
        if let Some(xlab) = xlab {
            let y = self.plt.y0 - self.par.mgp[0] * lh;
            let x = (self.plt.x0 + self.plt.x1) / 2.0;
            self.text_at(
                Point::new(x, y),
                xlab,
                0.5,
                1.0,
                0.0,
                ColorSpec::Color(self.par.col_lab),
                self.par.cex_lab,
                self.par.font_lab,
            );
        }
        if let Some(ylab) = ylab {
            let x = self.plt.x0 - self.par.mgp[0] * lh;
            let y = (self.plt.y0 + self.plt.y1) / 2.0;
            self.text_at(
                Point::new(x, y),
                ylab,
                0.5,
                1.0,
                90.0,
                ColorSpec::Color(self.par.col_lab),
                self.par.cex_lab,
                self.par.font_lab,
            );
        }
        if let Some(main) = main {
            let y = self.plt.y1 + (self.par.mar.top - 1.0).max(0.2) * lh;
            let x = (self.plt.x0 + self.plt.x1) / 2.0;
            self.text_at(
                Point::new(x, y),
                main,
                0.5,
                0.0,
                0.0,
                ColorSpec::Color(self.par.col_main),
                self.par.cex_main,
                self.par.font_main,
            );
        }
        if let Some(sub) = sub {
            let y = self.plt.y0 - (self.par.mgp[0] + 1.5) * lh;
            let x = (self.plt.x0 + self.plt.x1) / 2.0;
            self.text_at(
                Point::new(x, y),
                sub,
                0.5,
                1.0,
                0.0,
                ColorSpec::Color(self.par.col_sub),
                self.par.cex_sub,
                self.par.font_sub,
            );
        }
        self.par.xpd = saved_xpd;
    }

    /// `arrows(x0, y0, x1, y1, length = 0.25, ...)`: a segment plus a
    /// simple filled-triangle arrowhead at the end point(s), `length` in
    /// inches (matching R's default unit for the `length` argument).
    pub fn arrows(
        &mut self,
        x0: f64,
        y0: f64,
        x1: f64,
        y1: f64,
        style: LineStyle,
        head_length_in: f64,
    ) {
        let Some(color) = self.resolve(style.col) else {
            return;
        };
        let stroke = self.stroke_of(color, style.lwd, style.lty);
        self.push_clip_for(self.par.xpd);
        let p0 = self.user_to_inches(x0, y0);
        let p1 = self.user_to_inches(x1, y1);
        self.backend.draw_line(p0, p1, &stroke);
        let angle = (p1.y - p0.y).atan2(p1.x - p0.x);
        let spread = 20f64.to_radians();
        let back = |a: f64| {
            Point::new(
                p1.x - head_length_in * (angle + a).cos(),
                p1.y - head_length_in * (angle + a).sin(),
            )
        };
        let tri = [p1, back(spread), back(-spread)];
        self.backend.draw_polygon(&tri, Some(color), None);
        self.pop_clip();
    }

    /// A simple `legend(x, y, legend=, col=, pch=, lty=, ...)`, anchored
    /// at a user-coordinate point that becomes the legend box's top-left
    /// corner (matching `legend("topleft", ...)`-style usage; R's
    /// keyword positions like `"topright"` aren't implemented, callers
    /// resolve those to a user coordinate themselves).
    pub fn legend(&mut self, x: f64, y: f64, entries: &[LegendEntry]) {
        if entries.is_empty() {
            return;
        }
        let lh = self.line_height();
        let size_pt = self.par.ps * self.par.cex;
        let face = self.par.font_face(self.par.font);
        let text_h = afm::str_height_inches(face, size_pt, 1.0);
        let line_gap = text_h * 1.2;
        let symbol_col_w = 2.0 * lh;
        let pad = 0.3 * lh;

        let max_label_w = entries
            .iter()
            .map(|e| afm::str_width_inches(&e.label, face, size_pt, 1.0))
            .fold(0.0_f64, f64::max);
        let box_w = pad * 2.0 + symbol_col_w + max_label_w;
        let box_h = pad * 2.0 + entries.len() as f64 * line_gap;

        let top_left = self.user_to_inches(x, y);
        let rect = Rect::new(
            top_left.x,
            top_left.y - box_h,
            top_left.x + box_w,
            top_left.y,
        );
        let border_stroke = Stroke::simple(self.par.fg, 1.0);
        self.backend
            .draw_rect(rect, Some(self.par.bg), Some(&border_stroke));

        for (i, entry) in entries.iter().enumerate() {
            let row_top = top_left.y - pad - i as f64 * line_gap;
            let row_mid_y = row_top - line_gap / 2.0;
            let symbol_x_mid = top_left.x + pad + symbol_col_w / 2.0;
            if let Some(col) = self.resolve(entry.col) {
                if let Some(pch) = entry.pch {
                    let size = pch::symbol_size_inches(self.par.ps, self.par.cex);
                    self.draw_pch(pch, Point::new(symbol_x_mid, row_mid_y), size, col, None);
                } else {
                    let stroke = self.stroke_of(col, entry.lwd, entry.lty);
                    let half = symbol_col_w / 2.0 - pad * 0.3;
                    self.backend.draw_line(
                        Point::new(symbol_x_mid - half, row_mid_y),
                        Point::new(symbol_x_mid + half, row_mid_y),
                        &stroke,
                    );
                }
            }
            let label_x = top_left.x + pad + symbol_col_w;
            self.text_at(
                Point::new(label_x, row_mid_y),
                &entry.label,
                0.0,
                0.5,
                0.0,
                ColorSpec::Color(self.par.col),
                1.0,
                self.par.font,
            );
        }
    }

    pub fn backend_mut(&mut self) -> &mut B {
        &mut self.backend
    }
}

/// One row of a [`Device::legend`] call.
#[derive(Clone, Debug)]
pub struct LegendEntry {
    pub label: String,
    pub col: ColorSpec,
    pub pch: Option<i32>,
    pub lwd: f64,
    pub lty: LineType,
}

impl LegendEntry {
    pub fn point(label: impl Into<String>, col: ColorSpec, pch: i32) -> Self {
        LegendEntry {
            label: label.into(),
            col,
            pch: Some(pch),
            lwd: 1.0,
            lty: LineType::Pattern(&[]),
        }
    }

    pub fn line(label: impl Into<String>, col: ColorSpec, lwd: f64, lty: LineType) -> Self {
        LegendEntry {
            label: label.into(),
            col,
            pch: None,
            lwd,
            lty,
        }
    }
}

/// Resolve a text anchor + `adj`/`srt` into the baseline-left origin a
/// [`Backend::draw_text`] expects. See the module docs in `backend/mod.rs`
/// for why this lives outside the `Backend` trait.
fn resolve_text_origin(
    anchor: Point,
    width: f64,
    ascent: f64,
    descent: f64,
    hadj: f64,
    vadj: f64,
    rotation_deg: f64,
) -> Point {
    let dx = -hadj * width;
    let dy = descent - vadj * (ascent + descent);
    let theta = rotation_deg.to_radians();
    let (c, s) = (theta.cos(), theta.sin());
    Point::new(anchor.x + dx * c - dy * s, anchor.y + dx * s + dy * c)
}

fn cex_factor(size_inches: f64) -> f64 {
    size_inches * 72.0
}

/// Default numeric axis label formatting: R's `axis()` uses roughly
/// `format(at, trim = TRUE)`, which drops unnecessary trailing zeros.
fn format_axis_label(v: f64) -> String {
    if v == v.trunc() && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        let s = format!("{v:.10}");
        let s = s.trim_end_matches('0').trim_end_matches('.');
        s.to_string()
    }
}
