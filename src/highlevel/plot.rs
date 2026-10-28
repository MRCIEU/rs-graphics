// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Scatter and line plots of numeric x and y: the options of
//! [`Graphics::plot`](crate::graphics::Graphics::plot) and the plot types
//! ([`PlotType`]) it shares with `points()` and `lines()`.
//!
//! Only numeric x/y plots are supported: the vast majority of R's
//! `plot.default` is dispatch logic and argument munging for other input
//! types, which is out of scope here.
//!
//! This is R's `plot(x, y, ...)`, ported from `plot.default` in
//! `r-source/src/library/graphics/R/plot.R`.

use crate::backend::{Backend, LineEnd};
use crate::color::ColorSpec;
use crate::device::{AxisOptions, BoxOptions, Device, PlotWindowOptions, TitleOptions, XyOptions};
use crate::error::Error;
use crate::font::FontFace;
use crate::lty::LineType;
use crate::par::{AxisStyle, AxisType, Bty, Xpd};
use crate::plotmath::Label;
use crate::recording::Figure;

use super::{InlineAxis, data_range};

/// What a plot draws for its points: points, lines, both, vertical lines,
/// stair steps or nothing.
///
/// Used by `plot()`, `points()` and `lines()`; every type R has is here.
/// Lines (all but `"p"` and `"n"`) take the first `col`, `lwd` and `lty`,
/// except `"h"`, whose lines recycle `col`. Lines break at `NA`s (and at
/// values <= 0 on a log axis). Setters also take R's letters
/// (`.type_("b")`); any other string is an error of the call.
///
/// This is R's `type` argument.
///
/// # Examples
///
/// ```
/// use rs_graphics::prelude::*;
///
/// # let mut dev = Device::null().open().unwrap();
/// let y = [3, 1, 4, 1, 5];
/// dev.plot().x(&y).type_(PlotType::Both).draw();
/// dev.lines().x(&y).type_("s").draw();
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlotType {
    /// `"p"`: points (`plot()`'s default).
    Points,
    /// `"l"`: lines joining the points.
    Lines,
    /// `"b"`: both, points with the lines between them stopping short of
    /// each point.
    Both,
    /// `"c"`: the lines of `"b"` without the points.
    BrokenLines,
    /// `"o"`: overplotted, the line drawn straight through with points on
    /// top.
    Overplotted,
    /// `"h"`: a vertical line from each point to y = 0, or to the bottom
    /// of the plot on a log y axis ("histogram-like" or "high-density"
    /// lines, e.g. `plot(table(x))`).
    Histogram,
    /// `"s"`: stair steps, moving across to each point's x, then up or
    /// down to its y (so each y value holds until the next x, as an
    /// empirical CDF or survival curve).
    Steps,
    /// `"S"`: the other steps, moving up or down to each point's y, then
    /// across to its x.
    StepsVerticalFirst,
    /// `"n"`: nothing; `plot()` still sets up the window, axes and titles,
    /// so points or lines can be added afterwards.
    None,
}

impl PlotType {
    /// Reads a plot type from its letter: `"p"`, `"l"`, `"b"`, `"c"`,
    /// `"o"`, `"h"`, `"s"`, `"S"` or `"n"`; `None` for anything else (R
    /// errors: "invalid plot type").
    ///
    /// These are R's `type =` letters.
    ///
    /// # Examples
    ///
    /// ```
    /// use rs_graphics::prelude::*;
    ///
    /// assert_eq!(PlotType::parse("S"), Some(PlotType::StepsVerticalFirst));
    /// assert_eq!(PlotType::parse("x"), None);
    /// ```
    pub fn parse(s: &str) -> Option<PlotType> {
        match s {
            "p" => Some(PlotType::Points),
            "l" => Some(PlotType::Lines),
            "b" => Some(PlotType::Both),
            "c" => Some(PlotType::BrokenLines),
            "o" => Some(PlotType::Overplotted),
            "h" => Some(PlotType::Histogram),
            "s" => Some(PlotType::Steps),
            "S" => Some(PlotType::StepsVerticalFirst),
            "n" => Some(PlotType::None),
            _ => None,
        }
    }
}

crate::error::from_str_via_parse!(PlotType, "plot type");
crate::error::from_r_string!(
    PlotType,
    "plot type",
    "one of \"p\", \"l\", \"b\", \"c\", \"o\", \"h\", \"s\", \"S\" or \"n\""
);

