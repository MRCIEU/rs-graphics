// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! The low-level drawing functions `points()`, `lines()`, `segments()`,
//! `arrows()`, `rect()`, `polygon()`, `text()`, `box()` and
//! `plot.window()`, each taking its data and an options struct, as in R.
//!
//! An empty `Vec` (or `None`) field means R's default for that argument,
//! usually the `par()` value; a field R recycles per element (`col` in
//! `points(x, y, col = groups)`) is a `Vec` used in turn and repeated as
//! needed. `xpd` is R's inline `xpd = ` (clip to the figure region rather
//! than the plot region); `None` uses `par(xpd)`.

use super::{Device, Xpd};
use crate::backend::{Backend, LineEnd, LineJoin};
use crate::color::{Color, ColorSpec};
use crate::geometry::{Point, Rect};
use crate::highlevel::PlotType;
use crate::lty::LineType;
use crate::par::{AxisStyle, Bty};
use crate::plotmath::Label;

/// `v[i]`, recycling `v`; `default` when `v` is empty.
fn pick<T: Copy>(v: &[T], i: usize, default: T) -> T {
    if v.is_empty() {
        default
    } else {
        v[i % v.len()]
    }
}

/// The number of elements after R's recycling of `lens`: the longest, or
/// 0 when any is empty.
fn recycled_len(lens: &[usize]) -> usize {
    if lens.contains(&0) {
        0
    } else {
        lens.iter().copied().max().unwrap_or(0)
    }
}

/// Arguments of `points()` and `lines()`, both R's `plot.xy()`.
#[derive(Clone, Debug, Default)]
pub struct XyOptions {
    /// R's `type` (see [`PlotType`]): `None` is `"p"` for `points()` and
    /// `"l"` for `lines()`.
    pub type_: Option<PlotType>,
    /// Plotting symbols, recycled per point; empty: `par(pch)`.
    pub pch: Vec<i32>,
    /// Colours, recycled per point (lines use the first, except
    /// `type = "h"`'s, one per point); empty: `par(col)`.
    pub col: Vec<ColorSpec>,
    /// Fill of symbols 21-25, recycled per point; empty: none (R's `NA`).
    pub bg: Vec<ColorSpec>,
    /// Symbol sizes, recycled per point, times `par(cex)`; empty: 1.
    pub cex: Vec<f64>,
    /// Line widths, recycled per point (lines use the first); empty:
    /// `par(lwd)`.
    pub lwd: Vec<f64>,
    /// Line type of lines; `None`: `par(lty)`.
    pub lty: Option<LineType>,
    /// R's inline `xpd`: where to clip ([`Xpd::Plot`], [`Xpd::Figure`] or
    /// [`Xpd::Device`]; the setter also takes `false`/`true`); `None`:
    /// `par(xpd)`.
    pub xpd: Option<Xpd>,
    /// R's inline `lend`: line ends (`"round"`, `"butt"`, `"square"`);
    /// `None`: `par(lend)`.
    pub lend: Option<LineEnd>,
    /// R's inline `ljoin`: line joins (`"round"`, `"mitre"`, `"bevel"`);
    /// `None`: `par(ljoin)`.
    pub ljoin: Option<LineJoin>,
    /// R's inline `lmitre`: the mitre limit (at least 1); `None`:
    /// `par(lmitre)`.
    pub lmitre: Option<f64>,
}

crate::setters::setters!(XyOptions => [PointsCall, LinesCall] {
    type_: option PlotType,
    pch: values i32,
    col: values ColorSpec,
    bg: values ColorSpec,
    cex: values f64,
    lwd: values f64,
    lty: option LineType,
    xpd: option Xpd,
    lend: option LineEnd,
    ljoin: option LineJoin,
    lmitre: option f64,
});

