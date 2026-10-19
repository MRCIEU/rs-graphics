// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! The stateful graphics device: coordinate transforms (`plot.new()`/
//! `plot.window()`), clipping, and the primitive drawing operations
//! (`points`, `lines`, `segments`, `rect`, `polygon`, `text`, `box`,
//! `abline`, `axis`, `title`, `mtext`, `legend`, `arrows`).
//!
//! Each R function is one method taking its data and an options struct
//! (`dev.axis(1, &AxisOptions { .. })`), whose unset fields are R's
//! defaults, mostly the `par()` values. `points()`, `lines()`,
//! `segments()`, `arrows()`, `rect()`, `polygon()`, `text()`, `box()` and
//! `plot.window()` are in `primitives.rs`; `axis()`, `abline()`,
//! `legend()` and `mtext()` in their own child modules; the page,
//! layout, coordinate and clipping state, `plot.new()` and `title()` here. Nearly every high-level plot (`plot()`,
//! `hist()`, `barplot()`, ...) is built purely out of these primitives,
//! mirroring how R's own `graphics.c` centers everything around a handful
//! of `GE*` engine calls.

use crate::afm::{self};
use std::path::Path;

use crate::backend::pdf::PdfBackend;
use crate::backend::raster::RasterBackend;
use crate::backend::svg::SvgBackend;
use crate::backend::{AnyBackend, Backend, Stroke, TextStyle};
use crate::color::{Color, ColorSpec};
use crate::error::ParseValueError;
use crate::geometry::{Point, Rect};
use crate::layout::{Arrangement, Layout};
use crate::lty::LineType;
use crate::par::{AxisStyle, Par, Pty, Sides};
use crate::pch;
use crate::plotmath::Label;
use crate::pretty::{axis_pars, create_at_vector};

mod abline;
mod axis;
mod legend;
mod mtext;
mod plotmath;
mod primitives;
pub use abline::AblineOptions;
pub use axis::AxisOptions;
pub use legend::{LegendEntry, LegendKeyword, LegendLayout, LegendOptions, LegendPos};
pub use mtext::MtextOptions;
pub use primitives::{
    ArrowsOptions, BoxOptions, BoxWhich, PlotWindowOptions, PolygonOptions, RectOptions,
    SegmentsOptions, TextOptions, XyOptions,
};

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
    /// `par(fig)` (R's `defaultFigure = FALSE`): the figure region as
    /// fractions `[x0, x1, y0, y1]` of the inner region, overriding the
    /// arrangement until `mfrow`/`mfcol`/`layout` is called.
    fig_nic: Option<[f64; 4]>,
    /// `par("new")`, set by [`Device::set_new`] and [`Device::mfg`]: the
    /// next `plot_new()` reuses the current figure instead of advancing.
    /// Cleared by drawing (see [`Device::dirty`]), not by `plot_new()`.
    hold_figure: bool,
    /// Whether anything has been plotted yet (R's `gpptr(dd)->state`).
    used: bool,
    /// `par.ps` when the device was created: R's `startps`, which sets the
    /// device's character size `cra` (`1.2 * startps` points high).
    start_ps: f64,
}

/// Where drawing is clipped to: R's `xpd = FALSE` (plot region), `TRUE`
/// (figure region) and `NA` (the whole device).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Clip {
    Plot,
    Figure,
    Device,
}

/// The labels of [`Device::title`]; `None` draws nothing.
#[derive(Clone, Debug, Default)]
pub struct TitleOptions {
    pub main: Option<Label>,
    pub sub: Option<Label>,
    pub xlab: Option<Label>,
    pub ylab: Option<Label>,
}

crate::setters::setters!(TitleOptions {
    main: option Label,
    sub: option Label,
    xlab: option Label,
    ylab: option Label,
});

/// Shortcuts for a device with R's default `par()`. Use [`Device::new`]
/// to start from other parameters: in particular `par.ps`, R's
/// `pointsize`, sizes symbols and margin lines from creation on.
impl Device<PdfBackend> {
    /// R's `pdf(width = , height = )`: a PDF page `width_in` x
    /// `height_in` inches.
    pub fn pdf(width_in: f64, height_in: f64) -> Self {
        Device::new(PdfBackend::new(width_in, height_in), Par::default())
    }
}

impl Device<SvgBackend> {
    /// R's `svg(width = , height = )`.
    pub fn svg(width_in: f64, height_in: f64) -> Self {
        Device::new(SvgBackend::new(width_in, height_in), Par::default())
    }
}

