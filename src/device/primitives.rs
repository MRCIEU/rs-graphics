// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! The low-level drawing functions `points()`, `lines()`, `segments()`,
//! `arrows()`, `rect()`, `polygon()`, `text()`, `box()` and
//! `plot.window()`: the options structs of their builder calls (see
//! [`Graphics`](crate::graphics::Graphics)), with R's arguments.
//!
//! An empty `Vec` (or `None`) field means R's default for that argument,
//! usually the `par()` value; a field R recycles per element (`col` in
//! `points(x, y, col = groups)`) is a `Vec` used in turn and repeated as
//! needed. `xpd` is R's inline `xpd = ` (clip to the plot region, the
//! figure region or the device: [`Xpd`]); `None` uses [`par(xpd)`].
//!
//! [`par(xpd)`]: crate::par::Par::xpd

use super::hatch::{ShapeStyle, shading};
use super::{Device, Xpd};
use crate::backend::{Backend, LineEnd, LineJoin};
use crate::color::{Color, ColorSpec};
use crate::error::Error;
use crate::font::FontFace;
use crate::geometry::{Point, Rect};
use crate::highlevel::PlotType;
use crate::lty::LineType;
use crate::par::{AxisStyle, Bty};
use crate::plotmath::Label;
use crate::recycle::{pick, pick_na, recycled_len};

crate::setters::options! {
    /// Arguments of `points()` and `lines()`, both R's `plot.xy()`.
    #[derive(Clone, Debug, Default)]
    pub struct XyOptions => [PointsCall, LinesCall] {
        /// What to draw: points, lines, both, steps, ... (see [`PlotType`]).
        /// `None`: points for `points()`, lines for `lines()`. R's `type`.
        pub type_: Option<PlotType> => option PlotType,
        /// Plotting symbols, recycled over the points; characters work too
        /// (a negative value is minus a Unicode code point, as in R). Empty:
        /// [`par(pch)`]; `None`: no point there.
        ///
        /// [`par(pch)`]: crate::par::Par::pch
        pub pch: Vec<Option<i32>> => na_values i32,
        /// Colours, recycled over the points. Lines use the first, except
        /// for `type = "h"`, which has one per point. Empty: [`par(col)`].
        ///
        /// [`par(col)`]: crate::par::Par::col
        pub col: Vec<ColorSpec> => values ColorSpec,
        /// Fill colours of symbols 21 to 25, recycled over the points.
        /// Empty: no fill.
        pub bg: Vec<ColorSpec> => values ColorSpec,
        /// Symbol sizes, relative to [`par(cex)`], recycled over the points.
        /// Empty: 1; `None`: no point there.
        ///
        /// [`par(cex)`]: crate::par::Par::cex
        pub cex: Vec<Option<f64>> => na_values f64,
        /// Line widths, recycled over the points (lines use the first).
        /// Empty: [`par(lwd)`]; `None`: no point there, while lines use
        /// [`par(lwd)`], as in R.
        ///
        /// [`par(lwd)`]: crate::par::Par::lwd
        pub lwd: Vec<Option<f64>> => na_values f64,
        /// Line type of the lines. `None`: [`par(lty)`].
        ///
        /// [`par(lty)`]: crate::par::Par::lty
        pub lty: Option<LineType> => option LineType,
        /// Where drawing is clipped: [`Xpd::Plot`], [`Xpd::Figure`] or
        /// [`Xpd::Device`] (the setter also takes `false`/`true`). `None`:
        /// [`par(xpd)`]; overrides it for this call only.
        ///
        /// [`par(xpd)`]: crate::par::Par::xpd
        pub xpd: Option<Xpd> => option Xpd,
        /// How line ends are drawn: `"round"`, `"butt"` or `"square"`.
        /// `None`: [`par(lend)`]; overrides it for this call only.
        ///
        /// [`par(lend)`]: crate::par::Par::lend
        pub lend: Option<LineEnd> => option LineEnd,
        /// How line corners are joined: `"round"`, `"mitre"` or `"bevel"`.
        /// `None`: [`par(ljoin)`]; overrides it for this call only.
        ///
        /// [`par(ljoin)`]: crate::par::Par::ljoin
        pub ljoin: Option<LineJoin> => option LineJoin,
        /// Mitre limit of mitred joins (at least 1). `None`: [`par(lmitre)`];
        /// overrides it for this call only.
        ///
        /// [`par(lmitre)`]: crate::par::Par::lmitre
        pub lmitre: Option<f64> => option f64,
        /// Font of character symbols (such as `pch = "a"` or `'é'`). `None`:
        /// [`par(font)`]; overrides it for this call only.
        ///
        /// [`par(font)`]: crate::par::Par::font
        pub font: Option<FontFace> => option FontFace,
    }
}