/// Arguments of `segments()`.
#[derive(Clone, Debug, Default)]
pub struct SegmentsOptions {
    /// Recycled per segment; empty: `par(fg)` (R's default).
    pub col: Vec<ColorSpec>,
    /// Recycled per segment; empty: `par(lty)`.
    pub lty: Vec<LineType>,
    /// Recycled per segment; empty: `par(lwd)`.
    pub lwd: Vec<f64>,
    /// R's inline `lend`: how the segments end (`"round"`, `"butt"`,
    /// `"square"`); `None`: `par(lend)`.
    pub lend: Option<LineEnd>,
    /// R's inline `xpd`: where to clip ([`Xpd::Plot`], [`Xpd::Figure`] or
    /// [`Xpd::Device`]; the setter also takes `false`/`true`); `None`:
    /// `par(xpd)`.
    pub xpd: Option<Xpd>,
    /// R's inline `ljoin`: line joins (`"round"`, `"mitre"`, `"bevel"`);
    /// `None`: `par(ljoin)`.
    pub ljoin: Option<LineJoin>,
    /// R's inline `lmitre`: the mitre limit (at least 1); `None`:
    /// `par(lmitre)`.
    pub lmitre: Option<f64>,
}

crate::setters::setters!(SegmentsOptions => [SegmentsCall] {
    col: values ColorSpec,
    lty: values LineType,
    lwd: values f64,
    lend: option LineEnd,
    xpd: option Xpd,
    ljoin: option LineJoin,
    lmitre: option f64,
});

/// Arguments of `arrows()`.
#[derive(Clone, Debug, Default)]
pub struct ArrowsOptions {
    /// Length of the head's edges in inches; `None`: 0.25 (R's default).
    pub length: Option<f64>,
    /// Angle between the shaft and each edge of the head, in degrees (90
    /// gives flat caps, as for error bars); `None`: 30 (R's default).
    pub angle: Option<f64>,
    /// Heads at the start (1), the end (2) or both ends (3); `None`: 2
    /// (R's default).
    pub code: Option<u8>,
    /// Recycled per arrow; empty: `par(fg)` (R's default).
    pub col: Vec<ColorSpec>,
    /// Recycled per arrow; empty: `par(lty)`.
    pub lty: Vec<LineType>,
    /// Recycled per arrow; empty: `par(lwd)`.
    pub lwd: Vec<f64>,
    /// R's inline `xpd`: where to clip ([`Xpd::Plot`], [`Xpd::Figure`] or
    /// [`Xpd::Device`]; the setter also takes `false`/`true`); `None`:
    /// `par(xpd)`.
    pub xpd: Option<Xpd>,
    /// R's inline `lend`: line ends (`"round"`, `"butt"`, `"square"`);
    /// `None`: `par(lend)`.
    pub lend: Option<LineEnd>,
    /// R's inline `ljoin`: line joins (`"round"`, `"mitre"`, `"bevel"`);
    /// `None`: `par(ljoin)`.
    pub ljoin: Option<LineJoin>,
    /// R's inline `lmitre`: the mitre limit (at least 1); `None`:
    /// `par(lmitre)`.
    pub lmitre: Option<f64>,
}

crate::setters::setters!(ArrowsOptions => [ArrowsCall] {
    length: option f64,
    angle: option f64,
    code: option u8,
    col: values ColorSpec,
    lty: values LineType,
    lwd: values f64,
    xpd: option Xpd,
    lend: option LineEnd,
    ljoin: option LineJoin,
    lmitre: option f64,
});

/// Arguments of `rect()`.
#[derive(Clone, Debug, Default)]
pub struct RectOptions {
    /// Fill colours, recycled per rectangle; empty: no fill (R's `NA`).
    pub col: Vec<ColorSpec>,
    /// Border colours, recycled; empty: `par(fg)`. `ColorSpec::None`
    /// draws no border.
    pub border: Vec<ColorSpec>,
    /// Recycled per rectangle; empty: `par(lty)`.
    pub lty: Vec<LineType>,
    /// Recycled per rectangle; empty: `par(lwd)`.
    pub lwd: Vec<f64>,
    /// R's inline `xpd`: where to clip ([`Xpd::Plot`], [`Xpd::Figure`] or
    /// [`Xpd::Device`]; the setter also takes `false`/`true`); `None`:
    /// `par(xpd)`.
    pub xpd: Option<Xpd>,
    /// R's inline `lend`: line ends (`"round"`, `"butt"`, `"square"`);
    /// `None`: `par(lend)`.
    pub lend: Option<LineEnd>,
    /// R's inline `ljoin`: line joins (`"round"`, `"mitre"`, `"bevel"`);
    /// `None`: `par(ljoin)`.
    pub ljoin: Option<LineJoin>,
    /// R's inline `lmitre`: the mitre limit (at least 1); `None`:
    /// `par(lmitre)`.
    pub lmitre: Option<f64>,
}

