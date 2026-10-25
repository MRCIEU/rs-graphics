// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! `plot(x, y, ...)`, ported from `plot.default` in
//! `r-source/src/library/graphics/R/plot.R`, restricted to numeric x/y
//! scatter/line plots (the vast majority of `plot.default` is dispatch
//! logic and argument munging for other input types, which is out of
//! scope here).

use crate::backend::{Backend, LineEnd};
use crate::color::ColorSpec;
use crate::device::{AxisOptions, BoxOptions, Device, PlotWindowOptions, TitleOptions, XyOptions};
use crate::font::FontFace;
use crate::lty::LineType;
use crate::par::{AxisStyle, AxisType, Bty, Xpd};
use crate::plotmath::Label;
use crate::recording::Figure;

use super::{InlineAxis, data_range};

/// `plot(..., type = )` (and `points()`/`lines()`'s `type`): every type
/// R has. Lines (all but `"p"` and `"n"`) take the first `col`, `lwd`
/// and `lty`, except `"h"`, whose lines recycle `col`. Lines break at
/// `NA`s (and at values <= 0 on a log axis).
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
    /// R's `type =` letters `"p"`, `"l"`, `"b"`, `"c"`, `"o"`, `"h"`,
    /// `"s"`, `"S"` and `"n"`; `None` for anything else (R: "invalid plot
    /// type").
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
        /// What to draw (R's `type`, see [`PlotType`]); `None`: points
        /// (`"p"`).
        pub type_: Option<PlotType> => option PlotType,
        /// Plotting symbols, recycled per point (a negative value is minus a
        /// Unicode code point, as in R; the setter takes characters too);
        /// empty: `par(pch)`.
        /// `None` (R's `NA`): no point there.
        pub pch: Vec<Option<i32>> => na_values i32,
        /// Colours, recycled per point (lines use the first, except
        /// `type = "h"`'s, one per point); empty: `par(col)`.
        pub col: Vec<ColorSpec> => values ColorSpec,
        /// Fill of symbols 21-25, recycled per point; empty: none (R's `NA`).
        pub bg: Vec<ColorSpec> => values ColorSpec,
        /// Line widths, recycled per point (lines use the first); empty:
        /// `par(lwd)`.
        /// `None` (R's `NA`): no point there (the lines use `par(lwd)`).
        pub lwd: Vec<Option<f64>> => na_values f64,
        /// Line type of lines; `None`: `par(lty)`.
        pub lty: Option<LineType> => option LineType,
        /// Symbol sizes, recycled per point, times `par(cex)`; empty: 1.
        /// `None` (R's `NA`): no point there.
        pub cex: Vec<Option<f64>> => na_values f64,
        /// x limits; `None`: the finite data range, like R's
        /// `plot.window(xlim = range(x))` (positive values only on a log
        /// axis).
        pub xlim: Option<(f64, f64)> => pair f64,
        /// y limits; `None`: the finite data range, as `xlim`.
        pub ylim: Option<(f64, f64)> => pair f64,
        /// Log x axis (R's `log = "x"`); default `false`.
        pub xlog: bool => value bool,
        /// Log y axis (R's `log = "y"`); default `false`.
        pub ylog: bool => value bool,
        /// Main title, drawn by `title()`: text (`"Weight"`) or a plotmath
        /// expression (`Label::math("x^2")`); `None`: none.
        pub main: Option<Label> => option Label,
        /// Subtitle; `None`: none.
        pub sub: Option<Label> => option Label,
        /// x axis label; `None`: none. (R defaults `xlab`/`ylab` to the
        /// deparsed variable names passed as `x`/`y`; Rust has no equivalent,
        /// so pass explicit labels.)
        pub xlab: Option<Label> => option Label,
        /// y axis label; `None`: none (see `xlab`).
        pub ylab: Option<Label> => option Label,
        /// Size of `main`, relative to `par(cex)` (R's `cex.main`, passed
        /// to `title()`); `None`: `par(cex.main)`.
        pub cex_main: Option<f64> => option f64,
        /// Colour of `main` (R's `col.main`, passed to `title()`);
        /// `None`: `par(col.main)`.
        pub col_main: Option<ColorSpec> => option ColorSpec,
        /// Font of `main` (a [`FontFace`]; R's
        /// `font.main`, passed to `title()`); `None`: `par(font.main)`.
        pub font_main: Option<FontFace> => option FontFace,
        /// Size of `sub`, relative to `par(cex)` (R's `cex.sub`, passed
        /// to `title()`); `None`: `par(cex.sub)`.
        pub cex_sub: Option<f64> => option f64,
        /// Colour of `sub` (R's `col.sub`, passed to `title()`);
        /// `None`: `par(col.sub)`.
        pub col_sub: Option<ColorSpec> => option ColorSpec,
        /// Font of `sub` (a [`FontFace`]; R's
        /// `font.sub`, passed to `title()`); `None`: `par(font.sub)`.
        pub font_sub: Option<FontFace> => option FontFace,
        /// Size of `xlab` and `ylab`, relative to `par(cex)` (R's `cex.lab`, passed
        /// to `title()`); `None`: `par(cex.lab)`.
        pub cex_lab: Option<f64> => option f64,
        /// Colour of `xlab` and `ylab` (R's `col.lab`, passed to `title()`);
        /// `None`: `par(col.lab)`.
        pub col_lab: Option<ColorSpec> => option ColorSpec,
        /// Font of `xlab` and `ylab` (a [`FontFace`]; R's
        /// `font.lab`, passed to `title()`); `None`: `par(font.lab)`.
        pub font_lab: Option<FontFace> => option FontFace,
        /// Draw the axes and the box around the plot; default `true`.
        pub axes: bool => value bool,
        /// Which sides of the box to draw (R's inline `bty`, passed to
        /// `box()`); `None`: `par(bty)`.
        pub bty: Option<Bty> => option Bty,
        /// Inline `xaxt` (`plot(x, y, xaxt = "n")`): `AxisType::None` leaves
        /// out the x axis; `None`: `par(xaxt)`.
        pub xaxt: Option<AxisType> => option AxisType,
        /// Inline `yaxt`, as `xaxt`; `None`: `par(yaxt)`.
        pub yaxt: Option<AxisType> => option AxisType,
        /// Inline `xaxp` (`plot(x, y, xaxp = c(1, 10, 3))`): the default ticks
        /// of the x axis; `None`: `par(xaxp)` as `plot_window()` sets it. Like
        /// R, `par()` itself is left unchanged.
        pub xaxp: Option<[f64; 3]> => array [f64; 3],
        /// Inline `yaxp`, as `xaxp`; `None`: `par(yaxp)`.
        pub yaxp: Option<[f64; 3]> => array [f64; 3],
        /// Axis label orientation (R's inline `las`, passed to `axis()`): 0
        /// parallel to the axis, 1 horizontal, 2 perpendicular, 3 vertical;
        /// `None`: `par(las)`.
        pub las: Option<i32> => option i32,
        /// Axis label size (R's inline `cex.axis`); `None`: `par(cex.axis)`.
        pub cex_axis: Option<f64> => option f64,
        /// Axis label colour (R's inline `col.axis`); `None`: `par(col.axis)`.
        pub col_axis: Option<ColorSpec> => option ColorSpec,
        /// Axis label font (R's inline `font.axis`); `None`: `par(font.axis)`.
        pub font_axis: Option<FontFace> => option FontFace,
        /// Margin lines of the axis titles, labels and line (R's inline `mgp`,
        /// passed to `axis()` and `title()`); `None`: `par(mgp)`.
        pub mgp: Option<[f64; 3]> => array [f64; 3],
        /// Tick length as a fraction of the plot region (R's inline `tck`);
        /// `None`: `par(tck)`.
        pub tck: Option<f64> => option f64,
        /// Tick length in margin lines (R's inline `tcl`); `None`: `par(tcl)`.
        pub tcl: Option<f64> => option f64,
        /// R's `asp`: one x unit is `asp` y units long (`plot(x, y, asp = 1)`
        /// for equal scales); ignored on log axes. `None` (R's `NA`): none.
        pub asp: Option<f64> => option f64,
        /// x axis style (R's inline `xaxs`, passed to `plot.window()`): `"r"`
        /// adds 4% at each end, `"i"` none; `None`: `par(xaxs)`.
        pub xaxs: Option<AxisStyle> => option AxisStyle,
        /// As `xaxs`, for the y axis; `None`: `par(yaxs)`.
        pub yaxs: Option<AxisStyle> => option AxisStyle,
        /// Approximate numbers of x and y ticks (R's inline `lab`, passed to
        /// `plot.window()`); `None`: `par(lab)`.
        pub lab: Option<[i32; 3]> => option [i32; 3],
        /// Drawn after the window is set and before the points (R's
        /// `panel.first`, e.g. grid lines): a [`Figure`] of low-level calls,
        /// added with [`Device::add_figure`](crate::device::Device::add_figure)
        /// (so its `par()` changes don't last). `None`: nothing.
        pub panel_first: Option<Figure> => option Figure,
        /// As `panel_first`, drawn after the points and before the axes (R's
        /// `panel.last`).
        pub panel_last: Option<Figure> => option Figure,
        /// Draw the box around the plot region (R's `frame.plot`, shaped by
        /// `bty`); `None`: `axes`.
        pub frame_plot: Option<bool> => option bool,
        /// R's inline `font`: the font of character symbols (`pch = "a"`) and of
        /// the axis labels (over `font_axis`); `None`: `par(font)` for symbols,
        /// `font_axis` for the axes.
        pub font: Option<FontFace> => option FontFace,
        /// R's inline `fg`: the colour of the axes' line and ticks and of the
        /// frame; `None`: `par(fg)` for the axes, `par(col)` for the frame.
        pub fg: Option<ColorSpec> => option ColorSpec,
        /// R's inline `adj`: where the titles sit along their sides (0 left, 1
        /// right); `None`: `par(adj)`.
        pub adj: Option<f64> => option f64,
        /// R's inline `lend`: the line ends of the lines and symbols, the axes
        /// and the frame; `None`: `par(lend)`.
        pub lend: Option<LineEnd> => option LineEnd,
        /// R's inline `xpd`: where the points and lines clip, how far the axes'
        /// ticks may reach, and the titles' clip (at least the figure region);
        /// `None`: `par(xpd)`.
        pub xpd: Option<Xpd> => option Xpd,
    }
}

impl Default for PlotOptions {
    fn default() -> Self {
        PlotOptions {
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
    pub(crate) fn plot_impl(&mut self, x: &[f64], y: &[f64], opts: &PlotOptions) {
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

        self.plot_new_impl();
        // `localWindow(xlim, ylim, log, asp, ...)`.
        let window = PlotWindowOptions {
            xlog: opts.xlog,
            ylog: opts.ylog,
            asp: opts.asp,
            xaxs: opts.xaxs,
            yaxs: opts.yaxs,
            lab: opts.lab,
        };
        self.plot_window_impl(xlim, ylim, &window);
        if let Some(panel) = &opts.panel_first {
            self.add_figure(panel);
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
        self.points_impl(x, y, &xy);
        if let Some(panel) = &opts.panel_last {
            self.add_figure(panel);
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
                self.axis_impl(side, &inline.apply(o));
            }
        }
        // `if (frame.plot) localBox(...)`: `box()` with the inline `bty`.
        if opts.frame_plot.unwrap_or(opts.axes) {
            self.box_impl(&BoxOptions {
                bty: opts.bty,
                fg: opts.fg,
                lend: opts.lend,
                ..BoxOptions::default()
            });
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
    }
}