crate::setters::options! {
    /// Arguments of `segments()`.
    #[derive(Clone, Debug, Default)]
    pub struct SegmentsOptions => [SegmentsCall] {
        /// Segment colours, recycled per segment. Empty: [`par(fg)`], as in R.
        ///
        /// [`par(fg)`]: crate::par::Par::fg
        pub col: Vec<ColorSpec> => values ColorSpec,
        /// Line types, recycled per segment. Empty: [`par(lty)`]; `None`: no
        /// segment.
        ///
        /// [`par(lty)`]: crate::par::Par::lty
        pub lty: Vec<Option<LineType>> => na_values LineType,
        /// Line widths, recycled per segment. Empty: [`par(lwd)`]; `None`: no
        /// segment.
        ///
        /// [`par(lwd)`]: crate::par::Par::lwd
        pub lwd: Vec<Option<f64>> => na_values f64,
        /// How the segment ends are drawn: `"round"`, `"butt"` or
        /// `"square"`. `None`: [`par(lend)`]; overrides it for this call only.
        ///
        /// [`par(lend)`]: crate::par::Par::lend
        pub lend: Option<LineEnd> => option LineEnd,
        /// Where drawing is clipped: [`Xpd::Plot`], [`Xpd::Figure`] or
        /// [`Xpd::Device`] (the setter also takes `false`/`true`). `None`:
        /// [`par(xpd)`]; overrides it for this call only.
        ///
        /// [`par(xpd)`]: crate::par::Par::xpd
        pub xpd: Option<Xpd> => option Xpd,
        /// How line corners are joined: `"round"`, `"mitre"` or `"bevel"`.
        /// `None`: [`par(ljoin)`]; overrides it for this call only.
        ///
        /// [`par(ljoin)`]: crate::par::Par::ljoin
        pub ljoin: Option<LineJoin> => option LineJoin,
        /// Mitre limit of mitred joins (at least 1). `None`: [`par(lmitre)`];
        /// overrides it for this call only.
        ///
        /// [`par(lmitre)`]: crate::par::Par::lmitre
        pub lmitre: Option<f64> => option f64,
    }
}

crate::setters::options! {
    /// Arguments of `arrows()`.
    #[derive(Clone, Debug, Default)]
    pub struct ArrowsOptions => [ArrowsCall] {
        /// Length of the edges of each head, in inches. `None`: 0.25.
        pub length: Option<f64> => option f64,
        /// Angle between the shaft and each edge of the head, in degrees (90
        /// gives flat caps, as for error bars). `None`: 30.
        pub angle: Option<f64> => option f64,
        /// Where the heads go: at the start (1), the end (2) or both ends
        /// (3). `None`: 2.
        pub code: Option<u8> => option u8,
        /// Arrow colours, recycled per arrow. Empty: [`par(fg)`], as in R.
        ///
        /// [`par(fg)`]: crate::par::Par::fg
        pub col: Vec<ColorSpec> => values ColorSpec,
        /// Line types, recycled per arrow. Empty: [`par(lty)`]; `None`: no
        /// arrow.
        ///
        /// [`par(lty)`]: crate::par::Par::lty
        pub lty: Vec<Option<LineType>> => na_values LineType,
        /// Line widths, recycled per arrow. Empty: [`par(lwd)`]; `None`: no
        /// arrow.
        ///
        /// [`par(lwd)`]: crate::par::Par::lwd
        pub lwd: Vec<Option<f64>> => na_values f64,
        /// Where drawing is clipped: [`Xpd::Plot`], [`Xpd::Figure`] or
        /// [`Xpd::Device`] (the setter also takes `false`/`true`). `None`:
        /// [`par(xpd)`]; overrides it for this call only.
        ///
        /// [`par(xpd)`]: crate::par::Par::xpd
        pub xpd: Option<Xpd> => option Xpd,
        /// How line ends are drawn: `"round"`, `"butt"` or `"square"`.
        /// `None`: [`par(lend)`]; overrides it for this call only.
        ///
        /// [`par(lend)`]: crate::par::Par::lend
        pub lend: Option<LineEnd> => option LineEnd,
        /// How line corners are joined: `"round"`, `"mitre"` or `"bevel"`.
        /// `None`: [`par(ljoin)`]; overrides it for this call only.
        ///
        /// [`par(ljoin)`]: crate::par::Par::ljoin
        pub ljoin: Option<LineJoin> => option LineJoin,
        /// Mitre limit of mitred joins (at least 1). `None`: [`par(lmitre)`];
        /// overrides it for this call only.
        ///
        /// [`par(lmitre)`]: crate::par::Par::lmitre
        pub lmitre: Option<f64> => option f64,
    }
}