crate::setters::setters!(RectOptions => [RectCall] {
    col: values ColorSpec,
    border: values ColorSpec,
    lty: values LineType,
    lwd: values f64,
    xpd: option Xpd,
    lend: option LineEnd,
    ljoin: option LineJoin,
    lmitre: option f64,
});

/// Arguments of `polygon()`. A non-finite coordinate (R's `NA`) ends one
/// polygon and starts the next; `col`, `border` and `lty` are recycled
/// over those polygons.
#[derive(Clone, Debug, Default)]
pub struct PolygonOptions {
    /// Fill colours; empty: no fill (R's `NA`).
    pub col: Vec<ColorSpec>,
    /// Border colours; empty: `par(fg)`. `ColorSpec::None` draws no
    /// border.
    pub border: Vec<ColorSpec>,
    /// Empty: `par(lty)`.
    pub lty: Vec<LineType>,
    /// One width for all; `None`: `par(lwd)`.
    pub lwd: Option<f64>,
    /// R's inline `xpd`: where to clip ([`Xpd::Plot`], [`Xpd::Figure`] or
    /// [`Xpd::Device`]; the setter also takes `false`/`true`); `None`:
    /// `par(xpd)`.
    pub xpd: Option<Xpd>,
    /// R's inline `lend`: line ends (`"round"`, `"butt"`, `"square"`);
    /// `None`: `par(lend)`.
    pub lend: Option<LineEnd>,
    /// R's inline `ljoin`: line joins (`"round"`, `"mitre"`, `"bevel"`);
    /// `None`: `par(ljoin)`.
    pub ljoin: Option<LineJoin>,
    /// R's inline `lmitre`: the mitre limit (at least 1); `None`:
    /// `par(lmitre)`.
    pub lmitre: Option<f64>,
}

crate::setters::setters!(PolygonOptions => [PolygonCall] {
    col: values ColorSpec,
    border: values ColorSpec,
    lty: values LineType,
    lwd: option f64,
    xpd: option Xpd,
    lend: option LineEnd,
    ljoin: option LineJoin,
    lmitre: option f64,
});

/// Arguments of `text()`.
#[derive(Clone, Debug, Default)]
pub struct TextOptions {
    /// Horizontal adjustment, R's `adj[1]` (0 left, 0.5 centred, 1 right);
    /// `None`: `par(adj)`.
    pub adj: Option<f64>,
    /// Vertical adjustment, R's `adj[2]`: a finite value puts a text's
    /// baseline `vadj * height("M")` below the point, or puts the point
    /// that fraction of an expression's box height above the box's bottom;
    /// `None` (R's `NA`, the default) centres the text's ink, or an
    /// expression's box, on it.
    pub vadj: Option<f64>,
    /// Rotation in degrees, counterclockwise (R's `srt`); `None`:
    /// `par(srt)` (0 by default).
    pub srt: Option<f64>,
    /// Recycled per label, times `par(cex)`; empty: 1.
    pub cex: Vec<f64>,
    /// Recycled per label; empty: `par(col)`.
    pub col: Vec<ColorSpec>,
    /// Recycled per label (1 plain, 2 bold, 3 italic, 4 bold italic, 5
    /// Symbol); empty: `par(font)`.
    pub font: Vec<i32>,
    /// R's inline `xpd`: where to clip ([`Xpd::Plot`], [`Xpd::Figure`] or
    /// [`Xpd::Device`]; the setter also takes `false`/`true`); `None`:
    /// `par(xpd)`.
    pub xpd: Option<Xpd>,
    /// R's `pos`, recycled per label: put the label below (1), to the left
    /// of (2), above (3) or to the right of (4) its point, `offset` away;
    /// overrides `adj`/`vadj`. Empty: none.
    pub pos: Vec<i32>,
    /// With `pos`, the distance from the point in character widths (R's
    /// `offset`); `None`: 0.5.
    pub offset: Option<f64>,
}

crate::setters::setters!(TextOptions => [TextCall] {
    adj: option f64,
    vadj: option f64,
    srt: option f64,
    cex: values f64,
    col: values ColorSpec,
    font: values i32,
    xpd: option Xpd,
    pos: values i32,
    offset: option f64,
});

