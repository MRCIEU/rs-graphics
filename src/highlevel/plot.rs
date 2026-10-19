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

use crate::backend::Backend;
use crate::color::ColorSpec;
use crate::device::{AxisOptions, BoxOptions, Device, PlotWindowOptions, TitleOptions, XyOptions};
use crate::lty::LineType;
use crate::par::AxisType;
use crate::plotmath::Label;

use super::data_range;

/// `plot(..., type = )`. Only the core types are implemented: `"p"`
/// (points, the default), `"l"` (lines), `"b"` (both: points, with the
/// lines between them stopping short of each point), `"c"` (those broken
/// lines alone), `"o"` (overplotted: the line drawn straight through,
/// points on top) and `"n"`
/// (set up the axes/window but plot nothing, so points/lines can be added
/// afterwards).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlotType {
    Points,
    Lines,
    Both,
    /// `"c"`: the lines of `"b"` without the points.
    BrokenLines,
    Overplotted,
    None,
}

impl PlotType {
    /// R's `type =` letters `"p"`, `"l"`, `"b"`, `"c"`, `"o"` and `"n"`.
    /// `None` for anything else, including R's `"h"`, `"s"` and `"S"`,
    /// which aren't implemented.
    pub fn parse(s: &str) -> Option<PlotType> {
        match s {
            "p" => Some(PlotType::Points),
            "l" => Some(PlotType::Lines),
            "b" => Some(PlotType::Both),
            "c" => Some(PlotType::BrokenLines),
            "o" => Some(PlotType::Overplotted),
            "n" => Some(PlotType::None),
            _ => None,
        }
    }
}

crate::error::from_str_via_parse!(PlotType, "plot type");

/// Options for [`Graphics::plot`](crate::graphics::Graphics::plot), covering the subset of `plot.default`'s
/// arguments this crate implements.
#[derive(Clone, Debug)]
pub struct PlotOptions {
    pub type_: PlotType,
    /// Symbols, colours, fills, sizes and widths as for
    /// [`Graphics::points`](crate::graphics::Graphics::points): recycled per point, empty for the default
    /// (`par()`; no fill; size 1).
    pub pch: Vec<i32>,
    pub col: Vec<ColorSpec>,
    pub bg: Vec<ColorSpec>,
    pub lwd: Vec<f64>,
    /// `None`: `par(lty)`.
    pub lty: Option<LineType>,
    pub cex: Vec<f64>,
    /// Defaults to the finite data range, like R's `plot.window(xlim =
    /// range(x))` (positive values only on a log axis).
    pub xlim: Option<(f64, f64)>,
    pub ylim: Option<(f64, f64)>,
    pub xlog: bool,
    pub ylog: bool,
    /// Titles, drawn by `title()`: text (`Some("Weight".into())`) or a
    /// plotmath expression (`Some(Label::math("x^2"))`).
    pub main: Option<Label>,
    pub sub: Option<Label>,
    /// R defaults `xlab`/`ylab` to the deparsed variable names passed as
    /// `x`/`y`; we have no equivalent, so these default to `None` (no
    /// label) and callers should pass explicit labels.
    pub xlab: Option<Label>,
    pub ylab: Option<Label>,
    pub axes: bool,
    /// Inline `xaxt`/`yaxt` (`plot(x, y, xaxt = "n")`): `AxisType::None`
    /// leaves out that axis; `None` uses `par(xaxt/yaxt)`.
    pub xaxt: Option<AxisType>,
    pub yaxt: Option<AxisType>,
    /// Inline `xaxp`/`yaxp` (`plot(x, y, xaxp = c(1, 10, 3))`): the default
    /// ticks of that axis; `None` uses `par(xaxp/yaxp)` as `plot_window()`
    /// sets it. Like R, `par()` itself is left unchanged.
    pub xaxp: Option<[f64; 3]>,
    pub yaxp: Option<[f64; 3]>,
}

crate::setters::setters!(PlotOptions => [PlotCall] {
    type_: value PlotType,
    pch: values i32,
    col: values ColorSpec,
    bg: values ColorSpec,
    lwd: values f64,
    lty: option LineType,
    cex: values f64,
    xlim: pair f64,
    ylim: pair f64,
    xlog: value bool,
    ylog: value bool,
    main: option Label,
    sub: option Label,
    xlab: option Label,
    ylab: option Label,
    axes: value bool,
    xaxt: option AxisType,
    yaxt: option AxisType,
    xaxp: option [f64; 3],
    yaxp: option [f64; 3],
});

impl Default for PlotOptions {
    fn default() -> Self {
        PlotOptions {
            type_: PlotType::Points,
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
            axes: true,
            xaxt: None,
            yaxt: None,
            xaxp: None,
            yaxp: None,
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
        let window = PlotWindowOptions {
            xlog: opts.xlog,
            ylog: opts.ylog,
            ..PlotWindowOptions::default()
        };
        self.plot_window_impl(xlim, ylim, &window);

        // `plot.xy(xy, type, ...)`.
        let xy = XyOptions {
            type_: Some(opts.type_),
            pch: opts.pch.clone(),
            col: opts.col.clone(),
            bg: opts.bg.clone(),
            cex: opts.cex.clone(),
            lwd: opts.lwd.clone(),
            lty: opts.lty,
            xpd: None,
        };
        self.points_impl(x, y, &xy);

        if opts.axes {
            // `localAxis()`: `axis(side, ...)` with the `par()` defaults.
            for (side, axt, axp) in [(1, opts.xaxt, opts.xaxp), (2, opts.yaxt, opts.yaxp)] {
                let o = AxisOptions {
                    axt,
                    axp,
                    ..AxisOptions::default()
                };
                self.axis_impl(side, &o);
            }
            self.box_impl(&BoxOptions::default());
        }

        if self.par.ann {
            self.title_impl(&TitleOptions {
                main: opts.main.clone(),
                sub: opts.sub.clone(),
                xlab: opts.xlab.clone(),
                ylab: opts.ylab.clone(),
            });
        }
    }
}
