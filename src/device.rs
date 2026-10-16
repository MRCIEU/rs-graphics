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
use crate::layout::{Arrangement, Layout};
use crate::lty::LineType;
use crate::par::{AxisStyle, Par};
use crate::pch;
use crate::pretty::{log_axis_ticks, pretty_ticks};

mod abline;
mod axis;
mod legend;
mod mtext;
pub use abline::AblineOptions;
pub use axis::AxisOptions;
pub use legend::{LegendEntry, LegendKeyword, LegendLayout, LegendOptions, LegendPos};
pub use mtext::MtextOptions;

/// Which axis/box side, in R's `side` convention: `1` = bottom, `2` =
/// left, `3` = top, `4` = right.
pub type Side = i32;

/// `dev->yLineBias` for R's pdf/png/svg devices (see `GMtext()`).
const Y_LINE_BIAS: f64 = 0.2;

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
    /// How figures are arranged (`mfrow`/`mfcol`/`layout()`); the default
    /// is a single figure.
    arrangement: Arrangement,
    /// `par("mfg")`'s `currentFigure`, 1-based.
    current_figure: usize,
    /// `par("new")`, set by [`Device::set_new`] and [`Device::mfg`]: the
    /// next `plot_new()` reuses the current figure instead of advancing.
    /// Cleared by drawing (see [`Device::dirty`]), not by `plot_new()`.
    hold_figure: bool,
    /// Whether anything has been plotted yet (R's `gpptr(dd)->state`).
    used: bool,
}