/// The device's `yCharOffset`: how far up the text's height to put the
/// point for `text(pos = 2 or 4)` (0.3333 on R's pdf, cairo and Quartz
/// devices).
const Y_CHAR_OFFSET: f64 = 0.3333;

/// Which region `box()` outlines.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BoxWhich {
    /// The plot region (R's default).
    #[default]
    Plot,
    /// The current figure region.
    Figure,
    /// The inner region: the device minus the outer margins.
    Inner,
    /// The whole device.
    Outer,
}

/// Arguments of `box()`.
#[derive(Clone, Debug, Default)]
pub struct BoxOptions {
    /// Which region to outline; `None`: the plot region (R's default).
    pub which: Option<BoxWhich>,
    /// `None`: `par(col)` (R's `C_box`; R's `par(fg = )` sets `col` too,
    /// so set both `par.fg` and `par.col` to follow it).
    pub col: Option<ColorSpec>,
    /// `None`: solid (R's `box(lty = "solid")`, not `par(lty)`).
    pub lty: Option<LineType>,
    /// `None`: `par(lwd)`.
    pub lwd: Option<f64>,
    /// Which sides of the plot region to draw (R's inline `bty`); `None`:
    /// `par(bty)`. Ignored for the other regions, which always get all
    /// four sides (R's `GBox()`).
    pub bty: Option<Bty>,
    /// R's inline `lend`: line ends (`"round"`, `"butt"`, `"square"`);
    /// `None`: `par(lend)`.
    pub lend: Option<LineEnd>,
    /// R's inline `ljoin`: line joins (`"round"`, `"mitre"`, `"bevel"`);
    /// `None`: `par(ljoin)`.
    pub ljoin: Option<LineJoin>,
    /// R's inline `lmitre`: the mitre limit (at least 1); `None`:
    /// `par(lmitre)`.
    pub lmitre: Option<f64>,
}

crate::setters::setters!(BoxOptions => [BoxCall] {
    which: option BoxWhich,
    col: option ColorSpec,
    lty: option LineType,
    lwd: option f64,
    bty: option Bty,
    lend: option LineEnd,
    ljoin: option LineJoin,
    lmitre: option f64,
});

/// Arguments of `plot.window()` besides the limits.
#[derive(Clone, Debug, Default)]
pub struct PlotWindowOptions {
    /// Log x axis (R's `log = "x"`); default `false`.
    pub xlog: bool,
    /// Log y axis (R's `log = "y"`); default `false`.
    pub ylog: bool,
    /// R's `asp`: one x unit is `asp` y units long on the page. Ignored on
    /// log axes, as in R. `None` (R's `NA`): no constraint.
    pub asp: Option<f64>,
    /// x axis style for this window (R's inline `xaxs`: `"r"` adds 4% at
    /// each end, `"i"` none); `None`: `par(xaxs)`.
    pub xaxs: Option<AxisStyle>,
    /// As `xaxs`, for the y axis; `None`: `par(yaxs)`.
    pub yaxs: Option<AxisStyle>,
    /// Approximate numbers of x and y ticks (and label length) for the
    /// default ticks this window sets (R's inline `lab`); `None`:
    /// `par(lab)`.
    pub lab: Option<[i32; 3]>,
}

crate::setters::setters!(PlotWindowOptions => [PlotWindowCall] {
    xlog: value bool,
    ylog: value bool,
    asp: option f64,
    xaxs: option AxisStyle,
    yaxs: option AxisStyle,
    lab: option [i32; 3],
});

impl<B: Backend> Device<B> {
    fn clip_for(&self, xpd: Option<Xpd>) -> Xpd {
        xpd.unwrap_or(self.par.xpd)
    }

    /// Draws R's `plot_window()`; the public entry is
    /// [`Graphics::plot_window`](crate::graphics::Graphics::plot_window), whose docs
    /// describe it.
    pub(crate) fn plot_window_impl(
        &mut self,
        xlim: (f64, f64),
        ylim: (f64, f64),
        opts: &PlotWindowOptions,
    ) {
        // R's inline `xaxs`/`yaxs`/`lab` hold for this call only; the
        // `xaxp`/`yaxp` it sets stay.
        let mut dev = self.par_scope(|now, saved| {
            now.xaxs = saved.xaxs;
            now.yaxs = saved.yaxs;
            now.lab = saved.lab;
        });
        if let Some(x) = opts.xaxs {
            dev.par.xaxs = x;
        }
        if let Some(y) = opts.yaxs {
            dev.par.yaxs = y;
        }
        if let Some(lab) = opts.lab {
            dev.par.lab = lab;
        }
        dev.plot_window_draw(xlim, ylim, opts);
    }

