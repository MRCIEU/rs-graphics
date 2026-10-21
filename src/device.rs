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
//! Devices are opened for a file with [`Device::pdf`], [`Device::svg`],
//! [`Device::png`] or [`Device::format`] (a [`DeviceBuilder`], in
//! `open.rs`) and written by [`Device::close`]; [`Device::new`] with
//! [`Device::finish`] keeps the output in memory.
//!
//! The public entry to each R function is a [`Graphics`](crate::graphics::Graphics)
//! method; the drawing code is here, in crate-private `*_impl` methods
//! taking the data and an options struct, whose unset fields are R's
//! defaults, mostly the `par()` values. `points()`, `lines()`,
//! `segments()`, `arrows()`, `rect()`, `polygon()`, `text()`, `box()` and
//! `plot.window()` are in `primitives.rs`; `axis()`, `abline()`,
//! `legend()` and `mtext()` in their own child modules; the page,
//! layout, coordinate and clipping state, `plot.new()` and `title()` here. Nearly every high-level plot (`plot()`,
//! `hist()`, `barplot()`, ...) is built purely out of these primitives,
//! mirroring how R's own `graphics.c` centers everything around a handful
//! of `GE*` engine calls.

use crate::afm::{self};
use crate::font::FontFace;
use std::fs::File;
use std::io::Write;
use std::ops::{Deref, DerefMut};

use crate::backend::{Backend, LineEnd, LineJoin, Stroke, TextStyle};
use crate::color::{Color, ColorSpec};
use crate::geometry::{Point, Rect};
use crate::layout::{Arrangement, Layout};
use crate::lty::LineType;
use crate::par::{AxisStyle, Par, Pty, Sides, Xpd};
use crate::pch;
use crate::plotmath::Label;
use crate::pretty::{axis_pars, create_at_vector};

mod abline;
mod axis;
mod legend;
mod mtext;
mod open;
mod plotmath;
mod primitives;
pub use abline::AblineOptions;
pub use axis::AxisOptions;
pub(crate) use legend::check_legend;
pub use legend::{LegendBty, LegendKeyword, LegendLayout, LegendOptions, LegendPch, LegendPos};
pub use mtext::MtextOptions;
pub use open::DeviceBuilder;
pub use primitives::{
    ArrowsOptions, BoxOptions, BoxWhich, PlotWindowOptions, PolygonOptions, RectOptions,
    SegmentsOptions, TextOptions, XyOptions,
};

/// A side of the plot, figure or page, as R's `side` argument. Functions
/// take `impl Into<Side>`, so R's numbers work too: `1` = bottom, `2` =
/// left, `3` = top, `4` = right (`dev.axis().side(1)` is
/// `dev.axis().side(Side::Bottom)`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Side {
    Bottom = 1,
    Left = 2,
    Top = 3,
    Right = 4,
}

impl Side {
    /// All four sides, in R's order (1 to 4).
    pub const ALL: [Side; 4] = [Side::Bottom, Side::Left, Side::Top, Side::Right];

    /// R's number for the side (1 to 4).
    pub fn number(self) -> i32 {
        self as i32
    }

    /// Bottom or top: the sides that carry an x axis.
    pub fn is_horizontal(self) -> bool {
        matches!(self, Side::Bottom | Side::Top)
    }
}

impl From<i32> for Side {
    /// R's side numbers.
    ///
    /// # Panics
    /// If `n` isn't 1 to 4 (R: "invalid axis number").
    fn from(n: i32) -> Side {
        match n {
            1 => Side::Bottom,
            2 => Side::Left,
            3 => Side::Top,
            4 => Side::Right,
            _ => panic!("invalid side {n}: must be 1 (bottom), 2 (left), 3 (top) or 4 (right)"),
        }
    }
}

/// `dev->yLineBias` for R's pdf/png/svg devices (see `GMtext()`).
const Y_LINE_BIAS: f64 = 0.2;

/// A graphics device: a page (or several, for PDF) drawn onto a
/// [`Backend`], plus R's `par()` state and the page layout. Open one for
/// a file with [`Device::pdf`], [`Device::svg`], [`Device::png`] or
/// [`Device::format`], or wrap a backend with [`Device::new`]; draw with
/// the [`Graphics`](crate::graphics::Graphics) methods.
pub struct Device<B: Backend> {
    backend: B,
    /// R's `par()` state: changed by
    /// [`par()`](crate::graphics::Graphics::par) calls, read with
    /// [`Device::current_par`]. The page layout (`mfrow`, `layout`,
    /// `fig`, ...) is device state, not part of `Par`.
    pub(crate) par: Par,
    width_in: f64,
    height_in: f64,
    /// Inner region (the device minus the outer margins), as of the last
    /// `plot_new()`.
    inner: Rect,
    /// `par(mar)` as of the last `plot_new()`, for `title()`.
    mar: Sides<f64>,
    /// Figure region (part of the inner region).
    fig: Rect,
    /// Plot region (figure region minus `mar` margins).
    plt: Rect,
    /// R's error for invalid regions as of the last layout (see
    /// `region_error()`), raised by `plot_new()`.
    region_error: Option<&'static str>,
    /// `par(cex)` as of the last layout (R's `cexbase` in `xNDCPerChar`,
    /// set by `mapNDC2Dev()` at `plot.new()` and region changes, not by a
    /// later `par(cex = )`): the size of R's `CHARS` unit, e.g. `text()`'s
    /// `offset`.
    pub(crate) layout_cex: f64,
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
    /// `par(fig)` or `par(fin)` (R's `defaultFigure = FALSE`): one figure
    /// region, overriding the arrangement until `mfrow`/`mfcol`/`layout`
    /// is called.
    fig_spec: Option<FigSpec>,
    /// `par("new")`, set by `par(new = )` and `par(mfg = )`: the
    /// next `plot_new()` reuses the current figure instead of advancing.
    /// Cleared by drawing (see [`Device::dirty`]), not by `plot_new()`.
    hold_figure: bool,
    /// Whether anything has been plotted yet (R's `gpptr(dd)->state`).
    used: bool,
    /// `par.ps` when the device was created: R's `startps`, which sets the
    /// device's character size `cra` (`1.2 * startps` points high).
    start_ps: f64,
    /// The file created by [`DeviceBuilder::open`], which
    /// [`close`](Device::close) writes; `None` for [`Device::new`].
    file: Option<File>,
}

