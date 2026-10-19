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

use super::{Clip, Device};
use crate::backend::Backend;
use crate::color::{Color, ColorSpec};
use crate::geometry::{Point, Rect};
use crate::highlevel::PlotType;
use crate::lty::LineType;
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
    /// R's `type`: `None` is `"p"` for `points()` and `"l"` for `lines()`.
    pub type_: Option<PlotType>,
    /// Plotting symbols, recycled per point; empty: `par(pch)`.
    pub pch: Vec<i32>,
    /// Colours, recycled per point (lines use the first); empty:
    /// `par(col)`.
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
    /// R's inline `xpd`: `Some(true)` clips to the figure region,
    /// `Some(false)` to the plot region; `None`: `par(xpd)`.
    pub xpd: Option<bool>,
}

crate::setters::setters!(XyOptions {
    type_: option PlotType,
    pch: values i32,
    col: values ColorSpec,
    bg: values ColorSpec,
    cex: values f64,
    lwd: values f64,
    lty: option LineType,
    xpd: option bool,
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
    /// R's inline `xpd`: `Some(true)` clips to the figure region,
    /// `Some(false)` to the plot region; `None`: `par(xpd)`.
    pub xpd: Option<bool>,
}

crate::setters::setters!(SegmentsOptions {
    col: values ColorSpec,
    lty: values LineType,
    lwd: values f64,
    xpd: option bool,
});

/// Arguments of `arrows()`.
#[derive(Clone, Debug)]
pub struct ArrowsOptions {
    /// Length of the head's edges in inches (R's default 0.25).
    pub length: f64,
    /// Angle between the shaft and each edge of the head, in degrees (R's
    /// default 30; 90 gives flat caps, as for error bars).
    pub angle: f64,
    /// Heads at the start (1), the end (2, R's default) or both ends (3).
    pub code: u8,
    /// Recycled per arrow; empty: `par(fg)` (R's default).
    pub col: Vec<ColorSpec>,
    /// Recycled per arrow; empty: `par(lty)`.
    pub lty: Vec<LineType>,
    /// Recycled per arrow; empty: `par(lwd)`.
    pub lwd: Vec<f64>,
    /// R's inline `xpd`: `Some(true)` clips to the figure region,
    /// `Some(false)` to the plot region; `None`: `par(xpd)`.
    pub xpd: Option<bool>,
}

crate::setters::setters!(ArrowsOptions {
    length: value f64,
    angle: value f64,
    code: value u8,
    col: values ColorSpec,
    lty: values LineType,
    lwd: values f64,
    xpd: option bool,
});

impl Default for ArrowsOptions {
    fn default() -> Self {
        ArrowsOptions {
            length: 0.25,
            angle: 30.0,
            code: 2,
            col: Vec::new(),
            lty: Vec::new(),
            lwd: Vec::new(),
            xpd: None,
        }
    }
}

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
    /// R's inline `xpd`: `Some(true)` clips to the figure region,
    /// `Some(false)` to the plot region; `None`: `par(xpd)`.
    pub xpd: Option<bool>,
}