impl Device<RasterBackend> {
    /// A PNG of `width_in` x `height_in` inches at `dpi` (R's `png(width,
    /// height, units = "in", res = dpi)`), on white.
    pub fn png(width_in: f64, height_in: f64, dpi: f64) -> Self {
        Device::new(
            RasterBackend::new(width_in, height_in, dpi, Color::WHITE),
            Par::default(),
        )
    }
}

impl Device<AnyBackend> {
    /// A device whose format follows `path`'s extension (`.pdf`, `.svg`
    /// or `.png`; `dpi` is used for PNG only), for
    /// [`save`](Device::save) to the same path. Fails for other
    /// extensions.
    pub fn for_path(
        path: impl AsRef<Path>,
        width_in: f64,
        height_in: f64,
        dpi: f64,
    ) -> Result<Self, ParseValueError> {
        let backend = AnyBackend::for_path(path, width_in, height_in, dpi)?;
        Ok(Device::new(backend, Par::default()))
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
        let par_ps = par.ps;
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
            fig_nic: None,
            hold_figure: false,
            used: false,
            start_ps: par_ps,
        };
        dev.recompute_layout();
        dev
    }

    /// The backend, to finish it with its own `finish()` or set options.
    pub fn into_backend(self) -> B {
        self.backend
    }

    /// The finished file's bytes (PDF, PNG, or SVG as UTF-8).
    pub fn finish(self) -> Vec<u8> {
        self.backend.into_bytes()
    }

    /// Writes the finished file to `path` (R's `dev.off()`). The format is
    /// the backend's, whatever `path`'s extension; see
    /// [`Device::for_path`] to choose the backend from the path.
    pub fn save(self, path: impl AsRef<Path>) -> std::io::Result<()> {
        std::fs::write(path, self.finish())
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
        let omi = self.omi();
        let inner = Rect::new(
            omi.left,
            omi.bottom,
            self.width_in - omi.right,
            self.height_in - omi.top,
        );
        let [fx0, fx1, fy0, fy1] = self.fig_nic.unwrap_or_else(|| {
            self.arrangement
                .figure_region_nic(self.current_figure, inner.width(), inner.height())
        });
        self.fig = Rect::new(
            inner.x0 + fx0 * inner.width(),
            inner.y0 + fy0 * inner.height(),
            inner.x0 + fx1 * inner.width(),
            inner.y0 + fy1 * inner.height(),
        );
        self.plt = match self.par.plt {
            // `par(plt)`: fractions of the figure region.
            Some([px0, px1, py0, py1]) => Rect::new(
                self.fig.x0 + px0 * self.fig.width(),
                self.fig.y0 + py0 * self.fig.height(),
                self.fig.x0 + px1 * self.fig.width(),
                self.fig.y0 + py1 * self.fig.height(),
            ),
            // `mapPlotRegion()`: the figure less `mar`, made square about
            // its centre for `pty = "s"`.
            None => {
                let mai = self.mai();
                let (mut x0, mut x1) = (self.fig.x0 + mai.left, self.fig.x1 - mai.right);
                let (mut y0, mut y1) = (self.fig.y0 + mai.bottom, self.fig.y1 - mai.top);
                if self.par.pty == Pty::Square {
                    let (w, h) = (x1 - x0, y1 - y0);
                    if w > h {
                        let c = 0.5 * (x0 + x1);
                        (x0, x1) = (c - 0.5 * h, c + 0.5 * h);
                    } else {
                        let c = 0.5 * (y0 + y1);
                        (y0, y1) = (c - 0.5 * w, c + 0.5 * w);
                    }
                }
                Rect::new(x0, y0, x1, y1)
            }
        };
    }

    /// `par(fig = c(x0, x1, y0, y1))`: a single figure region, as fractions
    /// of the inner region (the device less `par(oma)`), for placing plots
    /// anywhere on the page. As in R, the next `plot_new()` starts a new
    /// page unless [`Device::set_new`]`(true)` is called after this (to
    /// add a plot to the current page), `par(cex)` is left as it is, and
    /// `mfrow`/`mfcol`/`layout` cancel it.
    ///
    /// # Panics
    /// Unless `0 <= x0 < x1 <= 1` and `0 <= y0 < y1 <= 1` (R's error).
    pub fn fig(&mut self, fig: [f64; 4]) {
        let [x0, x1, y0, y1] = fig;
        assert!(
            0.0 <= x0 && x0 < x1 && x1 <= 1.0 && 0.0 <= y0 && y0 < y1 && y1 <= 1.0,
            "invalid value specified for graphical parameter \"fig\""
        );
        self.arrangement = Arrangement::Grid {
            nrow: 1,
            ncol: 1,
            by_col: false,
        };
        self.current_figure = 1;
        self.fig_nic = Some(fig);
        self.recompute_layout();
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
        self.fig_nic = None;
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

    /// `plot.new()`: moves to the next figure and sets up its figure and
    /// plot regions.
    ///
    /// - **Coordinates:** like R, it leaves linear user coordinates of
    ///   0..1 on both axes (as `plot_window((0, 1), (0, 1), ..)`:
    ///   -0.04..1.04 with the default `xaxs = yaxs = "r"`) and their
    ///   default ticks, so you can draw straight away. Call
    ///   [`Device::plot_window`] next for other limits or log axes.
    /// - **Which figure:** the next one in the order set by
    ///   [`Device::mfrow`], [`Device::mfcol`], [`Device::layout`] or
    ///   [`Device::mfg`] (one figure per page by default). After the last
    ///   figure, it wraps to figure 1 on a new page. The first call always
    ///   starts page one. After [`Device::fig`], the next call starts a
    ///   new page unless `set_new(true)` comes first.
    /// - **`par(new = TRUE)`:** after [`Device::set_new`]`(true)` it stays
    ///   in the current figure and draws over it. The flag is not cleared
    ///   here; any drawing clears it, as in R.
    /// - **Regions:** figure and plot regions are computed now from
    ///   `par.mar`/`oma` (in lines of [`Device::line_height_inches`], so
    ///   `par.cex`, `mex` and the starting `ps` count too), `par.pty` and
    ///   `par.plt`. Changing these afterwards has no
    ///   effect until the next call.
    /// - **New page:** what happens depends on the backend. PDF starts
    ///   a new page in the file, SVG discards the previous page, and PNG
    ///   paints over it. The page is then filled with `par.bg`, unless
    ///   that is fully transparent, which it is by default on PDF (as R's
    ///   `pdf()`).
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
            self.backend.new_page();
            if self.par.bg.a > 0 {
                self.backend.draw_rect(
                    Rect::new(0.0, 0.0, self.width_in, self.height_in),
                    Some(self.par.bg),
                    None,
                );
            }
        }
        // `C_plot_new`: linear axes, `GScale(0, 1)` on both.
        self.set_window((0.0, 1.0), (0.0, 1.0), false, false);
    }

    /// The user coordinates for limits `xlim`/`ylim` (data units) on
    /// linear or log axes, through `GScale()`, and `par.xaxp`/`yaxp`; see
    /// [`Device::plot_window`].
    fn set_window(&mut self, xlim: (f64, f64), ylim: (f64, f64), xlog: bool, ylog: bool) {
        self.xlog = xlog;
        self.ylog = ylog;
        let (ux0, ux1) = Self::gscale(xlim.0, xlim.1, self.par.xaxs, xlog);
        let (uy0, uy1) = Self::gscale(ylim.0, ylim.1, self.par.yaxs, ylog);
        self.usr = (ux0, ux1, uy0, uy1);
        self.par.xaxp = axis_pars(ux0, ux1, self.par.lab[0], xlog);
        self.par.yaxp = axis_pars(uy0, uy1, self.par.lab[1], ylog);
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
    /// `at=` is given (R's `axTicks(side)`), in data units: from
    /// `par("xaxp")` (sides 1, 3) or `par("yaxp")` (sides 2, 4), which
    /// `plot_new()` and `plot_window()` set with `pretty()` (or R's
    /// log-axis rules) and `par("lab")`.
    pub fn default_ticks(&self, side: Side) -> Vec<f64> {
        let axp = if side == 1 || side == 3 { self.par.xaxp } else { self.par.yaxp };
        self.ticks_from_axp(side, axp)
    }

    /// `CreateAtVector()` for `side`'s axis with the given `axp`.
    fn ticks_from_axp(&self, side: Side, axp: [f64; 3]) -> Vec<f64> {
        let (lo, hi, log, nint) = if side == 1 || side == 3 {
            (self.usr.0, self.usr.1, self.xlog, self.par.lab[0])
        } else {
            (self.usr.2, self.usr.3, self.ylog, self.par.lab[1])
        };
        let data = |v: f64| if log { 10f64.powf(v) } else { v };
        create_at_vector(axp, data(lo), data(hi), nint, log)
    }

    // ---------------------------------------------------------------
    // Coordinate transforms
    // ---------------------------------------------------------------

    /// Map a point in data units (not log10 on a log axis) to device
    /// inches.
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

    /// The device's starting point size (R's `startps`): `par.ps` as
    /// passed to [`Device::new`]. Margin lines, plotting symbols and
    /// `par("cin")` are sized from it, as from R's `cra`; changing
    /// `par.ps` later only changes text size.
    pub fn start_ps(&self) -> f64 {
        self.start_ps
    }

    /// The height in inches of one margin line, the unit of `mar`, `oma`,
    /// `mgp` and `tcl`: `mex * cex * 1.2 * startps / 72` (R's
    /// `yNDCPerLine`, from `cra[1]`, in `mapNDC2Dev()`).
    pub fn line_height_inches(&self) -> f64 {
        self.par.mex * self.par.cex * 1.2 * self.start_ps / 72.0
    }

    /// `par("mai")`: `par.mar` in inches (bottom, left, top, right).
    pub fn mai(&self) -> Sides<f64> {
        let lh = self.line_height_inches();
        let m = self.par.mar;
        Sides::new(m.bottom * lh, m.left * lh, m.top * lh, m.right * lh)
    }

    /// `par("omi")`: `par.oma` in inches (bottom, left, top, right).
    pub fn omi(&self) -> Sides<f64> {
        let lh = self.line_height_inches();
        let m = self.par.oma;
        Sides::new(m.bottom * lh, m.left * lh, m.top * lh, m.right * lh)
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
    // clips to the figure region. `par.xpd` is a bool, so R's `NA`, "clip
    // to the device", is only available internally as `Clip::Device`,
    // used by `axis()` and `mtext(outer = TRUE)`).
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

    /// One symbol, as `GSymbol()`/`GESymbol()` draw it. `cex` is the
    /// per-point `cex` (R multiplies it by `cexbase`, i.e. `par(cex)`).
    fn draw_pch(&mut self, pch: i32, p: Point, cex: f64, col: Color, bg: Option<Color>, lwd: f64) {
        let cex = self.par.cex * cex;
        let size = pch::symbol_size_inches(self.start_ps, cex);
        if let Some(geom) = pch::pch_geometry(pch, p.x, p.y, size) {
            let stroke = Stroke::simple(col, lwd);
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

    /// `lines()`: a polyline, broken into separate pieces at non-finite
    /// (`NA`) coordinates, and at values <= 0 on a log axis, matching R.
    fn polyline_xy(
        &mut self,
        xs: &[f64],
        ys: &[f64],
        col: ColorSpec,
        lwd: f64,
        lty: LineType,
        clip: Clip,
    ) {
        let Some(col) = self.resolve(col) else {
            return;
        };
        let stroke = self.stroke_of(col, lwd, lty);
        self.push_clip_region(clip);
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
    /// height (`0.5 * par(cex) * 1.2 * startps / 72` inches) so the points stand
    /// clear, and left out when shorter than twice that. A port of
    /// `C_plotXY`'s `case 'b'`.
    fn broken_lines_xy(
        &mut self,
        xs: &[f64],
        ys: &[f64],
        col: ColorSpec,
        lwd: f64,
        lty: LineType,
        clip: Clip,
    ) {
        let Some(col) = self.resolve(col) else {
            return;
        };
        let stroke = self.stroke_of(col, lwd, lty);
        let d = 0.5 * self.par.cex * 1.2 * self.start_ps / 72.0;
        self.push_clip_region(clip);
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

    /// `rect()` variant taking a rectangle already in device inches. Not
    /// clipped.
    pub fn rect_inches(&mut self, rect: Rect, fill: Option<Color>, stroke: Option<&Stroke>) {
        self.dirty();
        self.backend.draw_rect(rect, fill, stroke);
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

    fn draw_line_if_finite(&mut self, from: Point, to: Point, stroke: &Stroke) {
        if [from.x, from.y, to.x, to.y].iter().all(|v| v.is_finite()) {
            self.backend.draw_line(from, to, stroke);
        }
    }

    // ---------------------------------------------------------------
    // Text
    // ---------------------------------------------------------------

    /// `(strwidth(text, "inches", cex, font), strheight(...))`: R's
    /// `GEStrWidth`/`GEStrHeight` at `par(ps) * par(cex) * cex` points
    /// (after the backend's font-size rounding). The width is that of the
    /// widest line; the height is the ascent of "M" plus one line height
    /// (`1.2 * ps / 72` inches, times the cex) per `\n`.
    pub fn text_metrics(&self, text: &str, font: i32, cex: f64) -> (f64, f64) {
        let face = self.par.font_face(font);
        let size = self.font_size_pt(cex);
        let width = text
            .split('\n')
            .map(|line| afm::str_width_inches(line, face, size, 1.0))
            .fold(0.0, f64::max);
        let newlines = text.matches('\n').count() as f64;
        let line = 1.2 * self.par.ps * self.par.cex * cex / 72.0;
        (width, newlines * line + afm::m_height_inches(face, size, 1.0))
    }

    /// Draws one line of text at `anchor` (device inches), clipped to
    /// `clip`. Placement follows `GEText()`: horizontally `hadj` of the
    /// string's width; vertically, a finite `vadj` puts the baseline
    /// `vadj * height("M")` below the anchor, while `None` (`NA`) centres the ink
    /// of the string's glyphs on it (baseline at `anchor - (ury + lly) / 2`
    /// of the glyph bounding boxes).
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


    // ---------------------------------------------------------------
    // title()
    // ---------------------------------------------------------------

    /// `title(main=, sub=, xlab=, ylab=)`, each text or a plotmath
    /// expression, drawn in `C_title`'s order (main, sub, xlab, ylab) with
    /// `par(cex.main/col.main/font.main)` etc. Text is placed by `GMtext()`
    /// (baseline on the line, nudged by `yLineBias`); expressions by
    /// `GMathText()` (`main`: box centred on its line) and `GMMathText()`
    /// (the others: box bottom on the nudged line).
    pub fn title(&mut self, opts: &TitleOptions) {
        let saved_xpd = self.par.xpd;
        self.par.xpd = true;
        let lh = self.line_height_inches();
        let x_mid = (self.plt.x0 + self.plt.x1) / 2.0;
        let y_mid = (self.plt.y0 + self.plt.y1) / 2.0;
        // Line offsets follow `C_title` + `GMtext()` (plot.c/graphics.c):
        // `GMtext` nudges the line by `yLineBias` (0.2 on pdf/png/svg
        // devices) - `1 - bias` on side 1, `bias` on sides 2/3.
        if let Some(main) = &opts.main {
            // `vpos = 0.5 * mar[2]`, `adjy = 0.5` (finite, so `GEText`
            // uses `yc * height(M)`, not exact ink centring).
            let y = self.plt.y1 + 0.5 * self.par.mar.top * lh;
            self.draw_label(
                Clip::Figure,
                Point::new(x_mid, y),
                main,
                0.5,
                Some(0.5),
                0.0,
                ColorSpec::Color(self.par.col_main),
                self.par.cex_main,
                self.par.font_main,
            );
        }
        if let Some(sub) = &opts.sub {
            let y = self.plt.y0 - (self.par.mgp[0] + 1.0 + (1.0 - Y_LINE_BIAS)) * lh;
            self.draw_label(
                Clip::Figure,
                Point::new(x_mid, y),
                sub,
                0.5,
                Some(0.0),
                0.0,
                ColorSpec::Color(self.par.col_sub),
                self.par.cex_sub,
                self.par.font_sub,
            );
        }
        if let Some(xlab) = &opts.xlab {
            let y = self.plt.y0 - (self.par.mgp[0] + (1.0 - Y_LINE_BIAS)) * lh;
            self.draw_label(
                Clip::Figure,
                Point::new(x_mid, y),
                xlab,
                0.5,
                Some(0.0),
                0.0,
                ColorSpec::Color(self.par.col_lab),
                self.par.cex_lab,
                self.par.font_lab,
            );
        }
        if let Some(ylab) = &opts.ylab {
            let x = self.plt.x0 - (self.par.mgp[0] + Y_LINE_BIAS) * lh;
            self.draw_label(
                Clip::Figure,
                Point::new(x, y_mid),
                ylab,
                0.5,
                Some(0.0),
                90.0,
                ColorSpec::Color(self.par.col_lab),
                self.par.cex_lab,
                self.par.font_lab,
            );
        }
        self.par.xpd = saved_xpd;
    }

    pub fn backend_mut(&mut self) -> &mut B {
        &mut self.backend
    }
}

/// Default numeric axis label formatting, as R's `axis()` does it.
///
/// The tick vector is formatted *jointly*, like R's `formatReal()` (7
/// significant digits): all labels share the same number of decimals
/// (`0.0 0.5 1.0`, not `0 0.5 1`), and scientific notation is chosen when
/// it is strictly narrower than fixed notation (`0 1e+05 2e+05`).
pub(crate) fn format_axis_labels(vals: &[f64]) -> Vec<String> {
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