/// A figure region set with `fig()` or `fin()`.
#[derive(Clone, Copy, Debug, PartialEq)]
enum FigSpec {
    /// `par(fig)`: fractions `[x0, x1, y0, y1]` of the inner region.
    Nic([f64; 4]),
    /// `par(fin)`: `[width, height]` in inches, centred.
    Inches([f64; 2]),
}

/// The labels of [`Graphics::title`](crate::graphics::Graphics::title) (`None` draws
/// nothing) and R's inline `cex.main`/`col.main`/`font.main` and the
/// `sub` and `lab` ones (`None`: the `par()` value).
#[derive(Clone, Debug, Default)]
pub struct TitleOptions {
    /// Main title, above the plot.
    pub main: Option<Label>,
    /// Subtitle, below the x axis label.
    pub sub: Option<Label>,
    /// x axis label.
    pub xlab: Option<Label>,
    /// y axis label.
    pub ylab: Option<Label>,
    /// Size of `main`, relative to `par(cex)` (R's inline `cex.main`);
    /// `None`: `par(cex.main)`.
    pub cex_main: Option<f64>,
    /// Colour of `main` (R's inline `col.main`); `None`:
    /// `par(col.main)`.
    pub col_main: Option<ColorSpec>,
    /// Font of `main` (a [`FontFace`]; R's
    /// inline `font.main`); `None`: `par(font.main)`.
    pub font_main: Option<FontFace>,
    /// Size of `sub`, relative to `par(cex)` (R's inline `cex.sub`);
    /// `None`: `par(cex.sub)`.
    pub cex_sub: Option<f64>,
    /// Colour of `sub` (R's inline `col.sub`); `None`:
    /// `par(col.sub)`.
    pub col_sub: Option<ColorSpec>,
    /// Font of `sub` (a [`FontFace`]; R's
    /// inline `font.sub`); `None`: `par(font.sub)`.
    pub font_sub: Option<FontFace>,
    /// Size of `xlab` and `ylab`, relative to `par(cex)` (R's inline `cex.lab`);
    /// `None`: `par(cex.lab)`.
    pub cex_lab: Option<f64>,
    /// Colour of `xlab` and `ylab` (R's inline `col.lab`); `None`:
    /// `par(col.lab)`.
    pub col_lab: Option<ColorSpec>,
    /// Font of `xlab` and `ylab` (a [`FontFace`]; R's
    /// inline `font.lab`); `None`: `par(font.lab)`.
    pub font_lab: Option<FontFace>,
    /// Where each title sits along its side of the plot region, and how
    /// it's justified there: 0 left (bottom for `ylab`), 0.5 centred, 1
    /// right (top) (R's inline `adj`); `None`: `par(adj)`.
    pub adj: Option<f64>,
    /// Margin lines for `xlab`/`ylab` (`mgp[0]`) and `sub` (`mgp[0] + 1`)
    /// (R's inline `mgp`); `None`: `par(mgp)`.
    pub mgp: Option<[f64; 3]>,
    /// Margin line for every title given (R's `line`), instead of their
    /// defaults (half the top margin for `main`, `mgp[0]` for `xlab`/`ylab`,
    /// `mgp[0] + 1` for `sub`); `main` then sits with its baseline on it.
    /// `None` (or non-finite): the defaults.
    pub line: Option<f64>,
    /// Draw in the outer margins (`par(oma)`) instead, positioned along the
    /// inner region (R's `outer = TRUE`); `main` defaults to half the top
    /// outer margin. Default `false`.
    pub outer: bool,
}

crate::setters::setters!(TitleOptions => [TitleCall] {
    main: option Label,
    sub: option Label,
    xlab: option Label,
    ylab: option Label,
    cex_main: option f64,
    col_main: option ColorSpec,
    font_main: option FontFace,
    cex_sub: option f64,
    col_sub: option ColorSpec,
    font_sub: option FontFace,
    cex_lab: option f64,
    col_lab: option ColorSpec,
    font_lab: option FontFace,
    adj: option f64,
    mgp: array [f64; 3],
    line: option f64,
    outer: value bool,
});