crate::setters::options! {
    /// Arguments of `rect()`.
    #[derive(Clone, Debug, Default)]
    pub struct RectOptions => [RectCall] {
        /// Fill colours, recycled per rectangle. Empty: no fill. With
        /// shading, the colour of the shading lines instead (see
        /// [`RectOptions::density`]).
        pub col: Vec<ColorSpec> => values ColorSpec,
        /// Border colours, recycled per rectangle. Empty: [`par(fg)`] (with
        /// shading: `col`); `ColorSpec::None`: no border.
        ///
        /// [`par(fg)`]: crate::par::Par::fg
        pub border: Vec<ColorSpec> => values ColorSpec,
        /// Border line types, recycled per rectangle. Empty: [`par(lty)`];
        /// `None`: no border and no shading lines (the fill is still drawn).
        ///
        /// [`par(lty)`]: crate::par::Par::lty
        pub lty: Vec<Option<LineType>> => na_values LineType,
        /// Border line widths, recycled per rectangle. Empty: [`par(lwd)`];
        /// `None`: no border and no shading lines (the fill is still drawn).
        ///
        /// [`par(lwd)`]: crate::par::Par::lwd
        pub lwd: Vec<Option<f64>> => na_values f64,
        /// Where drawing is clipped: [`Xpd::Plot`], [`Xpd::Figure`] or
        /// [`Xpd::Device`] (the setter also takes `false`/`true`). `None`:
        /// [`par(xpd)`]; overrides it for this call only.
        ///
        /// [`par(xpd)`]: crate::par::Par::xpd
        pub xpd: Option<Xpd> => option Xpd,
        /// How line ends are drawn: `"round"`, `"butt"` or `"square"`.
        /// `None`: [`par(lend)`]; overrides it for this call only.
        ///
        /// [`par(lend)`]: crate::par::Par::lend
        pub lend: Option<LineEnd> => option LineEnd,
        /// How line corners are joined: `"round"`, `"mitre"` or `"bevel"`.
        /// `None`: [`par(ljoin)`]; overrides it for this call only.
        ///
        /// [`par(ljoin)`]: crate::par::Par::ljoin
        pub ljoin: Option<LineJoin> => option LineJoin,
        /// Mitre limit of mitred joins (at least 1). `None`: [`par(lmitre)`];
        /// overrides it for this call only.
        ///
        /// [`par(lmitre)`]: crate::par::Par::lmitre
        pub lmitre: Option<f64> => option f64,
        /// Shading lines per inch, recycled per rectangle. `None` or a
        /// negative value fills with `col` instead; 0 draws neither lines
        /// nor fill. Empty (or all `None`/negative): no shading. With
        /// shading, `col` is the colour of the lines (empty or `None`:
        /// [`par(fg)`]) and `border` defaults to `col`. The lines are skipped
        /// on log axes, as in R (where they also give a warning).
        ///
        /// [`par(fg)`]: crate::par::Par::fg
        pub density: Vec<Option<f64>> => na_values f64,
        /// Angle of the shading lines in degrees, anticlockwise, recycled
        /// per rectangle. Empty: 45.
        pub angle: Vec<f64> => values f64,
    }
}