    fn plot_window_draw(&mut self, xlim: (f64, f64), ylim: (f64, f64), opts: &PlotWindowOptions) {
        let asp = opts.asp.filter(|a| a.is_finite() && *a > 0.0);
        let (Some(asp), false, false) = (asp, opts.xlog, opts.ylog) else {
            self.set_window(xlim, ylim, opts.xlog, opts.ylog);
            return;
        };
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
        self.set_window(
            (xlim.0 - xadd, xlim.1 + xadd),
            (ylim.0 - yadd, ylim.1 + yadd),
            false,
            false,
        );
    }

    /// Draws R's `points()`; the public entry is
    /// [`Graphics::points`](crate::graphics::Graphics::points), whose docs
    /// describe it.
    pub(crate) fn points_impl(&mut self, x: &[f64], y: &[f64], opts: &XyOptions) {
        let mut dev = self.line_pars(opts.lend, opts.ljoin, opts.lmitre);
        dev.plot_xy(x, y, opts.type_.unwrap_or(PlotType::Points), opts);
    }

    /// Draws R's `lines()`; the public entry is
    /// [`Graphics::lines`](crate::graphics::Graphics::lines), whose docs
    /// describe it.
    pub(crate) fn lines_impl(&mut self, x: &[f64], y: &[f64], opts: &XyOptions) {
        let mut dev = self.line_pars(opts.lend, opts.ljoin, opts.lmitre);
        dev.plot_xy(x, y, opts.type_.unwrap_or(PlotType::Lines), opts);
    }

    /// `C_plotXY`: the lines for `type`, then the points for `"p"`, `"b"`
    /// and `"o"`.
    fn plot_xy(&mut self, x: &[f64], y: &[f64], type_: PlotType, opts: &XyOptions) {
        let clip = self.clip_for(opts.xpd);
        let line = || {
            let col = pick(&opts.col, 0, ColorSpec::Color(self.par.col));
            let lwd = pick(&opts.lwd, 0, self.par.lwd);
            (col, lwd, opts.lty.unwrap_or(self.par.lty))
        };
        match type_ {
            PlotType::Points => self.xy_points(x, y, opts, clip),
            PlotType::Lines => {
                let (col, lwd, lty) = line();
                self.polyline_xy(x, y, col, lwd, lty, clip);
            }
            PlotType::BrokenLines => {
                let (col, lwd, lty) = line();
                self.broken_lines_xy(x, y, col, lwd, lty, clip);
            }
            PlotType::Both => {
                let (col, lwd, lty) = line();
                self.broken_lines_xy(x, y, col, lwd, lty, clip);
                self.xy_points(x, y, opts, clip);
            }
            PlotType::Overplotted => {
                let (col, lwd, lty) = line();
                self.polyline_xy(x, y, col, lwd, lty, clip);
                self.xy_points(x, y, opts, clip);
            }
            PlotType::Histogram => {
                let (_, lwd, lty) = line();
                self.histogram_xy(x, y, &opts.col, lwd, lty, clip);
            }
            PlotType::Steps | PlotType::StepsVerticalFirst => {
                let (col, lwd, lty) = line();
                let vertical_first = type_ == PlotType::StepsVerticalFirst;
                self.steps_xy(x, y, vertical_first, col, lwd, lty, clip);
            }
            PlotType::None => {}
        }
    }

    fn xy_points(&mut self, x: &[f64], y: &[f64], opts: &XyOptions, clip: Xpd) {
        let col: Vec<Option<Color>> = if opts.col.is_empty() {
            vec![self.resolve(ColorSpec::Color(self.par.col))]
        } else {
            opts.col.iter().map(|&c| self.resolve(c)).collect()
        };
        let bg: Vec<Option<Color>> = opts.bg.iter().map(|&c| self.resolve(c)).collect();
        let (pch, lwd) = (self.par.pch, self.par.lwd);
        self.push_clip_region(clip);
        for (i, (&x, &y)) in x.iter().zip(y).enumerate() {
            // Checked after conversion, like R, so values <= 0 on a log
            // axis are skipped too.
            let p = self.user_to_inches(x, y);
            if !p.x.is_finite() || !p.y.is_finite() {
                continue;
            }
            let Some(c) = col[i % col.len()] else {
                continue;
            };
            let cex = pick(&opts.cex, i, 1.0);
            if !cex.is_finite() {
                continue;
            }
            let bg = if bg.is_empty() {
                None
            } else {
                bg[i % bg.len()]
            };
            let pch = pick(&opts.pch, i, pch);
            let lwd = pick(&opts.lwd, i, lwd);
            self.draw_pch(pch, p, cex, c, bg, lwd);
        }
        self.pop_clip();
    }