impl<B: Backend> Device<B> {
    /// A device drawing onto `backend` with graphical parameters `par`: the
    /// general route, for any [`Backend`] and backend options that
    /// [`DeviceBuilder`] doesn't cover (e.g. PNG
    /// [`with_threads`](crate::backend::raster::RasterBackend::with_threads)). If `par.bg` is left
    /// at its default (white), it starts as the backend's background
    /// instead, as R's `par("bg")` does - transparent for PDF. The device
    /// has no file: get its output with [`finish`](Device::finish). To open
    /// a device for a file, use [`Device::pdf`], [`Device::svg`],
    /// [`Device::png`] or [`Device::format`].
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
            inner: Rect::new(0.0, 0.0, width_in, height_in),
            mar: Sides::new(0.0, 0.0, 0.0, 0.0),
            fig: Rect::new(0.0, 0.0, width_in, height_in),
            plt: Rect::new(0.0, 0.0, width_in, height_in),
            region_error: None,
            layout_cex: 1.0,
            usr: (0.0, 1.0, 0.0, 1.0),
            xlog: false,
            ylog: false,
            arrangement: Arrangement::Grid {
                nrow: 1,
                ncol: 1,
                by_col: false,
            },
            current_figure: 1,
            fig_spec: None,
            hold_figure: false,
            used: false,
            start_ps: par_ps,
            file: None,
        };
        dev.recompute_layout();
        dev
    }

    /// The backend, to finish it with its own `finish()` or set options.
    /// On a device opened for a file this doesn't write the file (it stays
    /// empty); use [`close`](Device::close) for that.
    pub fn into_backend(self) -> B {
        self.backend
    }

    /// The finished output's bytes (PDF, PNG, or SVG as UTF-8). On a device
    /// opened for a file this doesn't write the file (it stays empty); use
    /// [`close`](Device::close) for that.
    pub fn finish(self) -> Vec<u8> {
        self.backend.into_bytes()
    }

    /// Closes the device (R's `dev.off()`): finishes the output and writes
    /// it to the file created when the device was opened
    /// ([`DeviceBuilder::open`]). A device dropped without `close()`
    /// leaves that file empty. A device made with [`Device::new`] has no
    /// file: this returns an [`InvalidInput`](std::io::ErrorKind::InvalidInput)
    /// error; use [`finish`](Device::finish) for its bytes.
    pub fn close(mut self) -> std::io::Result<()> {
        let Some(mut file) = self.file.take() else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "no file to close: the device was made with Device::new; \
                 open one with Device::pdf/svg/png/format, or use finish()",
            ));
        };
        file.write_all(&self.finish())?;
        file.flush()
    }

    /// Page width in inches (R's `par("din")[1]`).
    pub fn width_in(&self) -> f64 {
        self.width_in
    }

    /// Page height in inches (R's `par("din")[2]`).
    pub fn height_in(&self) -> f64 {
        self.height_in
    }

    /// The current plot region, in device inches from the bottom-left
    /// corner of the page (R's `par("plt")`, but in inches).
    pub fn plot_region(&self) -> Rect {
        self.plt
    }

    /// R's `par("usr")`: the user coordinate limits `(x0, x1, y0, y1)` of
    /// the plot region. As in R, a log axis's limits are log10 values
    /// (`par("usr")` of `plot(1:100, log = "x")` starts at about 0, not
    /// 1); everything that takes coordinates (`points()`, `axis(at = )`,
    /// [`Device::user_to_inches`], ...) takes data units instead.
    pub fn usr(&self) -> (f64, f64, f64, f64) {
        self.usr
    }

    /// Whether the x axis is logarithmic (R's `par("xlog")`), as set by
    /// the last `plot_window()` (`plot_new()` resets it).
    pub fn xlog(&self) -> bool {
        self.xlog
    }

    /// Whether the y axis is logarithmic (R's `par("ylog")`); see
    /// [`Device::xlog`].
    pub fn ylog(&self) -> bool {
        self.ylog
    }

    fn recompute_layout(&mut self) {
        self.layout_cex = self.par.cex;
        // Inner region: the device minus the outer margins; figures are
        // fractions (NIC) of it.
        let omi = self.omi();
        let inner = Rect::new(
            omi.left,
            omi.bottom,
            self.width_in - omi.right,
            self.height_in - omi.top,
        );
        self.inner = inner;
        // Margins in lines, for `title()` (R's `updateFigureMargins()`
        // converts `mai` to `mar`).
        let lh = self.line_height_inches();
        self.mar = match self.par.mai {
            Some(m) => Sides::new(m.bottom / lh, m.left / lh, m.top / lh, m.right / lh),
            None => self.par.mar,
        };
        let [fx0, fx1, fy0, fy1] = match self.fig_spec {
            Some(FigSpec::Nic(fig)) => fig,
            // `updateFigureRegion()`: `fin` centred in the inner region.
            Some(FigSpec::Inches([w, h])) => {
                let (nw, nh) = (w / inner.width(), h / inner.height());
                [
                    0.5 - nw / 2.0,
                    0.5 + nw / 2.0,
                    0.5 - nh / 2.0,
                    0.5 + nh / 2.0,
                ]
            }
            None => self.arrangement.figure_region_nic(
                self.current_figure,
                inner.width(),
                inner.height(),
            ),
        };
        self.fig = Rect::new(
            inner.x0 + fx0 * inner.width(),
            inner.y0 + fy0 * inner.height(),
            inner.x0 + fx1 * inner.width(),
            inner.y0 + fy1 * inner.height(),
        );
        // Corners as computed, before `Rect::new` sorts them, so that
        // `region_error()` sees margins that cross.
        let [x0, x1, y0, y1] = match (self.par.pin, self.par.plt) {
            // `par(pin)`: that size in inches, centred in the figure
            // (`updatePlotRegion()`).
            (Some([w, h]), _) => {
                let (cx, cy) = (
                    0.5 * (self.fig.x0 + self.fig.x1),
                    0.5 * (self.fig.y0 + self.fig.y1),
                );
                [cx - w / 2.0, cx + w / 2.0, cy - h / 2.0, cy + h / 2.0]
            }
            // `par(plt)`: fractions of the figure region.
            (None, Some([px0, px1, py0, py1])) => [
                self.fig.x0 + px0 * self.fig.width(),
                self.fig.x0 + px1 * self.fig.width(),
                self.fig.y0 + py0 * self.fig.height(),
                self.fig.y0 + py1 * self.fig.height(),
            ],
            // `mapPlotRegion()`: the figure less `mar`, made square about
            // its centre for `pty = "s"`.
            (None, None) => {
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
                [x0, x1, y0, y1]
            }
        };
        self.plt = Rect::new(x0, y0, x1, y1);
        // The plot region as fractions of the figure (R's NFC `plt`).
        let (fw, fh) = (self.fig.width(), self.fig.height());
        let plt_nfc = [
            (x0 - self.fig.x0) / fw,
            (x1 - self.fig.x0) / fw,
            (y0 - self.fig.y0) / fh,
            (y1 - self.fig.y0) / fh,
        ];
        self.region_error = region_error([fx0, fx1, fy0, fy1], plt_nfc);
    }

    /// `par(fig = )`; see [`ParCall::fig`](crate::graphics::ParCall::fig).
    pub(crate) fn fig(&mut self, fig: [f64; 4]) {
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
        self.fig_spec = Some(FigSpec::Nic(fig));
        self.recompute_layout();
    }

    /// R's `currentFigure = lastFigure`: the next `plot_new()` starts a
    /// new page (for `par(oma = )` and co.).
    pub(crate) fn eject_page(&mut self) {
        self.current_figure = self.arrangement.last_figure();
    }

    /// `par(fin = )` (R's `fUnits = INCHES`); see
    /// [`ParCall::fin`](crate::graphics::ParCall::fin).
    pub(crate) fn fin(&mut self, fin: [f64; 2]) {
        assert!(
            fin.iter().all(|&v| v > 0.0 && v.is_finite()),
            "invalid value specified for graphical parameter \"fin\""
        );
        self.arrangement = Arrangement::Grid {
            nrow: 1,
            ncol: 1,
            by_col: false,
        };
        self.current_figure = 1;
        self.fig_spec = Some(FigSpec::Inches(fin));
        self.recompute_layout();
    }

    /// `par(mfrow = )`; see
    /// [`ParCall::mfrow`](crate::graphics::ParCall::mfrow).
    pub(crate) fn mfrow(&mut self, nrow: usize, ncol: usize) {
        self.set_arrangement(Arrangement::Grid {
            nrow,
            ncol,
            by_col: false,
        });
    }

    /// `par(mfcol = )`; see
    /// [`ParCall::mfcol`](crate::graphics::ParCall::mfcol).
    pub(crate) fn mfcol(&mut self, nrow: usize, ncol: usize) {
        self.set_arrangement(Arrangement::Grid {
            nrow,
            ncol,
            by_col: true,
        });
    }

    /// `layout(mat, widths, heights, respect)`: see [`Layout`]. Sets
    /// `par(cex)`/`mex` like `par(mfrow = )`
    /// ([`ParCall::mfrow`](crate::graphics::ParCall::mfrow)) does, based on
    /// the layout matrix's dimensions, and replaces `mfrow`/`mfcol` and
    /// `fig`/`fin`.
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
        self.fig_spec = None;
        self.hold_figure = false;
        self.recompute_layout();
    }

    /// `par(mfg = )`; see [`ParCall::mfg`](crate::graphics::ParCall::mfg).
    pub(crate) fn mfg(&mut self, row: usize, col: usize) {
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

    /// `par(new = )`; see [`ParOptions::new`](crate::par::ParOptions::new).
    pub(crate) fn set_new(&mut self, new: bool) {
        if self.used {
            self.hold_figure = new;
        }
    }

    /// Whether anything has been plotted yet.
    pub(crate) fn is_used(&self) -> bool {
        self.used
    }

    /// R's `par()` values as they are now (R's `par("cex")` is
    /// `dev.current_par().cex`). Change them with
    /// [`par()`](crate::graphics::Graphics::par).
    pub fn current_par(&self) -> &Par {
        &self.par
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

    /// Draws R's `plot_new()`; the public entry is
    /// [`Graphics::plot_new`](crate::graphics::Graphics::plot_new), whose docs
    /// describe it.
    pub(crate) fn plot_new_impl(&mut self) {
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
        if let Some(msg) = self.region_error {
            // R's `invalidError()`: back to the previous figure, so the
            // next `plot_new()` retries this one.
            self.current_figure -= 1;
            if self.current_figure < 1 {
                self.current_figure = self.arrangement.last_figure();
            }
            panic!("{msg}");
        }
        // `C_plot_new`: linear axes, `GScale(0, 1)` on both.
        self.set_window((0.0, 1.0), (0.0, 1.0), false, false);
    }

    /// The user coordinates for limits `xlim`/`ylim` (data units) on
    /// linear or log axes, through `GScale()`, and `par.xaxp`/`yaxp`; see
    /// [`Graphics::plot_window`](crate::graphics::Graphics::plot_window).
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
        let (mut min, mut max) = if log {
            (min.log10(), max.log10())
        } else {
            (min, max)
        };
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
                if v.is_finite() {
                    v
                } else if v < 0.0 {
                    -f64::MAX
                } else {
                    f64::MAX
                }
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
    pub fn default_ticks(&self, side: impl Into<Side>) -> Vec<f64> {
        let side = side.into();
        let axp = if side.is_horizontal() {
            self.par.xaxp
        } else {
            self.par.yaxp
        };
        self.ticks_from_axp(side, axp)
    }

    /// `CreateAtVector()` for `side`'s axis with the given `axp`.
    fn ticks_from_axp(&self, side: Side, axp: [f64; 3]) -> Vec<f64> {
        let (lo, hi, log, nint) = if side.is_horizontal() {
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
    pub fn user_to_inches(&self, x: impl Into<f64>, y: impl Into<f64>) -> Point {
        let (x, y) = (x.into(), y.into());
        let ux = if self.xlog { x.log10() } else { x };
        let uy = if self.ylog { y.log10() } else { y };
        self.usr_to_inches(ux, uy)
    }

    /// Map a point in device inches to data units (not log10 on a log
    /// axis): the inverse of [`Device::user_to_inches`] (R's
    /// `grconvertX(x, "inches", "user")`, but measured from the page's
    /// bottom-left corner, and in data units on log axes).
    pub fn inches_to_user(&self, p: Point) -> (f64, f64) {
        let (ux, uy) = self.inches_to_usr(p);
        let x = if self.xlog { 10f64.powf(ux) } else { ux };
        let y = if self.ylog { 10f64.powf(uy) } else { uy };
        (x, y)
    }

    /// Map a point in device inches to `usr` units (log10 on a log axis).
    pub(crate) fn inches_to_usr(&self, p: Point) -> (f64, f64) {
        let (x0, x1, y0, y1) = self.usr;
        let ux = x0 + (p.x - self.plt.x0) / self.plt.width() * (x1 - x0);
        let uy = y0 + (p.y - self.plt.y0) / self.plt.height() * (y1 - y0);
        (ux, uy)
    }

    /// Map a point in `usr` units (log10 on a log axis) to device inches.
    fn usr_to_inches(&self, ux: f64, uy: f64) -> Point {
        let (x0, x1, y0, y1) = self.usr;
        let px = self.plt.x0 + (ux - x0) / (x1 - x0) * self.plt.width();
        let py = self.plt.y0 + (uy - y0) / (y1 - y0) * self.plt.height();
        Point::new(px, py)
    }

    /// R's `getxlimits()`/`getylimits()` in `usr` units: the `usr` range
    /// for `xpd = FALSE`, else the figure region's (`TRUE`) or the
    /// device's (`NA`) extent along the axis.
    fn xpd_limits(&self, x_axis: bool, xpd: Xpd) -> (f64, f64) {
        let (u0, u1, p0, p1, f0, f1, d1) = if x_axis {
            let (u0, u1, ..) = self.usr;
            (
                u0,
                u1,
                self.plt.x0,
                self.plt.x1,
                self.fig.x0,
                self.fig.x1,
                self.width_in,
            )
        } else {
            let (.., u0, u1) = self.usr;
            (
                u0,
                u1,
                self.plt.y0,
                self.plt.y1,
                self.fig.y0,
                self.fig.y1,
                self.height_in,
            )
        };
        let to_usr = |d: f64| u0 + (d - p0) / (p1 - p0) * (u1 - u0);
        match xpd {
            Xpd::Plot => (u0, u1),
            Xpd::Figure => (to_usr(f0), to_usr(f1)),
            Xpd::Device => (to_usr(0.0), to_usr(d1)),
        }
    }

    /// The device's starting point size (R's `startps`): the
    /// [`pointsize`](DeviceBuilder::pointsize) it was opened with, or
    /// `par.ps` as passed to [`Device::new`]. Margin lines, plotting symbols and
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

    /// `par("mai")`: the margins in inches (bottom, left, top, right):
    /// `par.mai` if set, else `par.mar` converted.
    pub fn mai(&self) -> Sides<f64> {
        if let Some(mai) = self.par.mai {
            return mai;
        }
        let lh = self.line_height_inches();
        let m = self.par.mar;
        Sides::new(m.bottom * lh, m.left * lh, m.top * lh, m.right * lh)
    }

    /// `par("omi")`: the outer margins in inches (bottom, left, top,
    /// right), from `par.omd`, else `par.omi`, else `par.oma`.
    pub fn omi(&self) -> Sides<f64> {
        if let Some([x0, x1, y0, y1]) = self.par.omd {
            let (w, h) = (self.width_in, self.height_in);
            return Sides::new(y0 * h, x0 * w, (1.0 - y1) * h, (1.0 - x1) * w);
        }
        if let Some(omi) = self.par.omi {
            return omi;
        }
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
    // Clipping, as R's `xpd`: FALSE clips to the plot region, TRUE to the
    // figure region, NA to the device (`axis()` and `mtext(outer = TRUE)`
    // always use the device, as R).
    // ---------------------------------------------------------------

    fn push_clip_for(&mut self, xpd: Xpd) {
        self.push_clip_region(xpd);
    }

    /// Marks the current plot as drawn on, which resets `par(new)` (R's
    /// `GMode()`). Every drawing path goes through here, mostly via
    /// [`Device::push_clip_region`].
    fn dirty(&mut self) {
        self.hold_figure = false;
    }

    fn push_clip_region(&mut self, clip: Xpd) {
        self.dirty();
        let rect = match clip {
            Xpd::Plot => self.plt,
            Xpd::Figure => self.fig,
            Xpd::Device => Rect::new(0.0, 0.0, self.width_in, self.height_in),
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
            // `GSymbol()`: `par(lend/ljoin/lmitre)`, always a solid line.
            // Built as the constant default stroke unless `par()` differs,
            // which keeps the per-point path as fast as before.
            let mut stroke = Stroke::simple(col, lwd);
            let (lend, ljoin, lmitre) = (self.par.lend, self.par.ljoin, self.par.lmitre);
            if (lend, ljoin, lmitre) != (stroke.lend, stroke.ljoin, stroke.lmitre) {
                stroke.lend = lend;
                stroke.ljoin = ljoin;
                stroke.lmitre = lmitre;
            }
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
        } else if (32..=126).contains(&pch) || pch <= -32 {
            // `GEText(x, y, str, hadj = NA, yc = NA)`: centred on the
            // advance width, and vertically on the glyph's own bounding box.
            // A negative `pch` is minus a Unicode code point, as in R.
            let code = pch.unsigned_abs();
            let ch = char::from_u32(code).unwrap_or('?').to_string();
            let face = self.par.font;
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
        clip: Xpd,
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
        clip: Xpd,
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

    /// `type = "h"`: a vertical line from each point to y = 0 (on a log
    /// y axis, to the bottom of the plot), colours recycled per point and
    /// lines in a missing or transparent colour skipped. A port of
    /// `C_plotXY`'s `case 'h'`.
    fn histogram_xy(
        &mut self,
        xs: &[f64],
        ys: &[f64],
        col: &[ColorSpec],
        lwd: f64,
        lty: LineType,
        clip: Xpd,
    ) {
        let default = [ColorSpec::Color(self.par.col)];
        let col = if col.is_empty() { &default[..] } else { col };
        // R keeps `usr` in data units internally, so on a log axis its
        // lower y limit is the bottom of the plot.
        let (ux0, _, uy0, _) = self.usr;
        let base = if self.ylog {
            self.usr_to_inches(ux0, uy0).y
        } else {
            self.usr_to_inches(ux0, 0.0).y
        };
        self.push_clip_region(clip);
        for (i, (&x, &y)) in xs.iter().zip(ys).enumerate() {
            let p = self.user_to_inches(x, y);
            if !p.x.is_finite() || !p.y.is_finite() {
                continue;
            }
            let Some(c) = self.resolve(col[i % col.len()]) else {
                continue;
            };
            let stroke = self.stroke_of(c, lwd, lty);
            self.backend.draw_line(Point::new(p.x, base), p, &stroke);
        }
        self.pop_clip();
    }

    /// `type = "s"` (`vertical_first` false: across, then up or down) and
    /// `"S"` (up or down, then across): a staircase through the points,
    /// one polyline per run of finite points. A port of `C_plotXY`'s
    /// `case 's'` and `case 'S'`.
    #[allow(clippy::too_many_arguments)]
    fn steps_xy(
        &mut self,
        xs: &[f64],
        ys: &[f64],
        vertical_first: bool,
        col: ColorSpec,
        lwd: f64,
        lty: LineType,
        clip: Xpd,
    ) {
        let Some(col) = self.resolve(col) else {
            return;
        };
        let stroke = self.stroke_of(col, lwd, lty);
        self.push_clip_region(clip);
        let mut run: Vec<Point> = Vec::new();
        let mut old: Option<Point> = None;
        for (&x, &y) in xs.iter().zip(ys) {
            let p = self.user_to_inches(x, y);
            let p = (p.x.is_finite() && p.y.is_finite()).then_some(p);
            match (old, p) {
                (Some(a), Some(b)) => {
                    if run.is_empty() {
                        run.push(a);
                    }
                    run.push(if vertical_first {
                        Point::new(a.x, b.y)
                    } else {
                        Point::new(b.x, a.y)
                    });
                    run.push(b);
                }
                (_, None) if !run.is_empty() => {
                    self.backend.draw_polyline(&run, &stroke);
                    run.clear();
                }
                _ => {}
            }
            old = p;
        }
        if !run.is_empty() {
            self.backend.draw_polyline(&run, &stroke);
        }
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
    /// (`par(lheight) * 1.2 * ps / 72` inches, times the cex) per `\n`.
    pub fn text_metrics(
        &self,
        text: &str,
        font: impl Into<FontFace>,
        cex: impl Into<f64>,
    ) -> (f64, f64) {
        let cex = cex.into();
        let face = font.into();
        let size = self.font_size_pt(cex);
        let width = text
            .split('\n')
            .map(|line| afm::str_width_inches(line, face, size, 1.0))
            .fold(0.0, f64::max);
        let newlines = text.matches('\n').count() as f64;
        let line = self.par.lheight * 1.2 * self.par.ps * self.par.cex * cex / 72.0;
        (
            width,
            newlines * line + afm::m_height_inches(face, size, 1.0),
        )
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
        clip: Xpd,
        anchor: Point,
        label: &str,
        hadj: f64,
        vadj: Option<f64>,
        rotation_deg: f64,
        col: ColorSpec,
        cex: f64,
        font: FontFace,
    ) {
        let Some(color) = self.resolve(col) else {
            return;
        };
        if label.is_empty() {
            return;
        }
        let face = font;
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

    /// `GMtext()`'s `yLineBias` nudge (`graphics.c`), in margin lines, for
    /// text on `side`: `1 - bias` on sides 1 and 4, `bias` on 2 and 3, divided
    /// by `par(mex)` as R does (so the nudge stays the same size in inches);
    /// 0 for text perpendicular to the side. Shared by `title()`, `axis()`
    /// and `mtext()`.
    pub(crate) fn line_bias(&self, side: Side, parallel: bool) -> f64 {
        if !parallel {
            return 0.0;
        }
        let bias = match side {
            Side::Bottom | Side::Right => 1.0 - Y_LINE_BIAS,
            Side::Left | Side::Top => Y_LINE_BIAS,
        };
        bias / self.par.mex
    }

    /// Draws R's `title()`; the public entry is
    /// [`Graphics::title`](crate::graphics::Graphics::title), whose docs
    /// describe it.
    pub(crate) fn title_impl(&mut self, opts: &TitleOptions) {
        let lh = self.line_height_inches();
        // `C_title`: every title sits at `adj` along its side and is
        // justified by `adj` (`GMtext` uses `par(adj)` in all cases); along
        // the plot region (NPC), or with `outer` the inner region (NIC).
        let adj = opts.adj.unwrap_or(self.par.adj);
        let mgp = opts.mgp.unwrap_or(self.par.mgp);
        let line = opts.line.filter(|l| l.is_finite());
        let (region, clip) = if opts.outer {
            (self.inner, Xpd::Device)
        } else {
            // R raises `xpd` to at least `TRUE` (`NA` stays `NA`).
            let clip = match self.par.xpd {
                Xpd::Device => Xpd::Device,
                _ => Xpd::Figure,
            };
            (self.plt, clip)
        };
        let x_at = region.x0 + adj * region.width();
        let y_at = region.y0 + adj * region.height();
        if let Some(main) = &opts.main {
            // `GText(hpos, vpos, MAR3/OMA3, .., adj, adjy)`: at `line` with
            // the baseline on it (`adjy = 0`), else half the top margin
            // (`0.5 * mar[2]` or `oma[2]`) with `adjy = 0.5` (finite, so
            // `GEText` uses `yc * height(M)`, not exact ink centring). An
            // expression is always centred (`GMathText(.., adj, 0.5)`).
            let (y, adjy) = match line {
                Some(l) => (region.y1 + l * lh, 0.0),
                None => {
                    let margin = if opts.outer {
                        self.height_in - self.inner.y1
                    } else {
                        self.mar.top * lh
                    };
                    (region.y1 + 0.5 * margin, 0.5)
                }
            };
            let adjy = if matches!(main, Label::Math(_)) {
                0.5
            } else {
                adjy
            };
            self.draw_label(
                clip,
                Point::new(x_at, y),
                main,
                adj,
                Some(adjy),
                0.0,
                opts.col_main.unwrap_or(ColorSpec::Color(self.par.col_main)),
                opts.cex_main.unwrap_or(self.par.cex_main),
                opts.font_main.unwrap_or(self.par.font_main),
            );
        }
        // The rest via `GMtext()`: at `line`, else `mgp[0]` (+ 1 for
        // `sub`), nudged by `yLineBias` (see `line_bias`).
        if let Some(sub) = &opts.sub {
            let l = line.unwrap_or(mgp[0] + 1.0) + self.line_bias(Side::Bottom, true);
            self.draw_label(
                clip,
                Point::new(x_at, region.y0 - l * lh),
                sub,
                adj,
                Some(0.0),
                0.0,
                opts.col_sub.unwrap_or(ColorSpec::Color(self.par.col_sub)),
                opts.cex_sub.unwrap_or(self.par.cex_sub),
                opts.font_sub.unwrap_or(self.par.font_sub),
            );
        }
        if let Some(xlab) = &opts.xlab {
            let l = line.unwrap_or(mgp[0]) + self.line_bias(Side::Bottom, true);
            self.draw_label(
                clip,
                Point::new(x_at, region.y0 - l * lh),
                xlab,
                adj,
                Some(0.0),
                0.0,
                opts.col_lab.unwrap_or(ColorSpec::Color(self.par.col_lab)),
                opts.cex_lab.unwrap_or(self.par.cex_lab),
                opts.font_lab.unwrap_or(self.par.font_lab),
            );
        }
        if let Some(ylab) = &opts.ylab {
            let l = line.unwrap_or(mgp[0]) + self.line_bias(Side::Left, true);
            self.draw_label(
                clip,
                Point::new(region.x0 - l * lh, y_at),
                ylab,
                adj,
                Some(0.0),
                90.0,
                opts.col_lab.unwrap_or(ColorSpec::Color(self.par.col_lab)),
                opts.cex_lab.unwrap_or(self.par.cex_lab),
                opts.font_lab.unwrap_or(self.par.font_lab),
            );
        }
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

/// A temporary change to a device's `par`, as R's `op <- par(...);
/// on.exit(par(op))`: `restore` copies the changed fields back from the
/// saved `par` when the scope ends, also after an early return or a panic.
/// Derefs to the device.
pub(crate) struct ParScope<'a, B: Backend> {
    dev: &'a mut Device<B>,
    saved: Par,
    restore: fn(&mut Par, &Par),
}

impl<B: Backend> Device<B> {
    /// Starts a [`ParScope`]; `restore` copies back what the caller is
    /// about to change.
    pub(crate) fn par_scope(&mut self, restore: fn(&mut Par, &Par)) -> ParScope<'_, B> {
        let saved = self.par.clone();
        ParScope {
            dev: self,
            saved,
            restore,
        }
    }
}

impl<B: Backend> Device<B> {
    /// R's inline `lend`/`ljoin`/`lmitre` for one call (`ProcessInlinePars`,
    /// undone by `GRestorePars`): every line drawn in the scope uses them.
    ///
    /// # Panics
    /// If `lmitre` is below 1 (R's "invalid value specified for graphical
    /// parameter \"lmitre\"").
    pub(crate) fn line_pars(
        &mut self,
        lend: Option<LineEnd>,
        ljoin: Option<LineJoin>,
        lmitre: Option<f64>,
    ) -> ParScope<'_, B> {
        if let Some(m) = lmitre {
            assert!(
                m >= 1.0,
                "invalid value specified for graphical parameter \"lmitre\""
            );
        }
        let mut dev = self.par_scope(|now, saved| {
            now.lend = saved.lend;
            now.ljoin = saved.ljoin;
            now.lmitre = saved.lmitre;
        });
        if let Some(l) = lend {
            dev.par.lend = l;
        }
        if let Some(j) = ljoin {
            dev.par.ljoin = j;
        }
        if let Some(m) = lmitre {
            dev.par.lmitre = m;
        }
        dev
    }
}

impl<B: Backend> Deref for ParScope<'_, B> {
    type Target = Device<B>;
    fn deref(&self) -> &Device<B> {
        self.dev
    }
}

impl<B: Backend> DerefMut for ParScope<'_, B> {
    fn deref_mut(&mut self) -> &mut Device<B> {
        self.dev
    }
}

impl<B: Backend> Drop for ParScope<'_, B> {
    fn drop(&mut self) {
        (self.restore)(&mut self.dev.par, &self.saved);
    }
}

/// `GNewPlot()`'s checks (`graphics.c`), in its order: R's error message
/// when the figure region `fig` (fractions of the inner region) or the plot
/// region `plt` (fractions of the figure) is invalid. Region sizes are
/// absolute, as R's unit conversions take them, so outer margins wider
/// than the device leave a small figure whose margins don't fit ("figure
/// margins too large").
fn region_error(fig: [f64; 4], plt: [f64; 4]) -> Option<&'static str> {
    let eps = f64::from(f32::EPSILON);
    let within = |r: [f64; 4]| r[0] > -eps && r[1] < 1.0 + eps && r[2] > -eps && r[3] < 1.0 + eps;
    let ordered = |r: [f64; 4]| r[0] < r[1] && r[2] < r[3];
    if !ordered(fig) {
        Some("outer margins too large (figure region too small)")
    } else if !within(fig) {
        Some("figure region too large")
    } else if !ordered(plt) {
        Some("figure margins too large")
    } else if !within(plt) {
        Some("plot region too large")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::format_axis_labels;

    fn f(v: &[f64]) -> Vec<String> {
        format_axis_labels(v)
    }

    #[test]
    fn region_errors_follow_r_order() {
        use super::region_error;
        let ok = [0.0, 1.0, 0.0, 1.0];
        let plt = [0.2, 0.8, 0.2, 0.8];
        assert_eq!(region_error(ok, plt), None);
        // `GNewPlot()` checks the figure before the plot region, and order
        // before extent; R's absolute unit sizes make the first one hard to
        // reach from `par()`, so it's checked here.
        assert_eq!(
            region_error([0.6, 0.4, -1.0, 2.0], [0.8, 0.2, 0.0, 2.0]),
            Some("outer margins too large (figure region too small)")
        );
        assert_eq!(
            region_error([-1.0, 2.0, 0.0, 1.0], [0.8, 0.2, 0.0, 2.0]),
            Some("figure region too large")
        );
        assert_eq!(
            region_error(ok, [0.8, 0.2, 0.0, 2.0]),
            Some("figure margins too large")
        );
        assert_eq!(
            region_error(ok, [0.0, 2.0, 0.0, 1.0]),
            Some("plot region too large")
        );
        // R's FLT_EPSILON tolerance at the edges.
        assert_eq!(
            region_error([-1e-9, 1.0 + 1e-9, 0.0, 1.0], [-1e-9, 1.0, 0.0, 1.0]),
            None
        );
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

    #[test]
    fn inches_to_user_inverts_user_to_inches() {
        use super::{Device, Par, PlotWindowOptions, Side};
        use crate::backend::pdf::PdfBackend;
        let mut dev = Device::new(PdfBackend::new(7.0, 5.0), Par::default());
        dev.plot_new_impl();
        assert!(!dev.xlog() && !dev.ylog());
        let log_x = PlotWindowOptions {
            xlog: true,
            ..PlotWindowOptions::default()
        };
        dev.plot_window_impl((1.0, 1000.0), (-2.0, 3.0), &log_x);
        assert!(dev.xlog() && !dev.ylog());
        for (x, y) in [(1.0, -2.0), (31.6, 0.25), (1000.0, 3.0)] {
            let (x2, y2) = dev.inches_to_user(dev.user_to_inches(x, y));
            assert!(
                (x2 - x).abs() < 1e-9 * x && (y2 - y).abs() < 1e-9,
                "{x}, {y}"
            );
        }
        assert_eq!(Side::from(3), Side::Top);
        assert_eq!(Side::Right.number(), 4);
    }

    #[test]
    #[should_panic(expected = "invalid side 5")]
    fn side_numbers_outside_1_to_4_panic() {
        let _ = super::Side::from(5);
    }
}