crate::setters::setters!(RectOptions {
    col: values ColorSpec,
    border: values ColorSpec,
    lty: values LineType,
    lwd: values f64,
    xpd: option bool,
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
    /// R's inline `xpd`: `Some(true)` clips to the figure region,
    /// `Some(false)` to the plot region; `None`: `par(xpd)`.
    pub xpd: Option<bool>,
}

crate::setters::setters!(PolygonOptions {
    col: values ColorSpec,
    border: values ColorSpec,
    lty: values LineType,
    lwd: option f64,
    xpd: option bool,
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
    /// Rotation in degrees, counterclockwise (R's `srt`).
    pub srt: f64,
    /// Recycled per label, times `par(cex)`; empty: 1.
    pub cex: Vec<f64>,
    /// Recycled per label; empty: `par(col)`.
    pub col: Vec<ColorSpec>,
    /// Recycled per label (1 plain, 2 bold, 3 italic, 4 bold italic, 5
    /// Symbol); empty: `par(font)`.
    pub font: Vec<i32>,
    /// R's inline `xpd`: `Some(true)` clips to the figure region,
    /// `Some(false)` to the plot region; `None`: `par(xpd)`.
    pub xpd: Option<bool>,
}

crate::setters::setters!(TextOptions {
    adj: option f64,
    vadj: option f64,
    srt: value f64,
    cex: values f64,
    col: values ColorSpec,
    font: values i32,
    xpd: option bool,
});

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
    pub which: BoxWhich,
    /// `None`: `par(col)` (R's `C_box`; R's `par(fg = )` sets `col` too,
    /// so set both `par.fg` and `par.col` to follow it).
    pub col: Option<ColorSpec>,
    /// `None`: solid (R's `box(lty = "solid")`, not `par(lty)`).
    pub lty: Option<LineType>,
    /// `None`: `par(lwd)`.
    pub lwd: Option<f64>,
}

crate::setters::setters!(BoxOptions {
    which: value BoxWhich,
    col: option ColorSpec,
    lty: option LineType,
    lwd: option f64,
});

/// Arguments of `plot.window()` besides the limits.
#[derive(Clone, Debug, Default)]
pub struct PlotWindowOptions {
    /// Log x axis (R's `log = "x"`).
    pub xlog: bool,
    /// Log y axis (R's `log = "y"`).
    pub ylog: bool,
    /// R's `asp`: one x unit is `asp` y units long on the page. Ignored on
    /// log axes, as in R.
    pub asp: Option<f64>,
}

crate::setters::setters!(PlotWindowOptions {
    xlog: value bool,
    ylog: value bool,
    asp: option f64,
});

impl<B: Backend> Device<B> {
    fn clip_for(&self, xpd: Option<bool>) -> Clip {
        if xpd.unwrap_or(self.par.xpd) {
            Clip::Figure
        } else {
            Clip::Plot
        }
    }

    /// `plot.window(xlim, ylim, log, asp)`: sets up the user coordinates
    /// of the current plot region. Call it after [`Device::plot_new`] and
    /// before drawing.
    ///
    /// - `xlim`/`ylim` are in data units, also on a log axis, and may be
    ///   decreasing (a reversed axis).
    /// - `opts.xlog`/`ylog` make that axis log10 (R's `log = "x"`/`"y"`/
    ///   `"xy"`). [`Device::usr`] is then in log10 units on that axis, as
    ///   R's `par("usr")` is.
    /// - `opts.asp` (linear axes only, as in R): `C_plot_window` first
    ///   widens whichever range is needed so one x unit is `asp` y units
    ///   long on the page, centred on the given limits.
    /// - Each range then goes through R's `GScale()` (`graphics.c`):
    ///   - With `par(xaxs/yaxs) = "r"` (the default), 4% is added at each
    ///     end; with `"i"` the limits are used as given.
    ///   - A zero-width range is widened: to -1..1 around 0, otherwise by
    ///     40% of the value either side.
    ///   - Non-finite limits are replaced by huge finite ones. On a log
    ///     axis, a limit <= 0 gives a lower end of about 1e-307, where R
    ///     warns "nonfinite axis limits"; this function doesn't warn or
    ///     panic.
    /// - Sets `par.xaxp`/`par.yaxp` (the default tick range and count,
    ///   R's `GAxisPars()`) from the new range and `par.lab`. Later calls
    ///   to `axis()` without `at` read them. Changing `par.lab` afterwards
    ///   doesn't change them, as in R.
    pub fn plot_window(&mut self, xlim: (f64, f64), ylim: (f64, f64), opts: &PlotWindowOptions) {
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

    /// `points(x, y, ...)`: R's `plot.xy(type = "p")` unless
    /// `opts.type_` says otherwise. Points with a non-finite coordinate
    /// (including values <= 0 on a log axis) are skipped.
    pub fn points(&mut self, x: &[f64], y: &[f64], opts: &XyOptions) {
        self.plot_xy(x, y, opts.type_.unwrap_or(PlotType::Points), opts);
    }

    /// `lines(x, y, ...)`: R's `plot.xy(type = "l")` unless `opts.type_`
    /// says otherwise. The polyline breaks at non-finite coordinates
    /// (R's `NA`, or values <= 0 on a log axis).
    pub fn lines(&mut self, x: &[f64], y: &[f64], opts: &XyOptions) {
        self.plot_xy(x, y, opts.type_.unwrap_or(PlotType::Lines), opts);
    }

    /// `C_plotXY` for `type` `"p"`, `"l"`, `"b"`, `"c"`, `"o"` and `"n"`.
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
            PlotType::None => {}
        }
    }

    fn xy_points(&mut self, x: &[f64], y: &[f64], opts: &XyOptions, clip: Clip) {
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

    /// `segments(x0, y0, x1, y1, ...)` in data units. The four coordinate
    /// slices and the styles are recycled to the longest; a segment with a
    /// non-finite end after conversion (including values <= 0 on a log
    /// axis) is skipped, as `C_segments`.
    pub fn segments(
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

    /// `arrows(x0, y0, x1, y1, ...)`, a port of `C_arrows` / `GArrow()`:
    /// each segment with open heads - two lines of `length` inches at
    /// `angle` degrees to the shaft - at its start, end or both (`code`).
    /// Coordinates and styles are recycled to the longest. As in R, an
    /// arrow with a non-finite end after conversion (including values <= 0
    /// on a log axis) is skipped, and the heads of a segment shorter than
    /// 1/1000 inch are not drawn (R also warns).
    pub fn arrows(&mut self, x0: &[f64], y0: &[f64], x1: &[f64], y1: &[f64], opts: &ArrowsOptions) {
        let n = recycled_len(&[x0.len(), y0.len(), x1.len(), y1.len()]);
        let fg = ColorSpec::Color(self.par.fg);
        let (lty, lwd) = (self.par.lty, self.par.lwd);
        let (length, angle, code) = (opts.length, opts.angle.to_radians(), opts.code);
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

    /// `rect(xleft, ybottom, xright, ytop, ...)` in data units, with
    /// coordinates and styles recycled to the longest. Rectangles with a
    /// non-finite corner (e.g. at 0 on a log axis) are skipped, as in R.
    pub fn rect(&mut self, x0: &[f64], y0: &[f64], x1: &[f64], y1: &[f64], opts: &RectOptions) {
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

    /// `polygon(x, y, ...)`: one polygon per run of finite points
    /// (non-finite coordinates, R's `NA`, separate them), with `col`,
    /// `border` and `lty` recycled over the polygons, as `C_polygon`.
    pub fn polygon(&mut self, x: &[f64], y: &[f64], opts: &PolygonOptions) {
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
            if !run.is_empty() {
                let fill = self.resolve(pick(&opts.col, num, ColorSpec::None));
                let border = self.resolve(pick(&opts.border, num, fg));
                let stroke = border.map(|c| self.stroke_of(c, lwd, pick(&opts.lty, num, lty)));
                self.backend.draw_polygon(&run, fill, stroke.as_ref());
                run.clear();
                num += 1;
            }
        }
        self.pop_clip();
    }

    /// `text(x, y, labels, ...)` in data units: each label (text, or a
    /// plotmath expression such as `Label::math("alpha^2")`) at its point,
    /// with `x`/`y` recycled to the longer and labels recycled over the
    /// points (extra labels are ignored; R truncates them with a warning).
    /// Empty `labels`
    /// are R's default, the indices `"1"`, `"2"`, .... Placement follows
    /// `GEText()`/`GEMathText()`: horizontally `adj` of the width;
    /// vertically see [`TextOptions::vadj`].
    pub fn text<L: Clone + Into<Label>>(
        &mut self,
        x: &[f64],
        y: &[f64],
        labels: &[L],
        opts: &TextOptions,
    ) {
        let n = recycled_len(&[x.len(), y.len()]);
        let clip = self.clip_for(opts.xpd);
        let hadj = opts.adj.unwrap_or(self.par.adj);
        let (col, font) = (ColorSpec::Color(self.par.col), self.par.font);
        for i in 0..n {
            let label: Label = if labels.is_empty() {
                Label::Text((i + 1).to_string())
            } else {
                labels[i % labels.len()].clone().into()
            };
            let p = self.user_to_inches(x[i % x.len()], y[i % y.len()]);
            self.draw_label(
                clip,
                p,
                &label,
                hadj,
                opts.vadj,
                opts.srt,
                pick(&opts.col, i, col),
                pick(&opts.cex, i, 1.0),
                pick(&opts.font, i, font),
            );
        }
    }

    /// `box(which, ...)`: outlines the plot region, figure, inner region
    /// or the whole device. Not clipped.
    pub fn box_(&mut self, opts: &BoxOptions) {
        let Some(color) = self.resolve(opts.col.unwrap_or(ColorSpec::Color(self.par.col))) else {
            return;
        };
        let stroke = self.stroke_of(
            color,
            opts.lwd.unwrap_or(self.par.lwd),
            opts.lty.unwrap_or(LineType::SOLID),
        );
        let rect = match opts.which {
            BoxWhich::Plot => self.plt,
            BoxWhich::Figure => self.fig,
            BoxWhich::Inner => {
                let omi = self.omi();
                Rect::new(
                    omi.left,
                    omi.bottom,
                    self.width_in - omi.right,
                    self.height_in - omi.top,
                )
            }
            BoxWhich::Outer => Rect::new(0.0, 0.0, self.width_in, self.height_in),
        };
        self.dirty();
        self.backend.draw_rect(rect, None, Some(&stroke));
    }
}