crate::setters::options! {
    /// Options for [`Graphics::plot`](crate::graphics::Graphics::plot), covering the subset of
    /// `plot.default`'s arguments this crate implements. Unset fields are R's
    /// defaults, given for each.
    #[derive(Clone, Debug)]
    pub struct PlotOptions => [PlotCall] {
        /// What to draw: points, lines, both, steps, ... (see [`PlotType`]).
        /// `None`: points. R's `type`.
        pub type_: Option<PlotType> => option PlotType,
        /// Plotting symbols, recycled over the points; characters work too
        /// (a negative value is minus a Unicode code point). Empty:
        /// [`par(pch)`]; `None`: no point there.
        ///
        /// [`par(pch)`]: crate::par::Par::pch
        pub pch: Vec<Option<i32>> => na_values i32,
        /// Colours, recycled over the points. Lines use the first, except
        /// `type = "h"`, which has one line per point. Empty: [`par(col)`].
        ///
        /// [`par(col)`]: crate::par::Par::col
        pub col: Vec<ColorSpec> => values ColorSpec,
        /// Fill of symbols 21-25, recycled over the points. Empty: none (R's
        /// `NA`).
        pub bg: Vec<ColorSpec> => values ColorSpec,
        /// Line widths, recycled over the points; lines use the first.
        /// Empty: [`par(lwd)`]; `None`: no point there (lines use [`par(lwd)`]).
        ///
        /// [`par(lwd)`]: crate::par::Par::lwd
        pub lwd: Vec<Option<f64>> => na_values f64,
        /// Line type of the lines. `None`: [`par(lty)`].
        ///
        /// [`par(lty)`]: crate::par::Par::lty
        pub lty: Option<LineType> => option LineType,
        /// Symbol sizes, recycled over the points, times [`par(cex)`]. Empty:
        /// 1; `None`: no point there.
        ///
        /// [`par(cex)`]: crate::par::Par::cex
        pub cex: Vec<Option<f64>> => na_values f64,
        /// x axis limits. `None`: the range of the finite data (positive
        /// values only on a log axis), as R's `xlim = range(x)`.
        pub xlim: Option<(f64, f64)> => pair f64,
        /// y axis limits. `None`: the range of the finite data, as for
        /// `xlim`.
        pub ylim: Option<(f64, f64)> => pair f64,
        /// Makes the x axis logarithmic. Default `false`. R's `log = "x"`.
        pub xlog: bool => value bool,
        /// Makes the y axis logarithmic. Default `false`. R's `log = "y"`.
        pub ylog: bool => value bool,
        /// Main title: text (`"Weight"`) or a plotmath expression
        /// (`Label::math("x^2")`), drawn by `title()`. `None`: none.
        pub main: Option<Label> => option Label,
        /// Subtitle. `None`: none.
        pub sub: Option<Label> => option Label,
        /// x axis label. `None`: none. Unlike R, there is no default from
        /// the variable name passed as `x`, so pass a label explicitly.
        pub xlab: Option<Label> => option Label,
        /// y axis label. `None`: none (no default from the variable name, as
        /// for `xlab`).
        pub ylab: Option<Label> => option Label,
        /// Size of the main title, relative to [`par(cex)`]. `None`:
        /// [`par(cex.main)`]. R's `cex.main`.
        ///
        /// [`par(cex)`]: crate::par::Par::cex
        /// [`par(cex.main)`]: crate::par::Par::cex_main
        pub cex_main: Option<f64> => number f64,
        /// Colour of the main title. `None`: [`par(col.main)`]. R's
        /// `col.main`.
        ///
        /// [`par(col.main)`]: crate::par::Par::col_main
        pub col_main: Option<ColorSpec> => option ColorSpec,
        /// Font of the main title (a [`FontFace`]). `None`:
        /// [`par(font.main)`]. R's `font.main`.
        ///
        /// [`par(font.main)`]: crate::par::Par::font_main
        pub font_main: Option<FontFace> => option FontFace,
        /// Size of the subtitle, relative to [`par(cex)`]. `None`:
        /// [`par(cex.sub)`]. R's `cex.sub`.
        ///
        /// [`par(cex)`]: crate::par::Par::cex
        /// [`par(cex.sub)`]: crate::par::Par::cex_sub
        pub cex_sub: Option<f64> => number f64,
        /// Colour of the subtitle. `None`: [`par(col.sub)`]. R's `col.sub`.
        ///
        /// [`par(col.sub)`]: crate::par::Par::col_sub
        pub col_sub: Option<ColorSpec> => option ColorSpec,
        /// Font of the subtitle (a [`FontFace`]). `None`: [`par(font.sub)`].
        /// R's `font.sub`.
        ///
        /// [`par(font.sub)`]: crate::par::Par::font_sub
        pub font_sub: Option<FontFace> => option FontFace,
        /// Size of the axis titles (`xlab` and `ylab`), relative to
        /// [`par(cex)`]. `None`: [`par(cex.lab)`]. R's `cex.lab`.
        ///
        /// [`par(cex)`]: crate::par::Par::cex
        /// [`par(cex.lab)`]: crate::par::Par::cex_lab
        pub cex_lab: Option<f64> => number f64,
        /// Colour of the axis titles (`xlab` and `ylab`). `None`:
        /// [`par(col.lab)`]. R's `col.lab`.
        ///
        /// [`par(col.lab)`]: crate::par::Par::col_lab
        pub col_lab: Option<ColorSpec> => option ColorSpec,
        /// Font of the axis titles (`xlab` and `ylab`; a [`FontFace`]).
        /// `None`: [`par(font.lab)`]. R's `font.lab`.
        ///
        /// [`par(font.lab)`]: crate::par::Par::font_lab
        pub font_lab: Option<FontFace> => option FontFace,
        /// Whether to draw the axes and the box around the plot. Default
        /// `true`.
        pub axes: bool => value bool,
        /// Which sides of the box to draw. `None`: [`par(bty)`].
        ///
        /// [`par(bty)`]: crate::par::Par::bty
        pub bty: Option<Bty> => option Bty,
        /// Whether to draw the x axis: `AxisType::None` leaves it out.
        /// `None`: [`par(xaxt)`].
        ///
        /// [`par(xaxt)`]: crate::par::Par::xaxt
        pub xaxt: Option<AxisType> => option AxisType,
        /// Whether to draw the y axis, as for `xaxt`. `None`: [`par(yaxt)`].
        ///
        /// [`par(yaxt)`]: crate::par::Par::yaxt
        pub yaxt: Option<AxisType> => option AxisType,
        /// Default ticks of the x axis, as `[from, to, intervals]`. `None`:
        /// [`par(xaxp)`] as `plot_window()` sets it. Like R, leaves `par()`
        /// itself unchanged.
        ///
        /// [`par(xaxp)`]: crate::par::Par::xaxp
        pub xaxp: Option<[f64; 3]> => array [f64; 3],
        /// Default ticks of the y axis, as for `xaxp`. `None`: [`par(yaxp)`].
        ///
        /// [`par(yaxp)`]: crate::par::Par::yaxp
        pub yaxp: Option<[f64; 3]> => array [f64; 3],
        /// Orientation of the axis labels: 0 parallel to the axis, 1
        /// horizontal, 2 perpendicular, 3 vertical. `None`: [`par(las)`].
        ///
        /// [`par(las)`]: crate::par::Par::las
        pub las: Option<i32> => option i32,
        /// Size of the axis labels. `None`: [`par(cex.axis)`]. R's `cex.axis`.
        ///
        /// [`par(cex.axis)`]: crate::par::Par::cex_axis
        pub cex_axis: Option<f64> => number f64,
        /// Colour of the axis labels. `None`: [`par(col.axis)`]. R's
        /// `col.axis`.
        ///
        /// [`par(col.axis)`]: crate::par::Par::col_axis
        pub col_axis: Option<ColorSpec> => option ColorSpec,
        /// Font of the axis labels. `None`: [`par(font.axis)`]. R's
        /// `font.axis`.
        ///
        /// [`par(font.axis)`]: crate::par::Par::font_axis
        pub font_axis: Option<FontFace> => option FontFace,
        /// Margin lines of the axis titles, axis labels and axis line, for
        /// the axes and titles. `None`: [`par(mgp)`].
        ///
        /// [`par(mgp)`]: crate::par::Par::mgp
        pub mgp: Option<[f64; 3]> => array [f64; 3],
        /// Tick length as a fraction of the plot region. `None`: [`par(tck)`].
        ///
        /// [`par(tck)`]: crate::par::Par::tck
        pub tck: Option<f64> => number f64,
        /// Tick length in margin lines. `None`: [`par(tcl)`].
        ///
        /// [`par(tcl)`]: crate::par::Par::tcl
        pub tcl: Option<f64> => number f64,
        /// Aspect ratio: one x unit is `asp` y units long (1 for equal
        /// scales); ignored on log axes. `None` (R's `NA`): none.
        pub asp: Option<f64> => number f64,
        /// Style of the x axis range: `"r"` adds 4% at each end, `"i"`
        /// nothing. `None`: [`par(xaxs)`].
        ///
        /// [`par(xaxs)`]: crate::par::Par::xaxs
        pub xaxs: Option<AxisStyle> => option AxisStyle,
        /// Style of the y axis range, as for `xaxs`. `None`: [`par(yaxs)`].
        ///
        /// [`par(yaxs)`]: crate::par::Par::yaxs
        pub yaxs: Option<AxisStyle> => option AxisStyle,
        /// Approximate numbers of x and y ticks, used when setting up the
        /// window. `None`: [`par(lab)`].
        ///
        /// [`par(lab)`]: crate::par::Par::lab
        pub lab: Option<[i32; 3]> => option [i32; 3],
        /// Low-level calls (a [`Figure`]) drawn after the window is set and
        /// before the points, e.g. grid lines; added with
        /// [`Device::add_figure`](crate::device::Device::add_figure), so its
        /// `par()` changes don't last. `None`: nothing. R's `panel.first`.
        pub panel_first: Option<Figure> => option Figure,
        /// As `panel_first`, but drawn after the points and before the axes.
        /// R's `panel.last`.
        pub panel_last: Option<Figure> => option Figure,
        /// Whether to draw the box around the plot region (shaped by `bty`).
        /// `None`: as `axes`. R's `frame.plot`.
        pub frame_plot: Option<bool> => option bool,
        /// Font of character symbols (`pch = "a"`) and of the axis labels
        /// (over `font_axis`). `None`: [`par(font)`] for symbols, `font_axis`
        /// for the axes.
        ///
        /// [`par(font)`]: crate::par::Par::font
        pub font: Option<FontFace> => option FontFace,
        /// Colour of the axes' line and ticks and of the frame. `None`:
        /// [`par(fg)`] for the axes, [`par(col)`] for the frame.
        ///
        /// [`par(col)`]: crate::par::Par::col
        /// [`par(fg)`]: crate::par::Par::fg
        pub fg: Option<ColorSpec> => option ColorSpec,
        /// Position of the titles along their sides (0 left, 1 right).
        /// `None`: [`par(adj)`].
        ///
        /// [`par(adj)`]: crate::par::Par::adj
        pub adj: Option<f64> => number f64,
        /// Line ends of the lines and symbols, the axes and the frame.
        /// `None`: [`par(lend)`].
        ///
        /// [`par(lend)`]: crate::par::Par::lend
        pub lend: Option<LineEnd> => option LineEnd,
        /// Clipping: where the points and lines are clipped, how far the
        /// axes' ticks may reach, and the titles' clip (at least the figure
        /// region). `None`: [`par(xpd)`].
        ///
        /// [`par(xpd)`]: crate::par::Par::xpd
        pub xpd: Option<Xpd> => option Xpd,
    }
}