crate::setters::options! {
    /// Arguments of `polygon()`. A non-finite coordinate (R's `NA`) ends one
    /// polygon and starts the next; `col`, `border`, `lty`, `density` and
    /// `angle` are recycled over those polygons.
    #[derive(Clone, Debug, Default)]
    pub struct PolygonOptions => [PolygonCall] {
        /// Fill colours, recycled per polygon. Empty: no fill. With shading,
        /// the colour of the shading lines instead (see
        /// [`PolygonOptions::density`]).
        pub col: Vec<ColorSpec> => values ColorSpec,
        /// Border colours, recycled per polygon. Empty: [`par(fg)`] (with
        /// shading: `col`); `ColorSpec::None`: no border.
        ///
        /// [`par(fg)`]: crate::par::Par::fg
        pub border: Vec<ColorSpec> => values ColorSpec,
        /// Border line types, recycled per polygon. Empty: [`par(lty)`];
        /// `None`: no border and no shading lines (the fill is still drawn).
        ///
        /// [`par(lty)`]: crate::par::Par::lty
        pub lty: Vec<Option<LineType>> => na_values LineType,
        /// Border line width, one for all polygons. `None`: [`par(lwd)`].
        ///
        /// [`par(lwd)`]: crate::par::Par::lwd
        pub lwd: Option<f64> => option f64,
        /// Where drawing is clipped: [`Xpd::Plot`], [`Xpd::Figure`] or
        /// [`Xpd::Device`] (the setter also takes `false`/`true`). `None`:
        /// [`par(xpd)`]; overrides it for this call only.
        ///
        /// [`par(xpd)`]: crate::par::Par::xpd
        pub xpd: Option<Xpd> => option Xpd,
        /// How line ends are drawn: `"round"`, `"butt"` or `"square"`.
        /// `None`: [`par(lend)`]; overrides it for this call only.
        ///
        /// [`par(lend)`]: crate::par::Par::lend
        pub lend: Option<LineEnd> => option LineEnd,
        /// How line corners are joined: `"round"`, `"mitre"` or `"bevel"`.
        /// `None`: [`par(ljoin)`]; overrides it for this call only.
        ///
        /// [`par(ljoin)`]: crate::par::Par::ljoin
        pub ljoin: Option<LineJoin> => option LineJoin,
        /// Mitre limit of mitred joins (at least 1). `None`: [`par(lmitre)`];
        /// overrides it for this call only.
        ///
        /// [`par(lmitre)`]: crate::par::Par::lmitre
        pub lmitre: Option<f64> => option f64,
        /// Shading lines per inch, recycled per polygon. `None` or a
        /// negative value fills with `col` instead; 0 draws neither lines
        /// nor fill. Empty (or all `None`/negative): no shading. With
        /// shading, `col` is the colour of the lines (empty or `None`:
        /// [`par(fg)`]) and `border` defaults to `col`. The lines are skipped
        /// on log axes, as in R (where they also give a warning).
        ///
        /// [`par(fg)`]: crate::par::Par::fg
        pub density: Vec<Option<f64>> => na_values f64,
        /// Angle of the shading lines in degrees, anticlockwise, recycled
        /// per polygon. Empty: 45.
        pub angle: Vec<f64> => values f64,
        /// Uses the even-odd rule for shading: lines go where the border
        /// encircles a point an odd number of times, rather than a non-zero
        /// number (so a pentagram's centre isn't shaded). Default `false`.
        /// Affects only the shading: solid fills use the non-zero rule, as
        /// R's `pdf()` and the other devices here do. R's `fillOddEven`.
        pub fill_odd_even: bool => value bool,
    }
}

crate::setters::options! {
    /// Arguments of `text()`.
    #[derive(Clone, Debug, Default)]
    pub struct TextOptions => [TextCall] {
        /// Horizontal justification: 0 left, 0.5 centred, 1 right. `None`:
        /// [`par(adj)`]. R's `adj[1]`.
        ///
        /// [`par(adj)`]: crate::par::Par::adj
        pub adj: Option<f64> => option f64,
        /// Vertical justification. A finite value puts a text's baseline
        /// `vadj * height("M")` below the point, or puts the point that
        /// fraction of an expression's box height above the box's bottom.
        /// `None` (the default) centres the text's ink, or an expression's
        /// box, on the point. For text with newlines it places the block of
        /// lines (`None`: 0.5), each line's baseline as above. R's `adj[2]`.
        pub vadj: Option<f64> => option f64,
        /// Rotation in degrees, anticlockwise. `None`: [`par(srt)`] (0 by
        /// default). R's `srt`.
        ///
        /// [`par(srt)`]: crate::par::Par::srt
        pub srt: Option<f64> => option f64,
        /// Text sizes, relative to [`par(cex)`], recycled per label. Empty or
        /// `None`: 1.
        ///
        /// [`par(cex)`]: crate::par::Par::cex
        pub cex: Vec<Option<f64>> => na_values f64,
        /// Text colours, recycled per label. Empty: [`par(col)`].
        ///
        /// [`par(col)`]: crate::par::Par::col
        pub col: Vec<ColorSpec> => values ColorSpec,
        /// Fonts, recycled per label: R's numbers 1 to 5, names or
        /// [`FontFace`] values. Empty or `None`: [`par(font)`].
        ///
        /// [`par(font)`]: crate::par::Par::font
        pub font: Vec<Option<FontFace>> => na_values FontFace,
        /// Where drawing is clipped: [`Xpd::Plot`], [`Xpd::Figure`] or
        /// [`Xpd::Device`] (the setter also takes `false`/`true`). `None`:
        /// [`par(xpd)`]; overrides it for this call only.
        ///
        /// [`par(xpd)`]: crate::par::Par::xpd
        pub xpd: Option<Xpd> => option Xpd,
        /// Puts each label below (1), to the left of (2), above (3) or to
        /// the right of (4) its point, `offset` away, recycled per label;
        /// overrides `adj`/`vadj`. Empty: not used.
        pub pos: Vec<i32> => values i32,
        /// Distance of a label from its point when `pos` is given, in
        /// character widths. `None`: 0.5.
        pub offset: Option<f64> => option f64,
    }
}

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

