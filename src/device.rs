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
use crate::pretty::ge_pretty;

mod legend;
pub use legend::{LegendEntry, LegendKeyword, LegendLayout, LegendOptions, LegendPos};

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
    /// `par(new = TRUE)` as set by [`Device::mfg`]: the next `plot_new()`
    /// reuses the current figure instead of advancing.
    hold_figure: bool,
    /// Whether anything has been plotted yet (R's `gpptr(dd)->state`).
    used: bool,
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
        self.hold_figure = false;
        self.used = true;
        self.recompute_layout();
        if new_page {
            self.backend.draw_rect(
                Rect::new(0.0, 0.0, self.width_in, self.height_in),
                Some(self.par.bg),
                None,
            );
        }
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
        self.push_clip_for(self.par.xpd);
        for (&x, &y) in xs.iter().zip(ys) {
            if !x.is_finite() || !y.is_finite() {
                continue;
            }
            let p = self.user_to_inches(x, y);
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
        self.push_clip_for(self.par.xpd);
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
        self.backend.draw_rect(self.plt, None, Some(&stroke));
    }

    /// `axis(side, at=, labels=, ...)`. `at`/`labels` default to R's
    /// automatic tick placement (`pretty()` on the current axis range)
    /// and `format!("{}", v)`-style numeric labels.
    ///
    /// Follows `C_axis()` in `plot.c`: the axis line runs from the first to
    /// the last tick (clamped to the plot range), ticks use the signed
    /// `par(tcl)`, and labels are placed by `GMtext()` at line `mgp[1]`
    /// (plus its `yLineBias` nudge) with `yc = padj`, rotated/justified
    /// according to `las`, and dropped when they would overlap the previous
    /// label by less than `gap.axis`.
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
        self.axis_impl(side, at, labels, col, cex_axis, font_axis, las, true);
    }

    /// As [`Device::axis`] but draws only the tick labels - no axis line
    /// or tick marks - like R's `axis(..., lty = 0)` (used by `barplot()`
    /// for its category names).
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
        self.axis_impl(side, at, labels, col, cex_axis, font_axis, las, false);
    }

    #[allow(clippy::too_many_arguments)]
    fn axis_impl(
        &mut self,
        side: Side,
        at: Option<Vec<f64>>,
        labels: Option<Vec<String>>,
        col: ColorSpec,
        cex_axis: f64,
        font_axis: i32,
        las: i32,
        draw_line_and_ticks: bool,
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
        let labels = labels.unwrap_or_else(|| format_axis_labels(&ticks));
        let stroke = self.stroke_of(color, self.par.lwd, LineType::Pattern(&[]));
        let lh = self.line_height();
        let is_x = side == 1 || side == 3;

        // Position of the axis line, and which way is "outward" (away from
        // the plot region) along the perpendicular coordinate.
        let (edge, outward) = match side {
            1 => (self.plt.y0, -1.0),
            2 => (self.plt.x0, -1.0),
            3 => (self.plt.y1, 1.0),
            _ => (self.plt.x1, 1.0),
        };
        let at_perp = |perp: f64, along: f64| {
            if is_x {
                Point::new(along, perp)
            } else {
                Point::new(perp, along)
            }
        };
        let (a_lo, a_hi) = if is_x {
            (self.plt.x0, self.plt.x1)
        } else {
            (self.plt.y0, self.plt.y1)
        };

        // Where each tick sits along the axis (device inches), and whether
        // it lies inside the plot's user range.
        let mut items: Vec<(f64, bool, &str)> = Vec::new();
        for (v, label) in ticks.iter().zip(labels.iter()) {
            let (along, inside) = if is_x {
                let uv = if self.xlog { 10f64.powf(*v) } else { *v };
                let inside = uv >= self.range_min(true) - 1e-9 && uv <= self.range_max(true) + 1e-9;
                (self.user_to_inches(uv, self.range_min(false)).x, inside)
            } else {
                let uv = if self.ylog { 10f64.powf(*v) } else { *v };
                let inside =
                    uv >= self.range_min(false) - 1e-9 && uv <= self.range_max(false) + 1e-9;
                (self.user_to_inches(self.range_min(true), uv).y, inside)
            };
            if along.is_finite() {
                items.push((along, inside, label.as_str()));
            }
        }
        if items.is_empty() {
            self.par.xpd = saved_xpd;
            return;
        }
        // R sorts `at` and draws labels in increasing position order.
        items.sort_by(|a, b| a.0.total_cmp(&b.0));

        // Axis line: first to last tick, clamped to the plot range.
        let base = edge + outward * self.par.mgp[2] * lh;
        let first = items.first().unwrap().0.clamp(a_lo, a_hi);
        let last = items.last().unwrap().0.clamp(a_lo, a_hi);
        if draw_line_and_ticks {
            self.backend
                .draw_line(at_perp(base, first), at_perp(base, last), &stroke);
        }

        // Ticks (`tcl` is signed: negative = outward, R's default).
        let tick_end = base - outward * self.par.tcl * lh;
        for &(along, inside, _) in &items {
            if inside && draw_line_and_ticks {
                self.backend
                    .draw_line(at_perp(base, along), at_perp(tick_end, along), &stroke);
            }
        }

        // Labels.
        let perpendicular = if is_x {
            las == 2 || las == 3
        } else {
            las == 1 || las == 2
        };
        // `ComputePAdjValue()` with the default `padj = NA`.
        let padj = match las {
            1 => {
                if is_x {
                    0.0
                } else {
                    0.5
                }
            }
            2 => 0.5,
            3 => {
                if is_x {
                    0.5
                } else {
                    0.0
                }
            }
            _ => 0.0,
        };
        // `par(adj)` override that `C_axis()` applies for the labels.
        let hadj = if perpendicular {
            if side == 1 || side == 2 {
                1.0
            } else {
                0.0
            }
        } else {
            0.5
        };
        // `GMtext()`'s rotation and line-bias rules.
        let (angle, bias) = match side {
            1 => {
                if las == 2 || las == 3 {
                    (90.0, 0.0)
                } else {
                    (0.0, 1.0 - Y_LINE_BIAS)
                }
            }
            2 => {
                if las == 1 || las == 2 {
                    (0.0, 0.0)
                } else {
                    (90.0, Y_LINE_BIAS)
                }
            }
            3 => {
                if las == 2 || las == 3 {
                    (90.0, 0.0)
                } else {
                    (0.0, Y_LINE_BIAS)
                }
            }
            _ => {
                if las == 1 || las == 2 {
                    (0.0, 0.0)
                } else {
                    (90.0, 1.0 - Y_LINE_BIAS)
                }
            }
        };
        let label_perp = edge + outward * (self.par.mgp[1] + bias) * lh;

        let face = self.par.font_face(font_axis);
        let size_pt = self.font_size_pt(cex_axis);
        let m_h = afm::m_height_inches(face, size_pt, 1.0);
        let m_w = afm::str_width_inches("m", face, size_pt, 1.0);
        // `gap.axis` default: 1 "m" width (parallel) / 0.25 "m" height
        // (perpendicular) of clear space between adjacent labels.
        let gap = if perpendicular { 0.25 * m_h } else { m_w };

        let mut tlast = f64::NEG_INFINITY;
        for &(along, inside, label) in &items {
            // Labels are only drawn strictly inside the (epsilon-widened)
            // user range, like ticks.
            if !inside {
                continue;
            }
            let labw = if perpendicular {
                m_h
            } else {
                afm::str_width_inches(label, face, size_pt, 1.0)
            };
            let tnew = along - 0.5 * labw;
            if tnew - tlast >= gap {
                self.text_gc_at(
                    at_perp(label_perp, along),
                    label,
                    hadj,
                    padj,
                    angle,
                    ColorSpec::Color(color),
                    cex_axis,
                    font_axis,
                );
                tlast = along + 0.5 * labw;
            }
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
        // Line offsets follow `C_title` + `GMtext()` (plot.c/graphics.c):
        // text is baseline-anchored (yadj = 0) and `GMtext` nudges the line
        // by `yLineBias` (0.2 on pdf/png/svg devices) - `1 - bias` on side 1,
        // `bias` on sides 2/3.
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