    /// Draws R's `segments()`; the public entry is
    /// [`Graphics::segments`](crate::graphics::Graphics::segments), whose docs
    /// describe it.
    pub(crate) fn segments_impl(
        &mut self,
        x0: &[f64],
        y0: &[f64],
        x1: &[f64],
        y1: &[f64],
        opts: &SegmentsOptions,
    ) {
        let mut dev = self.line_pars(opts.lend, opts.ljoin, opts.lmitre);
        dev.segments_draw(x0, y0, x1, y1, opts);
    }

    fn segments_draw(
        &mut self,
        x0: &[f64],
        y0: &[f64],
        x1: &[f64],
        y1: &[f64],
        opts: &SegmentsOptions,
    ) {
        let n = recycled_len(&[x0.len(), y0.len(), x1.len(), y1.len()]);
        let fg = ColorSpec::Color(self.par.fg);
        let (lty, lwd) = (self.par.lty, self.par.lwd);
        self.push_clip_region(self.clip_for(opts.xpd));
        for i in 0..n {
            let p0 = self.user_to_inches(x0[i % x0.len()], y0[i % y0.len()]);
            let p1 = self.user_to_inches(x1[i % x1.len()], y1[i % y1.len()]);
            if ![p0.x, p0.y, p1.x, p1.y].iter().all(|v| v.is_finite()) {
                continue;
            }
            let Some(c) = self.resolve(pick(&opts.col, i, fg)) else {
                continue;
            };
            let stroke = self.stroke_of(c, pick(&opts.lwd, i, lwd), pick(&opts.lty, i, lty));
            self.backend.draw_line(p0, p1, &stroke);
        }
        self.pop_clip();
    }

    /// Draws R's `arrows()`; the public entry is
    /// [`Graphics::arrows`](crate::graphics::Graphics::arrows), whose docs
    /// describe it.
    pub(crate) fn arrows_impl(
        &mut self,
        x0: &[f64],
        y0: &[f64],
        x1: &[f64],
        y1: &[f64],
        opts: &ArrowsOptions,
    ) {
        let mut dev = self.line_pars(opts.lend, opts.ljoin, opts.lmitre);
        dev.arrows_draw(x0, y0, x1, y1, opts);
    }