/// Where drawing is clipped to: R's `xpd = FALSE` (plot region), `TRUE`
/// (figure region) and `NA` (the whole device).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Clip {
    Plot,
    Figure,
    Device,
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
    /// A device drawing onto `backend` with graphical parameters `par`. If
    /// `par.bg` is left at its default (white), it starts as the backend's
    /// background instead, as R's `par("bg")` does - transparent for PDF.
    pub fn new(backend: B, mut par: Par) -> Self {
        if par.bg == Par::default().bg {
            par.bg = backend.default_bg();
        }
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
            arrangement: Arrangement::Grid {
                nrow: 1,
                ncol: 1,
                by_col: false,
            },
            current_figure: 1,
            hold_figure: false,
            used: false,
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
        // Inner region: the device minus the outer margins; figures are
        // fractions (NIC) of it.
        let omi = self.par.omi();
        let inner = Rect::new(
            omi.left,
            omi.bottom,
            self.width_in - omi.right,
            self.height_in - omi.top,
        );
        let [fx0, fx1, fy0, fy1] =
            self.arrangement
                .figure_region_nic(self.current_figure, inner.width(), inner.height());
        self.fig = Rect::new(
            inner.x0 + fx0 * inner.width(),
            inner.y0 + fy0 * inner.height(),
            inner.x0 + fx1 * inner.width(),
            inner.y0 + fy1 * inner.height(),
        );
        let mai = self.par.mai();
        self.plt = Rect::new(
            self.fig.x0 + mai.left,
            self.fig.y0 + mai.bottom,
            self.fig.x1 - mai.right,
            self.fig.y1 - mai.top,
        );
    }

    /// `par(mfrow = c(nrow, ncol))`: a grid of figures filled row by row.
    /// Like R, also sets `par(cex)` to 0.83 (2 x 2) or 0.66 (more than 2
    /// rows or columns) and `mex` to 1. The next `plot_new()` starts a new
    /// page at the first figure.
    ///
    /// # Panics
    /// If `nrow` or `ncol` is zero.
    pub fn mfrow(&mut self, nrow: usize, ncol: usize) {
        self.set_arrangement(Arrangement::Grid {
            nrow,
            ncol,
            by_col: false,
        });
    }

    /// `par(mfcol = c(nrow, ncol))`: as [`Device::mfrow`] but filled
    /// column by column.
    pub fn mfcol(&mut self, nrow: usize, ncol: usize) {
        self.set_arrangement(Arrangement::Grid {
            nrow,
            ncol,
            by_col: true,
        });
    }

    /// `layout(mat, widths, heights, respect)`: see [`Layout`]. Sets
    /// `par(cex)`/`mex` like [`Device::mfrow`] does, based on the layout
    /// matrix's dimensions.
    pub fn layout(&mut self, layout: Layout) {
        self.set_arrangement(Arrangement::Layout(layout));
    }

    fn set_arrangement(&mut self, arrangement: Arrangement) {
        assert!(
            arrangement.nrow() > 0 && arrangement.ncol() > 0,
            "a figure grid needs at least one row and one column"
        );
        self.par.cex = arrangement.cex_base();
        self.par.mex = 1.0;
        // R sets `currentFigure = lastFigure` so the next plot.new()
        // wraps round to figure 1 on a fresh page.
        self.current_figure = arrangement.last_figure();
        self.arrangement = arrangement;
        self.hold_figure = false;
        self.recompute_layout();
    }

    /// `par(mfg = c(row, col))`: makes the figure at (`row`, `col`)
    /// (1-based) the one the next `plot_new()` draws into, without
    /// advancing or starting a new page. As in R, the figure number is
    /// derived from the `mfrow`/`mfcol` ordering even under `layout()`.
    ///
    /// # Panics
    /// If `row`/`col` are outside the current arrangement.
    pub fn mfg(&mut self, row: usize, col: usize) {
        let (nrow, ncol) = (self.arrangement.nrow(), self.arrangement.ncol());
        assert!(
            (1..=nrow).contains(&row) && (1..=ncol).contains(&col),
            "mfg out of range"
        );
        let by_col = matches!(self.arrangement, Arrangement::Grid { by_col: true, .. });
        self.current_figure = if by_col {
            (col - 1) * nrow + row
        } else {
            (row - 1) * ncol + col
        };
        self.hold_figure = true;
        self.recompute_layout();
    }

    /// `par(new = new)`: with `true`, the next `plot_new()` draws into the
    /// current figure without advancing or clearing it, so a second plot
    /// can be overlaid on the first (e.g. with its own `plot_window()` and
    /// an axis on side 4). As in R, drawing anything (other than
    /// `mtext(outer = TRUE)`) resets it to `false`, `plot_new()` itself
    /// does not, and it is ignored (R warns) before anything is plotted.
    pub fn set_new(&mut self, new: bool) {
        if self.used {
            self.hold_figure = new;
        }
    }

    /// `par("new")`.
    pub fn is_new(&self) -> bool {
        self.hold_figure
    }

    /// The current figure number (`par("mfg")`'s position, 1-based).
    pub fn current_figure(&self) -> usize {
        self.current_figure
    }

    /// The current figure region in device inches.
    pub fn figure_region(&self) -> Rect {
        self.fig
    }

    /// `plot.new()`: advances to the next figure (starting a new "page"
    /// after the last one) and recomputes the figure/plot regions from the
    /// current margins. A new page clears the device to `par("bg")`.
    ///
    /// The backends hold a single page, so "a new page" is emulated by
    /// painting the background over whatever was drawn before.
    pub fn plot_new(&mut self) {
        let mut new_page = false;
        if !self.hold_figure || !self.used {
            self.current_figure += 1;
            if self.current_figure > self.arrangement.last_figure() || !self.used {
                self.current_figure = 1;
                new_page = true;
            }
        }
        self.used = true;
        self.recompute_layout();
        if new_page {
            self.backend.clear_page();
            if self.par.bg.a > 0 {
                self.backend.draw_rect(
                    Rect::new(0.0, 0.0, self.width_in, self.height_in),
                    Some(self.par.bg),
                    None,
                );
            }
        }
    }

    /// `plot.window(xlim, ylim, log)`: sets up the user-coordinate system.
    /// Applies R's default `xaxs = yaxs = "r"` 4%-range-expansion (see
    /// `GScale()` in `graphics.c`) unless the axis style is `"i"`.
    pub fn plot_window(&mut self, xlim: (f64, f64), ylim: (f64, f64), xlog: bool, ylog: bool) {
        self.xlog = xlog;
        self.ylog = ylog;
        let (ux0, ux1) = Self::gscale(xlim.0, xlim.1, self.par.xaxs, xlog);
        let (uy0, uy1) = Self::gscale(ylim.0, ylim.1, self.par.yaxs, ylog);
        self.usr = (ux0, ux1, uy0, uy1);
    }

    /// `plot.window(xlim, ylim, asp = asp)` on linear axes: widens
    /// whichever range is needed so one x unit is `asp` y units long on
    /// the page (centred on the given limits), *then* applies
    /// `par(xaxs/yaxs)` as usual. Port of `C_plot_window`'s `asp` branch.
    pub fn plot_window_asp(&mut self, xlim: (f64, f64), ylim: (f64, f64), asp: f64) {
        if !(asp.is_finite() && asp > 0.0) {
            self.plot_window(xlim, ylim, false, false);
            return;
        }
        let (pin1, pin2) = (self.plt.width(), self.plt.height());
        let xdelta = (xlim.1 - xlim.0).abs() / asp;
        let ydelta = (ylim.1 - ylim.0).abs();
        let (mut xadd, mut yadd) = if xdelta == 0.0 && ydelta == 0.0 {
            let a = if xlim.0 == 0.0 { 1.0 } else { 0.4 } * asp;
            (a * asp, a)
        } else {
            let scale = (pin1 / xdelta).min(pin2 / ydelta);
            (
                0.5 * (pin1 / scale - xdelta) * asp,
                0.5 * (pin2 / scale - ydelta),
            )
        };
        if xlim.1 < xlim.0 {
            xadd = -xadd;
        }
        if ylim.1 < ylim.0 {
            yadd = -yadd;
        }
        self.plot_window(
            (xlim.0 - xadd, xlim.1 + xadd),
            (ylim.0 - yadd, ylim.1 + yadd),
            false,
            false,
        );
    }

    /// `GScale()` (`graphics.c`): the `usr` range (log10 units on a log
    /// axis) for data limits `min..max` (which may be decreasing). Non-finite
    /// limits are replaced by huge ones (on a log axis, limits <= 0 give
    /// about 1e-307, as R does after warning "nonfinite axis limits"), a
    /// zero-width range is widened (to -1..1 around 0, else by 40% of the
    /// value), and `xaxs = "r"` adds 4% at each end.
    fn gscale(min: f64, max: f64, style: AxisStyle, log: bool) -> (f64, f64) {
        const EPS_FAC_1: f64 = 16.0;
        let (mut min, mut max) = if log { (min.log10(), max.log10()) } else { (min, max) };
        let fix = |v: f64| {
            if v.is_finite() {
                v
            } else if log {
                if v < 0.0 { -320.0 } else { 308.254715559 }
            } else {
                0.45 * if v < 0.0 { -f64::MAX } else { f64::MAX }
            }
        };
        min = fix(min);
        max = fix(max);
        let mut temp = min.abs().max(max.abs());
        if temp == 0.0 {
            min = -1.0;
            max = 1.0;
        } else {
            let mut tf = if temp > 1.0 {
                (temp * f64::EPSILON) * EPS_FAC_1
            } else {
                (temp * EPS_FAC_1) * f64::EPSILON
            };
            if tf == 0.0 {
                tf = f64::MIN_POSITIVE;
            }
            if (max - min).abs() < tf {
                temp *= if min == max { 0.4 } else { 1e-2 };
                min -= temp;
                max += temp;
            }
        }
        if style == AxisStyle::Regular {
            let d = if temp > 100.0 {
                0.04 * max - 0.04 * min
            } else {
                0.04 * (max - min)
            };
            let clamp = |v: f64| {
                if v.is_finite() { v } else if v < 0.0 { -f64::MAX } else { f64::MAX }
            };
            if log {
                min -= d;
                max += d;
            } else {
                min = clamp(min - d);
                max = clamp(max + d);
            }
        }
        if log {
            // Keep 10^min above 0 and 10^max finite.
            if 10f64.powf(min) == 0.0 {
                min = (1.01 * f64::MIN_POSITIVE).log10();
            }
            if max >= 308.25035 {
                max = (0.99 * f64::MAX).log10();
            }
        }
        (min, max)
    }

    /// Default tick locations R's `axis()` would pick for `side` when no
    /// `at=` is given, in data units: `pretty()` on the current `usr` range
    /// for that axis, aiming for `par("lab")[1]` (x) or `[2]` (y)
    /// intervals, or R's log-axis rules (see [`log_axis_ticks`]) on a log
    /// axis.
    pub fn default_ticks(&self, side: Side) -> Vec<f64> {
        let (lo, hi, log, n) = if side == 1 || side == 3 {
            (self.usr.0, self.usr.1, self.xlog, self.par.lab[0])
        } else {
            (self.usr.2, self.usr.3, self.ylog, self.par.lab[1])
        };
        if log {
            log_axis_ticks(lo.min(hi), lo.max(hi), n)
        } else {
            pretty_ticks(lo.min(hi), lo.max(hi), n)
        }
    }

    // ---------------------------------------------------------------
    // Coordinate transforms
    // ---------------------------------------------------------------

    /// Map a user-coordinate point to device inches.
    pub fn user_to_inches(&self, x: f64, y: f64) -> Point {
        let ux = if self.xlog { x.log10() } else { x };
        let uy = if self.ylog { y.log10() } else { y };
        self.usr_to_inches(ux, uy)
    }

    /// Map a point in `usr` units (log10 on a log axis) to device inches.
    fn usr_to_inches(&self, ux: f64, uy: f64) -> Point {
        let (x0, x1, y0, y1) = self.usr;
        let px = self.plt.x0 + (ux - x0) / (x1 - x0) * self.plt.width();
        let py = self.plt.y0 + (uy - y0) / (y1 - y0) * self.plt.height();
        Point::new(px, py)
    }

    /// R's `getxlimits()`/`getylimits()` in `usr` units: the `usr` range
    /// with `par(xpd = FALSE)`, else the figure region's extent along the
    /// axis.
    fn xpd_limits(&self, x_axis: bool) -> (f64, f64) {
        let (u0, u1, p0, p1, f0, f1) = if x_axis {
            let (u0, u1, ..) = self.usr;
            (u0, u1, self.plt.x0, self.plt.x1, self.fig.x0, self.fig.x1)
        } else {
            let (.., u0, u1) = self.usr;
            (u0, u1, self.plt.y0, self.plt.y1, self.fig.y0, self.fig.y1)
        };
        if self.par.xpd {
            let to_usr = |d: f64| u0 + (d - p0) / (p1 - p0) * (u1 - u0);
            (to_usr(f0), to_usr(f1))
        } else {
            (u0, u1)
        }
    }

    fn line_height(&self) -> f64 {
        self.par.line_height_inches()
    }

    /// Font size in points for `par(ps) * par(cex) * cex`, after any
    /// rounding the backend's real-world counterpart applies (R's `pdf()`
    /// rounds to whole points; `svg()`/`png()` don't).
    fn font_size_pt(&self, cex: f64) -> f64 {
        self.backend
            .effective_font_size(self.par.ps * self.par.cex * cex)
    }

    // ---------------------------------------------------------------
    // Clipping (matches R's `xpd`: FALSE clips to the plot region, TRUE
    // clips to the figure region - we treat R's `NA`, "clip to device",
    // the same as figure-region clipping, since we don't yet support
    // outer margins wide enough for that distinction to usually matter).
    // ---------------------------------------------------------------

    fn push_clip_for(&mut self, xpd: bool) {
        self.push_clip_region(if xpd { Clip::Figure } else { Clip::Plot });
    }

    /// Marks the current plot as drawn on, which resets `par(new)` (R's
    /// `GMode()`). Every drawing path goes through here, mostly via
    /// [`Device::push_clip_region`].
    fn dirty(&mut self) {
        self.hold_figure = false;
    }

    fn push_clip_region(&mut self, clip: Clip) {
        self.dirty();
        let rect = match clip {
            Clip::Plot => self.plt,
            Clip::Figure => self.fig,
            Clip::Device => Rect::new(0.0, 0.0, self.width_in, self.height_in),
        };
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
        self.push_clip_for(self.par.xpd);
        for (&x, &y) in xs.iter().zip(ys) {
            // Checked after conversion, like R, so values <= 0 on a log
            // axis are skipped too.
            let p = self.user_to_inches(x, y);
            if !p.x.is_finite() || !p.y.is_finite() {
                continue;
            }
            self.draw_pch(pch, p, cex, col, bg);
        }
        self.pop_clip();
    }

    /// One symbol, as `GSymbol()`/`GESymbol()` draw it. `cex` is the
    /// per-point `cex` (R multiplies it by `cexbase`, i.e. `par(cex)`).
    fn draw_pch(&mut self, pch: i32, p: Point, cex: f64, col: Color, bg: Option<Color>) {
        let cex = self.par.cex * cex;
        let size = pch::symbol_size_inches(self.par.ps, cex);
        if let Some(geom) = pch::pch_geometry(pch, p.x, p.y, size) {
            let stroke = Stroke::simple(col, self.par.lwd);
            for shape in &geom.outline_only {
                self.paint_shape(shape, None, Some(&stroke));
            }
            for shape in &geom.filled_with_col {
                self.paint_shape(shape, Some(col), None);
            }
            for shape in &geom.filled_and_stroked_with_col {
                self.paint_shape(shape, Some(col), Some(&stroke));
            }
            for shape in &geom.filled_with_bg {
                self.paint_shape(shape, bg, Some(&stroke));
            }
            for &(a, b) in &geom.lines {
                self.backend
                    .draw_line(Point::new(a.0, a.1), Point::new(b.0, b.1), &stroke);
            }
        } else if pch == '.' as i32 {
            // A filled 0.01in square scaled by `cex`, but at least one
            // device unit across.
            if cex <= 0.0 {
                return;
            }
            let xc = (cex * 0.005).max(0.5 * self.backend.device_unit_inches());
            self.backend.draw_rect(
                Rect::new(p.x - xc, p.y - xc, p.x + xc, p.y + xc),
                Some(col),
                None,
            );
        } else if (32..=126).contains(&pch) {
            // `GEText(x, y, str, hadj = NA, yc = NA)`: centred on the
            // advance width, and vertically on the glyph's own bounding box.
            let ch = char::from_u32(pch as u32).unwrap_or('?').to_string();
            let face = self.par.font_face(self.par.font);
            let size_pt = self.backend.effective_font_size(self.par.ps * cex);
            if size_pt <= 0.0 {
                return;
            }
            let style = TextStyle::new(col, face, size_pt);
            let w = afm::str_width_inches(&ch, face, size_pt, 1.0);
            let (asc, desc) = afm::glyph_ascent_descent_inches(&ch, face, size_pt, 1.0);
            let origin = Point::new(p.x - 0.5 * w, p.y - 0.5 * (asc - desc));
            self.backend.draw_text(origin, &ch, &style);
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
    /// separate segments at non-finite (`NA`) coordinates, and at values
    /// <= 0 on a log axis, matching R.
    pub fn lines(&mut self, xs: &[f64], ys: &[f64], style: LineStyle) {
        let Some(col) = self.resolve(style.col) else {
            return;
        };
        let stroke = self.stroke_of(col, style.lwd, style.lty);
        self.push_clip_for(self.par.xpd);
        let mut run: Vec<Point> = Vec::new();
        for (&x, &y) in xs.iter().zip(ys) {
            let p = self.user_to_inches(x, y);
            if p.x.is_finite() && p.y.is_finite() {
                run.push(p);
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

    /// The lines of `plot(type = "b")` (and `"c"`): each segment between
    /// consecutive points, shortened at both ends by half a character
    /// height (`0.5 * par(cex) * 1.2 * ps / 72` inches) so the points stand
    /// clear, and left out when shorter than that. A port of `C_plotXY`'s
    /// `case 'b'`.
    pub fn broken_lines(&mut self, xs: &[f64], ys: &[f64], style: LineStyle) {
        let Some(col) = self.resolve(style.col) else {
            return;
        };
        let stroke = self.stroke_of(col, style.lwd, style.lty);
        let d = 0.5 * self.par.cex * 1.2 * self.par.ps / 72.0;
        self.push_clip_for(self.par.xpd);
        let mut old: Option<Point> = None;
        for (&x, &y) in xs.iter().zip(ys) {
            let p = self.user_to_inches(x, y);
            let p = (p.x.is_finite() && p.y.is_finite()).then_some(p);
            if let (Some(a), Some(b)) = (old, p) {
                let len = (b.x - a.x).hypot(b.y - a.y);
                if d < 0.5 * len {
                    let f = d / len;
                    let from = Point::new(a.x + f * (b.x - a.x), a.y + f * (b.y - a.y));
                    let to = Point::new(b.x + f * (a.x - b.x), b.y + f * (a.y - b.y));
                    self.backend.draw_line(from, to, &stroke);
                }
            }
            old = p;
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
        // R skips rectangles with a non-finite corner, e.g. at 0 on a log
        // axis.
        if [p0.x, p0.y, p1.x, p1.y].iter().all(|v| v.is_finite()) {
            self.backend
                .draw_rect(Rect::new(p0.x, p0.y, p1.x, p1.y), fill_c, stroke.as_ref());
        }
        self.pop_clip();
    }

    /// `rect()` variant taking a rectangle already in device inches
    /// (used internally for legends and margin decorations).
    pub fn rect_inches(&mut self, rect: Rect, fill: Option<Color>, stroke: Option<&Stroke>) {
        self.dirty();
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

    /// `abline(a, b)` / `abline(h = )` / `abline(v = )` with one colour,
    /// width and line type for all. See [`Device::abline_with`] (R's
    /// `C_abline`) for vectors of `h`/`v`, recycled styles and how lines
    /// behave on log axes.
    #[allow(clippy::too_many_arguments)]
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
        self.abline_with(&AblineOptions {
            coef: a.zip(b),
            h: h.into_iter().collect(),
            v: v.into_iter().collect(),
            col: vec![col],
            lty: vec![lty],
            lwd: vec![lwd],
            ..AblineOptions::default()
        });
    }

    /// `abline(a, b, untf = TRUE)`: the line `y = a + b * x` in data units,
    /// curved on a log axis. Without log axes, the same as
    /// [`Device::abline`].
    pub fn abline_untf(&mut self, a: f64, b: f64, col: ColorSpec, lwd: f64, lty: LineType) {
        self.abline_with(&AblineOptions {
            coef: Some((a, b)),
            untf: true,
            col: vec![col],
            lty: vec![lty],
            lwd: vec![lwd],
            ..AblineOptions::default()
        });
    }

    fn draw_line_if_finite(&mut self, from: Point, to: Point, stroke: &Stroke) {
        if [from.x, from.y, to.x, to.y].iter().all(|v| v.is_finite()) {
            self.backend.draw_line(from, to, stroke);
        }
    }

    // ---------------------------------------------------------------
    // Text
    // ---------------------------------------------------------------

    /// Width/height (inches) `strwidth()`/`strheight()` would report for
    /// `text` under the current `par(font=, ps=)` (times `cex`).
    pub fn text_metrics(&self, text: &str, font: i32, cex: f64) -> (f64, f64) {
        let face = self.par.font_face(font);
        let size = self.backend.effective_font_size(self.par.ps * cex);
        (
            afm::str_width_inches(text, face, size, 1.0),
            afm::str_height_inches(face, size, 1.0),
        )
    }

    /// `text(x, y, labels, adj=, srt=, col=, cex=, font=)` in user
    /// coordinates. `vadj = None` is R's `NA` vertical adjustment, which
    /// `text()` uses unless `adj` has two elements (so it is the default).
    #[allow(clippy::too_many_arguments)]
    pub fn text(
        &mut self,
        x: f64,
        y: f64,
        label: &str,
        hadj: f64,
        vadj: Option<f64>,
        rotation_deg: f64,
        col: ColorSpec,
        cex: f64,
        font: i32,
    ) {
        let p = self.user_to_inches(x, y);
        self.text_at(p, label, hadj, vadj, rotation_deg, col, cex, font);
    }

    /// As [`Device::text`] but the anchor is already in device inches.
    /// Placement follows `GEText()`: horizontally `hadj` of the string's
    /// width; vertically, a finite `vadj` puts the baseline `vadj *
    /// height("M")` below the anchor, while `None` (`NA`) centres the ink
    /// of the string's glyphs on it (baseline at `anchor - (ury + lly) / 2`
    /// of the glyph bounding boxes).
    #[allow(clippy::too_many_arguments)]
    pub fn text_at(
        &mut self,
        anchor: Point,
        label: &str,
        hadj: f64,
        vadj: Option<f64>,
        rotation_deg: f64,
        col: ColorSpec,
        cex: f64,
        font: i32,
    ) {
        let clip = if self.par.xpd { Clip::Figure } else { Clip::Plot };
        self.text_at_clipped(clip, anchor, label, hadj, vadj, rotation_deg, col, cex, font);
    }

    /// [`Device::text_at`] clipped to `clip` instead of the `par(xpd)` region.
    #[allow(clippy::too_many_arguments)]
    fn text_at_clipped(
        &mut self,
        clip: Clip,
        anchor: Point,
        label: &str,
        hadj: f64,
        vadj: Option<f64>,
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
        let size_pt = self.font_size_pt(cex);
        if size_pt <= 0.0 {
            return;
        }
        let width = afm::str_width_inches(label, face, size_pt, 1.0);
        let dy = match vadj {
            Some(yc) => -yc * afm::m_height_inches(face, size_pt, 1.0),
            None => {
                let (asc, desc) = afm::glyph_ascent_descent_inches(label, face, size_pt, 1.0);
                -0.5 * (asc - desc)
            }
        };
        let dx = -hadj * width;
        let theta = rotation_deg.to_radians();
        let (c, s) = (theta.cos(), theta.sin());
        let origin = Point::new(anchor.x + dx * c - dy * s, anchor.y + dx * s + dy * c);
        let style = TextStyle {
            rotation_deg,
            ..TextStyle::new(color, face, size_pt)
        };
        self.push_clip_region(clip);
        self.backend.draw_text(origin, label, &style);
        self.pop_clip();
    }

    /// [`Device::text_at`] with a finite vertical adjustment `yc`, as
    /// `title()`/`mtext()`/`axis()`/`legend()` use.
    #[allow(clippy::too_many_arguments)]
    fn text_gc_at(
        &mut self,
        anchor: Point,
        label: &str,
        xc: f64,
        yc: f64,
        rotation_deg: f64,
        col: ColorSpec,
        cex: f64,
        font: i32,
    ) {
        self.text_at(anchor, label, xc, Some(yc), rotation_deg, col, cex, font);
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
        self.dirty();
        self.backend.draw_rect(self.plt, None, Some(&stroke));
    }

    /// `axis(side, at=, labels=, col=, cex.axis=, font=, las=)`: `col` is
    /// used for the line, ticks and labels. `at`/`labels` default to R's
    /// automatic ticks and labels. See [`Device::axis_with`] for the other
    /// arguments.
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
        let opts = AxisOptions {
            at,
            labels,
            col: Some(col),
            col_axis: Some(col),
            cex_axis: Some(cex_axis),
            font: Some(font_axis),
            las: Some(las),
            ..AxisOptions::default()
        };
        self.axis_with(side, &opts);
    }

    /// As [`Device::axis`] but draws only the tick labels - no axis line
    /// or tick marks - like R's `axis(..., tick = FALSE)` (`barplot()`
    /// uses `lty = 0`, which looks the same, for its category names).
    #[allow(clippy::too_many_arguments)]
    pub fn axis_labels(
        &mut self,
        side: Side,
        at: Option<Vec<f64>>,
        labels: Option<Vec<String>>,
        col: ColorSpec,
        cex_axis: f64,
        font_axis: i32,
        las: i32,
    ) {
        let opts = AxisOptions {
            at,
            labels,
            tick: false,
            col: Some(col),
            col_axis: Some(col),
            cex_axis: Some(cex_axis),
            font: Some(font_axis),
            las: Some(las),
            ..AxisOptions::default()
        };
        self.axis_with(side, &opts);
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
        // Line offsets follow `C_title` + `GMtext()` (plot.c/graphics.c):
        // text is baseline-anchored (yadj = 0) and `GMtext` nudges the line
        // by `yLineBias` (0.2 on pdf/png/svg devices) - `1 - bias` on side 1,
        // `bias` on sides 2/3.
        // In `C_title`'s order: main, sub, xlab, ylab.
        if let Some(main) = main {
            // `vpos = 0.5 * mar[2]`, `adjy = 0.5` (finite, so `GEText`
            // uses `yc * height(M)`, not exact ink centring).
            let y = self.plt.y1 + 0.5 * self.par.mar.top * lh;
            let x = (self.plt.x0 + self.plt.x1) / 2.0;
            self.text_gc_at(
                Point::new(x, y),
                main,
                0.5,
                0.5,
                0.0,
                ColorSpec::Color(self.par.col_main),
                self.par.cex_main,
                self.par.font_main,
            );
        }
        if let Some(sub) = sub {
            let y = self.plt.y0 - (self.par.mgp[0] + 1.0 + (1.0 - Y_LINE_BIAS)) * lh;
            let x = (self.plt.x0 + self.plt.x1) / 2.0;
            self.text_gc_at(
                Point::new(x, y),
                sub,
                0.5,
                0.0,
                0.0,
                ColorSpec::Color(self.par.col_sub),
                self.par.cex_sub,
                self.par.font_sub,
            );
        }
        if let Some(xlab) = xlab {
            let y = self.plt.y0 - (self.par.mgp[0] + (1.0 - Y_LINE_BIAS)) * lh;
            let x = (self.plt.x0 + self.plt.x1) / 2.0;
            self.text_gc_at(
                Point::new(x, y),
                xlab,
                0.5,
                0.0,
                0.0,
                ColorSpec::Color(self.par.col_lab),
                self.par.cex_lab,
                self.par.font_lab,
            );
        }
        if let Some(ylab) = ylab {
            let x = self.plt.x0 - (self.par.mgp[0] + Y_LINE_BIAS) * lh;
            let y = (self.plt.y0 + self.plt.y1) / 2.0;
            self.text_gc_at(
                Point::new(x, y),
                ylab,
                0.5,
                0.0,
                90.0,
                ColorSpec::Color(self.par.col_lab),
                self.par.cex_lab,
                self.par.font_lab,
            );
        }
        self.par.xpd = saved_xpd;
    }

    /// `arrows(x0, y0, x1, y1, length, angle, code, col =, lwd =, lty =)`,
    /// a port of `C_arrows` / `GArrow()`: each segment is drawn with open
    /// heads - two lines of `length` inches at `angle` degrees to the shaft -
    /// at its start (`code = 1`), end (`2`, R's default) or both ends (`3`).
    /// `angle = 90` gives flat caps, as for error bars. R's defaults are
    /// `length = 0.25`, `angle = 30`, `code = 2`.
    ///
    /// As in R, segments with a non-finite coordinate are skipped, and the
    /// heads of a segment shorter than 1/1000 inch are not drawn (R also
    /// warns). Clipped according to `par(xpd)`.
    #[allow(clippy::too_many_arguments)]
    pub fn arrows(
        &mut self,
        x0: &[f64],
        y0: &[f64],
        x1: &[f64],
        y1: &[f64],
        length: f64,
        angle: f64,
        code: u8,
        style: LineStyle,
    ) {
        let Some(color) = self.resolve(style.col) else {
            return;
        };
        let stroke = self.stroke_of(color, style.lwd, style.lty);
        let angle = angle.to_radians();
        self.push_clip_for(self.par.xpd);
        for i in 0..x0.len().min(y0.len()).min(x1.len()).min(y1.len()) {
            if ![x0[i], y0[i], x1[i], y1[i]].iter().all(|v| v.is_finite()) {
                continue;
            }
            let from = self.user_to_inches(x0[i], y0[i]);
            let to = self.user_to_inches(x1[i], y1[i]);
            self.backend.draw_line(from, to, &stroke);
            if code & 3 == 0 || length == 0.0 || (from.x - to.x).hypot(from.y - to.y) < 1e-3 {
                continue;
            }
            // A head at `tip`, opening back along the shaft towards `tail`.
            let mut head = |tip: Point, tail: Point| {
                let rot = (tail.y - tip.y).atan2(tail.x - tip.x);
                let barb = |a: f64| {
                    Point::new(tip.x + length * (rot + a).cos(), tip.y + length * (rot + a).sin())
                };
                self.backend
                    .draw_polyline(&[barb(angle), tip, barb(-angle)], &stroke);
            };
            if code & 1 != 0 {
                head(from, to);
            }
            if code & 2 != 0 {
                head(to, from);
            }
        }
        self.pop_clip();
    }

    pub fn backend_mut(&mut self) -> &mut B {
        &mut self.backend
    }
}

/// Default numeric axis label formatting: R's `axis()` uses roughly
/// `format(at, trim = TRUE)`, which drops unnecessary trailing zeros.
///
/// The tick vector is formatted *jointly*, like R's `formatReal()` (7
/// significant digits): all labels share the same number of decimals
/// (`0.0 0.5 1.0`, not `0 0.5 1`), and scientific notation is chosen when
/// it is strictly narrower than fixed notation (`0 1e+05 2e+05`).
fn format_axis_labels(vals: &[f64]) -> Vec<String> {
    // (significant digits, decimal exponent) of each finite value.
    let sci: Vec<Option<(i32, i32)>> = vals
        .iter()
        .map(|&v| {
            if !v.is_finite() {
                return None;
            }
            if v == 0.0 {
                return Some((1, 0));
            }
            let s = format!("{:.6e}", v.abs());
            let (mant, exp) = s.split_once('e')?;
            let digits = mant.replace('.', "");
            let nsig = digits.trim_end_matches('0').len().max(1) as i32;
            Some((nsig, exp.parse().ok()?))
        })
        .collect();
    let finite: Vec<(i32, i32)> = sci.iter().flatten().copied().collect();
    if finite.is_empty() {
        return vals.iter().map(|v| format!("{v}")).collect();
    }
    let rgt = finite.iter().map(|&(s, e)| s - e - 1).max().unwrap().max(0);
    let left = finite.iter().map(|&(_, e)| e + 1).max().unwrap().max(1);
    let mxsl = finite.iter().map(|&(s, _)| s).max().unwrap();
    let max_abs_exp = finite.iter().map(|&(_, e)| e.abs()).max().unwrap();
    let neg = vals.iter().any(|&v| v < 0.0) as i32;
    let fixed_w = neg + left + if rgt > 0 { rgt + 1 } else { 0 };
    let exp_w = if max_abs_exp >= 100 { 5 } else { 4 };
    let sci_w = neg + if mxsl > 1 { mxsl + 1 } else { mxsl } + exp_w;
    vals.iter()
        .map(|&v| {
            if !v.is_finite() {
                return format!("{v}");
            }
            if fixed_w <= sci_w {
                format!("{:.*}", rgt as usize, if v == 0.0 { 0.0 } else { v })
            } else {
                let s = format!("{:.*e}", (mxsl - 1) as usize, v);
                let (m, e) = s.split_once('e').unwrap();
                let e: i32 = e.parse().unwrap();
                format!("{m}e{}{:02}", if e < 0 { '-' } else { '+' }, e.abs())
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::format_axis_labels;

    fn f(v: &[f64]) -> Vec<String> {
        format_axis_labels(v)
    }

    #[test]
    fn axis_labels_share_decimals() {
        assert_eq!(f(&[0.0, 0.5, 1.0]), ["0.0", "0.5", "1.0"]);
        assert_eq!(f(&[0.0, 2.0, 4.0]), ["0", "2", "4"]);
        assert_eq!(f(&[-1.0, -0.5, 0.0]), ["-1.0", "-0.5", "0.0"]);
        assert_eq!(f(&[0.0, 0.25, 0.5]), ["0.00", "0.25", "0.50"]);
    }

    #[test]
    fn axis_labels_scientific_when_narrower() {
        assert_eq!(f(&[0.0, 1e5, 2e5]), ["0e+00", "1e+05", "2e+05"]);
        assert_eq!(f(&[0.0, 50000.0, 100000.0]), ["0e+00", "5e+04", "1e+05"]);
        assert_eq!(f(&[0.0, 2000.0, 4000.0]), ["0", "2000", "4000"]);
    }
}