crate::setters::options! {
    /// Arguments of `box()`.
    #[derive(Clone, Debug, Default)]
    pub struct BoxOptions => [BoxCall] {
        /// Which region to outline. `None`: the plot region.
        pub which: Option<BoxWhich> => option BoxWhich,
        /// Colour of the box. `None` or `ColorSpec::None`: `fg` if given,
        /// else [`par(col)`], as in R. (R's `par(fg = )` sets `col` too, so
        /// set both [`par(fg)`] and [`par(col)`] to follow it.)
        ///
        /// [`par(col)`]: crate::par::Par::col
        /// [`par(fg)`]: crate::par::Par::fg
        pub col: Option<ColorSpec> => option ColorSpec,
        /// Colour of the box when `col` isn't given (or is `None`). `None`
        /// or `ColorSpec::None`: [`par(col)`].
        ///
        /// [`par(col)`]: crate::par::Par::col
        pub fg: Option<ColorSpec> => option ColorSpec,
        /// Line type. `None`: solid, not [`par(lty)`] (R's `box()` passes
        /// `lty = "solid"`).
        ///
        /// [`par(lty)`]: crate::par::Par::lty
        pub lty: Option<LineType> => option LineType,
        /// Line width. `None`: [`par(lwd)`].
        ///
        /// [`par(lwd)`]: crate::par::Par::lwd
        pub lwd: Option<f64> => option f64,
        /// Which sides of the plot region to draw. `None`: [`par(bty)`].
        /// Ignored for the other regions, which always get all four sides.
        ///
        /// [`par(bty)`]: crate::par::Par::bty
        pub bty: Option<Bty> => option Bty,
        /// How line ends are drawn: `"round"`, `"butt"` or `"square"`.
        /// `None`: [`par(lend)`]; overrides it for this call only.
        ///
        /// [`par(lend)`]: crate::par::Par::lend
        pub lend: Option<LineEnd> => option LineEnd,
        /// How line corners are joined: `"round"`, `"mitre"` or `"bevel"`.
        /// `None`: [`par(ljoin)`]; overrides it for this call only.
        ///
        /// [`par(ljoin)`]: crate::par::Par::ljoin
        pub ljoin: Option<LineJoin> => option LineJoin,
        /// Mitre limit of mitred joins (at least 1). `None`: [`par(lmitre)`];
        /// overrides it for this call only.
        ///
        /// [`par(lmitre)`]: crate::par::Par::lmitre
        pub lmitre: Option<f64> => option f64,
    }
}

crate::setters::options! {
    /// Arguments of `plot.window()` besides the limits.
    #[derive(Clone, Debug, Default)]
    pub struct PlotWindowOptions => [PlotWindowCall] {
        /// Makes the x axis logarithmic. Default `false`. R's `log = "x"`.
        pub xlog: bool => value bool,
        /// Makes the y axis logarithmic. Default `false`. R's `log = "y"`.
        pub ylog: bool => value bool,
        /// Aspect ratio: one x unit is `asp` y units long on the page.
        /// Ignored on log axes, as in R. `None`: no constraint.
        pub asp: Option<f64> => option f64,
        /// How the x range is extended: `"r"` adds 4% at each end, `"i"`
        /// nothing. `None`: [`par(xaxs)`]; overrides it for this call only.
        ///
        /// [`par(xaxs)`]: crate::par::Par::xaxs
        pub xaxs: Option<AxisStyle> => option AxisStyle,
        /// As `xaxs`, for the y axis. `None`: [`par(yaxs)`].
        ///
        /// [`par(yaxs)`]: crate::par::Par::yaxs
        pub yaxs: Option<AxisStyle> => option AxisStyle,
        /// Approximate numbers of x and y ticks (and label length) for the
        /// default ticks this window sets. `None`: [`par(lab)`]; overrides it
        /// for this call only.
        ///
        /// [`par(lab)`]: crate::par::Par::lab
        pub lab: Option<[i32; 3]> => option [i32; 3],
    }
}