    fn arrows_draw(
        &mut self,
        x0: &[f64],
        y0: &[f64],
        x1: &[f64],
        y1: &[f64],
        opts: &ArrowsOptions,
    ) {
        let n = recycled_len(&[x0.len(), y0.len(), x1.len(), y1.len()]);
        let fg = ColorSpec::Color(self.par.fg);
        let (lty, lwd) = (self.par.lty, self.par.lwd);
        let length = opts.length.unwrap_or(0.25);
        let angle = opts.angle.unwrap_or(30.0).to_radians();
        let code = opts.code.unwrap_or(2);
        self.push_clip_region(self.clip_for(opts.xpd));
        for i in 0..n {
            let (a, b, c, d) = (
                x0[i % x0.len()],
                y0[i % y0.len()],
                x1[i % x1.len()],
                y1[i % y1.len()],
            );
            let from = self.user_to_inches(a, b);
            let to = self.user_to_inches(c, d);
            if ![from.x, from.y, to.x, to.y].iter().all(|v| v.is_finite()) {
                continue;
            }
            let Some(color) = self.resolve(pick(&opts.col, i, fg)) else {
                continue;
            };
            let stroke = self.stroke_of(color, pick(&opts.lwd, i, lwd), pick(&opts.lty, i, lty));
            self.backend.draw_line(from, to, &stroke);
            if code & 3 == 0 || length == 0.0 || (from.x - to.x).hypot(from.y - to.y) < 1e-3 {
                continue;
            }
            // A head at `tip`, opening back along the shaft towards `tail`.
            let mut head = |tip: Point, tail: Point| {
                let rot = (tail.y - tip.y).atan2(tail.x - tip.x);
                let barb = |a: f64| {
                    Point::new(
                        tip.x + length * (rot + a).cos(),
                        tip.y + length * (rot + a).sin(),
                    )
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

    /// Draws R's `rect()`; the public entry is
    /// [`Graphics::rect`](crate::graphics::Graphics::rect), whose docs
    /// describe it.
    pub(crate) fn rect_impl(
        &mut self,
        x0: &[f64],
        y0: &[f64],
        x1: &[f64],
        y1: &[f64],
        opts: &RectOptions,
    ) {
        let mut dev = self.line_pars(opts.lend, opts.ljoin, opts.lmitre);
        dev.rect_draw(x0, y0, x1, y1, opts);
    }

    fn rect_draw(&mut self, x0: &[f64], y0: &[f64], x1: &[f64], y1: &[f64], opts: &RectOptions) {
        let n = recycled_len(&[x0.len(), y0.len(), x1.len(), y1.len()]);
        let fg = ColorSpec::Color(self.par.fg);
        let (lty, lwd) = (self.par.lty, self.par.lwd);
        self.push_clip_region(self.clip_for(opts.xpd));
        for i in 0..n {
            let p0 = self.user_to_inches(x0[i % x0.len()], y0[i % y0.len()]);
            let p1 = self.user_to_inches(x1[i % x1.len()], y1[i % y1.len()]);
            if ![p0.x, p0.y, p1.x, p1.y].iter().all(|v| v.is_finite()) {
                continue;
            }
            let fill = self.resolve(pick(&opts.col, i, ColorSpec::None));
            let border = self.resolve(pick(&opts.border, i, fg));
            let stroke =
                border.map(|c| self.stroke_of(c, pick(&opts.lwd, i, lwd), pick(&opts.lty, i, lty)));
            self.backend
                .draw_rect(Rect::new(p0.x, p0.y, p1.x, p1.y), fill, stroke.as_ref());
        }
        self.pop_clip();
    }

    /// Draws R's `polygon()`; the public entry is
    /// [`Graphics::polygon`](crate::graphics::Graphics::polygon), whose docs
    /// describe it.
    pub(crate) fn polygon_impl(&mut self, x: &[f64], y: &[f64], opts: &PolygonOptions) {
        let mut dev = self.line_pars(opts.lend, opts.ljoin, opts.lmitre);
        dev.polygon_draw(x, y, opts);
    }

    fn polygon_draw(&mut self, x: &[f64], y: &[f64], opts: &PolygonOptions) {
        let fg = ColorSpec::Color(self.par.fg);
        let (lty, lwd) = (self.par.lty, opts.lwd.unwrap_or(self.par.lwd));
        self.push_clip_region(self.clip_for(opts.xpd));
        let mut num = 0;
        let mut run: Vec<Point> = Vec::new();
        let n = x.len().min(y.len());
        for i in 0..=n {
            let p = (i < n).then(|| self.user_to_inches(x[i], y[i]));
            if let Some(p) = p.filter(|p| p.x.is_finite() && p.y.is_finite()) {
                run.push(p);
                continue;
            }
            // As `C_polygon`: a run needs at least two points; a shorter one
            // is skipped without using up a `col`/`border`/`lty` value.
            if run.len() >= 2 {
                let fill = self.resolve(pick(&opts.col, num, ColorSpec::None));
                let border = self.resolve(pick(&opts.border, num, fg));
                let stroke = border.map(|c| self.stroke_of(c, lwd, pick(&opts.lty, num, lty)));
                self.backend.draw_polygon(&run, fill, stroke.as_ref());
                num += 1;
            }
            run.clear();
        }
        self.pop_clip();
    }

    /// Draws R's `text()`; the public entry is
    /// [`Graphics::text`](crate::graphics::Graphics::text), whose docs
    /// describe it.
    pub(crate) fn text_impl<L: Clone + Into<Label>>(
        &mut self,
        x: &[f64],
        y: &[f64],
        labels: &[L],
        opts: &TextOptions,
    ) {
        let n = recycled_len(&[x.len(), y.len()]);
        let clip = self.clip_for(opts.xpd);
        let hadj = opts.adj.unwrap_or(self.par.adj);
        assert!(
            opts.pos.iter().all(|p| (1..=4).contains(p)),
            "invalid 'pos' value"
        );
        // `offset` characters, as `GConvertXUnits(offset, CHARS, INCHES)`
        // before any label's own `cex`: the character height (R's
        // `cra[1]`) times `par(cex)` as of the last layout (see
        // `layout_cex`).
        let offset = opts.offset.unwrap_or(0.5) * self.layout_cex * 1.2 * self.start_ps / 72.0;
        let (col, font) = (ColorSpec::Color(self.par.col), self.par.font);
        for i in 0..n {
            let label: Label = if labels.is_empty() {
                Label::Text((i + 1).to_string())
            } else {
                labels[i % labels.len()].clone().into()
            };
            let p = self.user_to_inches(x[i % x.len()], y[i % y.len()]);
            // As `C_text`: a label at a non-finite position (`NA`, or <= 0
            // on a log axis) is skipped; the others keep their index.
            if !(p.x.is_finite() && p.y.is_finite()) {
                continue;
            }
            let (p, hadj, vadj) = match opts.pos.get(i % opts.pos.len().max(1)) {
                Some(1) => (
                    Point::new(p.x, p.y - offset),
                    0.5,
                    Some(1.0 - (0.5 - Y_CHAR_OFFSET)),
                ),
                Some(2) => (Point::new(p.x - offset, p.y), 1.0, Some(Y_CHAR_OFFSET)),
                Some(3) => (Point::new(p.x, p.y + offset), 0.5, Some(0.0)),
                Some(4) => (Point::new(p.x + offset, p.y), 0.0, Some(Y_CHAR_OFFSET)),
                _ => (p, hadj, opts.vadj),
            };
            self.draw_label(
                clip,
                p,
                &label,
                hadj,
                vadj,
                opts.srt.unwrap_or(self.par.srt),
                pick(&opts.col, i, col),
                pick(&opts.cex, i, 1.0),
                pick(&opts.font, i, font),
            );
        }
    }

    /// Draws R's `box()`; the public entry is
    /// [`Graphics::box_`](crate::graphics::Graphics::box_), whose docs
    /// describe it.
    pub(crate) fn box_impl(&mut self, opts: &BoxOptions) {
        let mut dev = self.line_pars(opts.lend, opts.ljoin, opts.lmitre);
        dev.box_draw(opts);
    }

    fn box_draw(&mut self, opts: &BoxOptions) {
        let Some(color) = self.resolve(opts.col.unwrap_or(ColorSpec::Color(self.par.col))) else {
            return;
        };
        let stroke = self.stroke_of(
            color,
            opts.lwd.unwrap_or(self.par.lwd),
            opts.lty.unwrap_or(LineType::SOLID),
        );
        let which = opts.which.unwrap_or_default();
        let rect = match which {
            BoxWhich::Plot => self.plt,
            BoxWhich::Figure => self.fig,
            BoxWhich::Inner => self.inner,
            BoxWhich::Outer => Rect::new(0.0, 0.0, self.width_in, self.height_in),
        };
        self.dirty();
        let bty = match which {
            BoxWhich::Plot => opts.bty.unwrap_or(self.par.bty),
            _ => Bty::O,
        };
        // `GBox()`'s corners, in its order: x[0..7] = bottom-left,
        // bottom-right, top-right, top-left, then the first three again;
        // each `bty` is a run of them.
        let (x0, y0, x1, y1) = (rect.x0, rect.y0, rect.x1, rect.y1);
        let corners = [
            Point::new(x0, y0),
            Point::new(x1, y0),
            Point::new(x1, y1),
            Point::new(x0, y1),
            Point::new(x0, y0),
            Point::new(x1, y0),
            Point::new(x1, y1),
        ];
        let run = match bty {
            Bty::O => {
                self.backend.draw_rect(rect, None, Some(&stroke));
                return;
            }
            Bty::N => return,
            Bty::L => &corners[3..6],
            Bty::Seven => &corners[1..4],
            Bty::C => &corners[2..6],
            Bty::Bracket => &corners[0..4],
            Bty::U => &corners[3..7],
        };
        self.backend.draw_polyline(run, &stroke);
    }
}