impl Default for PlotOptions {
    fn default() -> Self {
        PlotOptions {
            deferred_error: None,
            type_: None,
            pch: Vec::new(),
            col: Vec::new(),
            bg: Vec::new(),
            lwd: Vec::new(),
            lty: None,
            cex: Vec::new(),
            xlim: None,
            ylim: None,
            xlog: false,
            ylog: false,
            main: None,
            sub: None,
            xlab: None,
            ylab: None,
            cex_main: None,
            col_main: None,
            font_main: None,
            cex_sub: None,
            col_sub: None,
            font_sub: None,
            cex_lab: None,
            col_lab: None,
            font_lab: None,
            axes: true,
            bty: None,
            xaxt: None,
            yaxt: None,
            xaxp: None,
            yaxp: None,
            las: None,
            cex_axis: None,
            col_axis: None,
            font_axis: None,
            mgp: None,
            tck: None,
            tcl: None,
            asp: None,
            xaxs: None,
            yaxs: None,
            lab: None,
            panel_first: None,
            panel_last: None,
            frame_plot: None,
            font: None,
            fg: None,
            adj: None,
            lend: None,
            xpd: None,
        }
    }
}

impl<B: Backend> Device<B> {
    /// Draws R's `plot()`; the public entry is
    /// [`Graphics::plot`](crate::graphics::Graphics::plot), whose docs
    /// describe it.
    pub(crate) fn plot_impl(
        &mut self,
        x: &[f64],
        y: &[f64],
        opts: &PlotOptions,
    ) -> Result<(), Error> {
        // On a log axis, values <= 0 are dropped (R's `xy.coords()` makes
        // them `NA`); `points()`/`lines()` then skip them.
        let range = |v: &[f64], log: bool| {
            if log {
                let positive: Vec<f64> = v.iter().copied().filter(|&a| a > 0.0).collect();
                data_range(&positive)
            } else {
                data_range(v)
            }
        };
        let xlim = opts.xlim.unwrap_or_else(|| range(x, opts.xlog));
        let ylim = opts.ylim.unwrap_or_else(|| range(y, opts.ylog));

        self.plot_new_impl()?;
        // `localWindow(xlim, ylim, log, asp, ...)`.
        let window = PlotWindowOptions {
            deferred_error: None,
            xlog: opts.xlog,
            ylog: opts.ylog,
            asp: opts.asp,
            xaxs: opts.xaxs,
            yaxs: opts.yaxs,
            lab: opts.lab,
        };
        self.plot_window_impl(xlim, ylim, &window);
        if let Some(panel) = &opts.panel_first {
            self.try_add_figure(panel)?;
        }

        // `plot.xy(xy, type, ...)`.
        let xy = XyOptions {
            type_: opts.type_,
            pch: opts.pch.clone(),
            col: opts.col.clone(),
            bg: opts.bg.clone(),
            cex: opts.cex.clone(),
            lwd: opts.lwd.clone(),
            lty: opts.lty,
            font: opts.font,
            lend: opts.lend,
            xpd: opts.xpd,
            ..XyOptions::default()
        };
        self.points_impl(x, y, &xy)?;
        if let Some(panel) = &opts.panel_last {
            self.try_add_figure(panel)?;
        }

        if opts.axes {
            // `localAxis()`: `axis(side, ...)` with the inline axis pars
            // (`col`, `lwd`, ... are the points', so they don't reach it;
            // `fg` is `axis()`'s `col`).
            let inline = InlineAxis {
                las: opts.las,
                cex_axis: opts.cex_axis,
                col_axis: opts.col_axis,
                font_axis: opts.font_axis,
                mgp: opts.mgp,
                tck: opts.tck,
                tcl: opts.tcl,
                font: opts.font,
                col: opts.fg,
                lend: opts.lend,
                xpd: opts.xpd,
                ..InlineAxis::default()
            };
            for (side, axt, axp) in [(1, opts.xaxt, opts.xaxp), (2, opts.yaxt, opts.yaxp)] {
                let o = AxisOptions {
                    axt,
                    axp,
                    ..AxisOptions::default()
                };
                self.axis_impl(side, &inline.apply(o))?;
            }
        }
        // `if (frame.plot) localBox(...)`: `box()` with the inline `bty`.
        if opts.frame_plot.unwrap_or(opts.axes) {
            self.box_impl(&BoxOptions {
                bty: opts.bty,
                fg: opts.fg,
                lend: opts.lend,
                ..BoxOptions::default()
            })?;
        }

        if self.par.ann {
            self.title_impl(&TitleOptions {
                main: opts.main.clone(),
                sub: opts.sub.clone(),
                xlab: opts.xlab.clone(),
                ylab: opts.ylab.clone(),
                cex_main: opts.cex_main,
                col_main: opts.col_main,
                font_main: opts.font_main,
                cex_sub: opts.cex_sub,
                col_sub: opts.col_sub,
                font_sub: opts.font_sub,
                cex_lab: opts.cex_lab,
                col_lab: opts.col_lab,
                font_lab: opts.font_lab,
                mgp: opts.mgp,
                adj: opts.adj,
                xpd: opts.xpd,
                ..TitleOptions::default()
            });
        }
        Ok(())
    }
}