impl<B: Backend> Device<B> {
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
    pub(crate) fn points_impl(
        &mut self,
        x: &[f64],
        y: &[f64],
        opts: &XyOptions,
    ) -> Result<(), Error> {
        let mut dev = self.xy_pars(opts)?;
        dev.plot_xy(x, y, opts.type_.unwrap_or(PlotType::Points), opts);
        Ok(())
    }

    /// `points()`/`lines()`'s inline `lend`/`ljoin`/`lmitre` and `font`.
    fn xy_pars(&mut self, opts: &XyOptions) -> Result<super::ParScope<'_, B>, Error> {
        let mut dev = self.line_pars(opts.lend, opts.ljoin, opts.lmitre)?;
        if let Some(f) = opts.font {
            dev.par.font = f;
        }
        Ok(dev)
    }

    /// Draws R's `lines()`; the public entry is
    /// [`Graphics::lines`](crate::graphics::Graphics::lines), whose docs
    /// describe it.
    pub(crate) fn lines_impl(
        &mut self,
        x: &[f64],
        y: &[f64],
        opts: &XyOptions,
    ) -> Result<(), Error> {
        let mut dev = self.xy_pars(opts)?;
        dev.plot_xy(x, y, opts.type_.unwrap_or(PlotType::Lines), opts);
        Ok(())
    }

    /// `C_plotXY`: the lines for `type`, then the points for `"p"`, `"b"`
    /// and `"o"`.
    fn plot_xy(&mut self, x: &[f64], y: &[f64], type_: PlotType, opts: &XyOptions) {
        let clip = self.clip_for(opts.xpd);
        // The lines use the first `col` and `lwd`; an `NA` `lwd` (`None`)
        // is `par(lwd)` there, as in R.
        let lwd = pick_na(&opts.lwd, 0, self.par.lwd).unwrap_or(self.par.lwd);
        let lty = opts.lty.unwrap_or(self.par.lty);
        // `None`: no lines, or an `NA` colour.
        let has_lines = !matches!(
            type_,
            PlotType::Points | PlotType::Histogram | PlotType::None
        );
        let stroke = if has_lines {
            self.resolve(pick(&opts.col, 0, ColorSpec::Color(self.par.col)))
                .map(|c| self.stroke_of(c, lwd, lty))
        } else {
            None
        };
        let stroke = stroke.as_ref();
        match type_ {
            PlotType::Points => self.xy_points(x, y, opts, clip),
            PlotType::Lines | PlotType::Overplotted => {
                if let Some(stroke) = stroke {
                    self.polyline_xy(x, y, stroke, clip);
                }
                if type_ == PlotType::Overplotted {
                    self.xy_points(x, y, opts, clip);
                }
            }
            PlotType::BrokenLines | PlotType::Both => {
                if let Some(stroke) = stroke {
                    self.broken_lines_xy(x, y, stroke, clip);
                }
                if type_ == PlotType::Both {
                    self.xy_points(x, y, opts, clip);
                }
            }
            PlotType::Histogram => self.histogram_xy(x, y, &opts.col, lwd, lty, clip),
            PlotType::Steps | PlotType::StepsVerticalFirst => {
                if let Some(stroke) = stroke {
                    let vertical_first = type_ == PlotType::StepsVerticalFirst;
                    self.steps_xy(x, y, vertical_first, stroke, clip);
                }
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
            let Some(p) = self.finite_point(x, y) else {
                continue;
            };
            let Some(c) = col[i % col.len()] else {
                continue;
            };
            // An `NA` (`None`) `pch`, `cex` or `lwd` skips the point.
            let (Some(cex), Some(pch), Some(lwd)) = (
                pick_na(&opts.cex, i, 1.0),
                pick_na(&opts.pch, i, pch),
                pick_na(&opts.lwd, i, lwd),
            ) else {
                continue;
            };
            if !cex.is_finite() {
                continue;
            }
            let bg = if bg.is_empty() {
                None
            } else {
                bg[i % bg.len()]
            };
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
    ) -> Result<(), Error> {
        let mut dev = self.line_pars(opts.lend, opts.ljoin, opts.lmitre)?;
        dev.segments_draw(x0, y0, x1, y1, opts);
        Ok(())
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
            let (Some(p0), Some(p1)) = (
                self.finite_point(x0[i % x0.len()], y0[i % y0.len()]),
                self.finite_point(x1[i % x1.len()], y1[i % y1.len()]),
            ) else {
                continue;
            };
            // An `NA` colour, `lwd` or `lty` skips the segment.
            let (Some(c), Some(lwd), Some(lty)) = (
                self.resolve(pick(&opts.col, i, fg)),
                pick_na(&opts.lwd, i, lwd),
                pick_na(&opts.lty, i, lty),
            ) else {
                continue;
            };
            let stroke = self.stroke_of(c, lwd, lty);
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
    ) -> Result<(), Error> {
        let mut dev = self.line_pars(opts.lend, opts.ljoin, opts.lmitre)?;
        dev.arrows_draw(x0, y0, x1, y1, opts);
        Ok(())
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
            let (Some(from), Some(to)) = (self.finite_point(a, b), self.finite_point(c, d)) else {
                continue;
            };
            // An `NA` colour, `lwd` or `lty` skips the arrow.
            let (Some(color), Some(lwd), Some(lty)) = (
                self.resolve(pick(&opts.col, i, fg)),
                pick_na(&opts.lwd, i, lwd),
                pick_na(&opts.lty, i, lty),
            ) else {
                continue;
            };
            let stroke = self.stroke_of(color, lwd, lty);
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
    ) -> Result<(), Error> {
        let mut dev = self.line_pars(opts.lend, opts.ljoin, opts.lmitre)?;
        dev.rect_draw(x0, y0, x1, y1, opts);
        Ok(())
    }

    fn rect_draw(&mut self, x0: &[f64], y0: &[f64], x1: &[f64], y1: &[f64], opts: &RectOptions) {
        self.push_clip_region(self.clip_for(opts.xpd));
        let style = ShapeStyle {
            col: &opts.col,
            border: &opts.border,
            lty: &opts.lty,
            lwd: &opts.lwd,
            density: &opts.density,
            angle: &opts.angle,
            odd_even: false,
        };
        self.rects(x0, y0, x1, y1, &style);
        self.pop_clip();
    }

    /// The rectangles of `rect()`, without setting a clip. With shading
    /// (see [`shading`]), as R's `rect()`: each rectangle becomes a polygon
    /// (bottom-left, bottom-right, top-right, top-left) and goes to
    /// `polygon()`; otherwise `C_rect`: each is filled with `col` (empty:
    /// none) and outlined with `border` (empty: [`par(fg)`]).
    ///
    /// [`par(fg)`]: crate::par::Par::fg
    pub(crate) fn rects(&mut self, x0: &[f64], y0: &[f64], x1: &[f64], y1: &[f64], s: &ShapeStyle) {
        let n = recycled_len(&[x0.len(), y0.len(), x1.len(), y1.len()]);
        if shading(s.density) {
            // `rbind(NA, xleft, xright, xright, xleft)[-1]`, likewise for y.
            let (mut x, mut y) = (Vec::with_capacity(5 * n), Vec::with_capacity(5 * n));
            for i in 0..n {
                if i > 0 {
                    x.push(f64::NAN);
                    y.push(f64::NAN);
                }
                let (a, b) = (x0[i % x0.len()], x1[i % x1.len()]);
                let (c, d) = (y0[i % y0.len()], y1[i % y1.len()]);
                x.extend([a, b, b, a]);
                y.extend([c, c, d, d]);
            }
            self.polygons(&x, &y, s);
            return;
        }
        let fg = ColorSpec::Color(self.par.fg);
        let (lty, lwd) = (self.par.lty, self.par.lwd);
        for i in 0..n {
            let (Some(p0), Some(p1)) = (
                self.finite_point(x0[i % x0.len()], y0[i % y0.len()]),
                self.finite_point(x1[i % x1.len()], y1[i % y1.len()]),
            ) else {
                continue;
            };
            // An `NA` `lwd` or `lty` leaves out the border (R does this for
            // `lwd`), as an `NA` border colour does; the fill is drawn.
            let fill = self.resolve(pick(s.col, i, ColorSpec::None));
            let border = self.resolve(pick(s.border, i, fg));
            let line = pick_na(s.lwd, i, lwd).zip(pick_na(s.lty, i, lty));
            let stroke = border
                .zip(line)
                .map(|(c, (lwd, lty))| self.stroke_of(c, lwd, lty));
            self.backend
                .draw_rect(Rect::new(p0.x, p0.y, p1.x, p1.y), fill, stroke.as_ref());
        }
    }

    /// Draws R's `polygon()`; the public entry is
    /// [`Graphics::polygon`](crate::graphics::Graphics::polygon), whose docs
    /// describe it.
    pub(crate) fn polygon_impl(
        &mut self,
        x: &[f64],
        y: &[f64],
        opts: &PolygonOptions,
    ) -> Result<(), Error> {
        let mut dev = self.line_pars(opts.lend, opts.ljoin, opts.lmitre)?;
        dev.polygon_draw(x, y, opts);
        Ok(())
    }

    fn polygon_draw(&mut self, x: &[f64], y: &[f64], opts: &PolygonOptions) {
        self.push_clip_region(self.clip_for(opts.xpd));
        let lwd: Vec<Option<f64>> = opts.lwd.map(Some).into_iter().collect();
        let style = ShapeStyle {
            col: &opts.col,
            border: &opts.border,
            lty: &opts.lty,
            lwd: &lwd,
            density: &opts.density,
            angle: &opts.angle,
            odd_even: opts.fill_odd_even,
        };
        self.polygons(x, y, &style);
        self.pop_clip();
    }

    /// Draws R's `text()`; the public entry is
    /// [`Graphics::text`](crate::graphics::Graphics::text), whose docs
    /// describe it.
    pub(crate) fn text_impl(
        &mut self,
        x: &[f64],
        y: &[f64],
        labels: &[Option<Label>],
        opts: &TextOptions,
    ) -> Result<(), Error> {
        check_text(opts)?;
        let n = recycled_len(&[x.len(), y.len()]);
        let clip = self.clip_for(opts.xpd);
        let hadj = opts.adj.unwrap_or(self.par.adj);
        // `offset` characters, as `GConvertXUnits(offset, CHARS, INCHES)`
        // before any label's own `cex`: the character height (R's
        // `cra[1]`) times `par(cex)` as of the last layout (see
        // `layout_cex`).
        let offset = opts.offset.unwrap_or(0.5) * self.layout_cex * 1.2 * self.start_ps / 72.0;
        let (col, font) = (ColorSpec::Color(self.par.col), self.par.font);
        for i in 0..n {
            // An `NA` (`None`) label is skipped, as `C_text`.
            let label: Label = if labels.is_empty() {
                Label::Text((i + 1).to_string())
            } else {
                match &labels[i % labels.len()] {
                    Some(l) => l.clone(),
                    None => continue,
                }
            };
            // As `C_text`: a label at a non-finite position (`NA`, or <= 0
            // on a log axis) is skipped; the others keep their index.
            let Some(p) = self.finite_point(x[i % x.len()], y[i % y.len()]) else {
                continue;
            };
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
                // An `NA` `cex` or `font` is the default, as in R.
                pick_na(&opts.cex, i, 1.0).unwrap_or(1.0),
                pick_na(&opts.font, i, font).unwrap_or(font),
            );
        }
        Ok(())
    }

    /// Draws R's `box()`; the public entry is
    /// [`Graphics::box_`](crate::graphics::Graphics::box_), whose docs
    /// describe it.
    pub(crate) fn box_impl(&mut self, opts: &BoxOptions) -> Result<(), Error> {
        let mut dev = self.line_pars(opts.lend, opts.ljoin, opts.lmitre)?;
        dev.box_draw(opts);
        Ok(())
    }

    fn box_draw(&mut self, opts: &BoxOptions) {
        // `C_box`: a non-`NA` `col`, else a non-`NA` inline `fg`, else
        // `par(col)`.
        let given = |c: Option<ColorSpec>| c.filter(|c| *c != ColorSpec::None);
        let col = given(opts.col)
            .or(given(opts.fg))
            .unwrap_or(ColorSpec::Color(self.par.col));
        let Some(color) = self.resolve(col) else {
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

/// R's check of `text()`'s `pos`: each value 1 to 4.
pub(crate) fn check_text(opts: &TextOptions) -> Result<(), Error> {
    if !opts.pos.iter().all(|p| (1..=4).contains(p)) {
        return Err(Error::invalid("invalid 'pos' value"));
    }
    Ok(())
}
