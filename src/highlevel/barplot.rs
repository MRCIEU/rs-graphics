// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Bar charts: the options of
//! [`Graphics::barplot`](crate::graphics::Graphics::barplot), its heights
//! ([`BarHeights`]) and the order of stacked segments ([`OrderH`]).
//!
//! Supported: a plain vector (R's `vectorInput` case) and a matrix (see
//! [`BarHeights`]) drawn either stacked or `beside`, vertical or
//! `horiz`ontal, with `width`/`space`, a per-series colour vector (default
//! `gray.colors(nrow)`), `names.arg`, `legend.text` (and `args.legend`),
//! `xlim`/`ylim`, log axes (`xlog`/`ylog`), `offset`, `orderH`, `add`,
//! `axisnames`, `cex.names`, `axis.lty`, shading lines (`density`/
//! `angle`) and R's inline axis parameters (`las`, `cex.axis`, ...).
//!
//! Also `xpd` and `plot = FALSE`. Not supported: `panel.first`/
//! `panel.last`.
//!
//! This is R's `barplot(height, ...)`, ported from `barplot.default` in
//! `r-source/src/library/graphics/R/barplot.R`.
//!
//! **R bugs not copied:** with `orderH`, R reorders the colours of each
//! stack's segments but not `density`/`angle`, so the shading moves to
//! other rows; here it stays with its row, as the colour does. For the
//! legend, R reverses `density`/`angle` before recycling them over the
//! labels; here they are recycled first, as the colours are. And for
//! stacked bars R works out the default value axis limits with `offset`
//! recycled over every element of the matrix of stacked sums instead of
//! once per column (so `offset = c(0, 10, 0)` on a 3-column matrix gives
//! limits that cut off the bars it draws). We use
//! each column's own offset, as R does when drawing the bars.

use crate::backend::Backend;
use crate::color::{Color, ColorSpec, gray_colors};
use crate::device::{
    AxisOptions, Device, LegendOptions, LegendPos, PlotWindowOptions, RectOptions, TitleOptions,
};
use crate::error::{Error, from_r_string, from_str_via_parse};
use crate::font::FontFace;
use crate::lty::LineType;
use crate::numbers::{Num, Numbers};
use crate::par::{AxisStyle, Xpd};
use crate::plotmath::Label;
use crate::recycle::rep_len;

use super::InlineAxis;

crate::setters::options! {
    /// Options for [`Graphics::barplot`](crate::graphics::Graphics::barplot), covering the subset of
    /// `barplot.default`'s arguments this crate implements. Unset fields are
    /// R's defaults, given for each.
    #[derive(Clone, Debug)]
    pub struct BarplotOptions => [BarplotCall] {
        /// Labels under the bars, or beside them if `horiz`. For a `beside`
        /// matrix, either one name per bar or one per column (centred under
        /// each group). `None`: no names (R takes them from
        /// `names(height)`/`colnames(height)`, which Rust data doesn't have).
        /// R's `names.arg`.
        pub names: Option<Vec<Label>> => list Label,
        /// Fill colours, recycled over the bars of a vector or the rows of a
        /// matrix. Empty: `"grey"` for a vector, `gray.colors(nrow)` for a
        /// matrix.
        pub col: Vec<ColorSpec> => values ColorSpec,
        /// Colour of the bar borders. `None`: [`par(fg)`].
        ///
        /// [`par(fg)`]: crate::par::Par::fg
        pub border: Option<ColorSpec> => option ColorSpec,
        /// Shading lines per inch in the bars, recycled as `col` is; the
        /// lines take the bar's `col`. `None` or a negative value fills the
        /// bar instead, 0 leaves it empty (see [`RectOptions::density`]).
        /// Empty: no shading. The legend's boxes are shaded the same way.
        /// Not allowed with a log axis.
        pub density: Vec<Option<f64>> => na_values f64,
        /// Angle of the shading lines in degrees, recycled as `density` is.
        /// Empty: 45.
        pub angle: Vec<f64> => numbers f64,
        /// Bar widths, recycled. Empty: 1.
        pub width: Vec<f64> => numbers f64,
        /// Space before each bar, as a fraction of the mean bar width: one
        /// value, or `[within group, between groups]` for a `beside` matrix.
        /// `None` (or empty): `0.2`, or `[0, 1]` for a `beside` matrix.
        pub space: Option<Vec<f64>> => option_numbers f64,
        /// Matrix only: draws the bars side by side instead of stacked.
        /// Default `false`.
        pub beside: bool => value bool,
        /// Draws horizontal bars. Default `false`.
        pub horiz: bool => value bool,
        /// Legend labels, one per row of a matrix or per bar of a vector,
        /// filled with `col`. `None`: no legend. R's `legend.text`.
        pub legend_text: Option<Vec<Label>> => list Label,
        /// Position of the legend, e.g. `"topleft"` or `(x, y)`. `None`: 0.1
        /// inch in from the top-right corner of the plot region, as R. R's
        /// `args.legend = list(x = .., y = ..)`.
        pub legend_pos: Option<LegendPos> => option LegendPos,
        /// Legend options (e.g. `bty`, `inset`, `ncol`, `cex`, `title`,
        /// `fill`); every field set here overrides barplot's choice (`fill`
        /// from `col`, `xjust = yjust = 1`, otherwise `legend()`'s defaults).
        /// The labels always come from `legend_text`. Default: no overrides.
        /// The rest of R's `args.legend`.
        pub args_legend: LegendOptions => value LegendOptions,
        /// Limits of the x axis. `None`: from the bars.
        pub xlim: Option<(f64, f64)> => pair f64,
        /// Limits of the y axis. `None`: from the bars.
        pub ylim: Option<(f64, f64)> => pair f64,
        /// Makes the x axis logarithmic. Default `false`. On a log value
        /// axis the bars start at the first value of `ylim` (or `xlim` if
        /// `horiz`) when given, else at 0.9 times the smallest height, as
        /// in R. R's `log = "x"`.
        pub xlog: bool => value bool,
        /// Makes the y axis logarithmic. Default `false`. See `xlog`. R's
        /// `log = "y"`.
        pub ylog: bool => value bool,
        /// Shifts the bars along the value axis, recycled over the bars of a
        /// vector, the rows of a `beside` matrix, or the columns (stacks) of
        /// a stacked one. Empty: 0.
        pub offset: Vec<f64> => numbers f64,
        /// Stacked bars only: order of the segments within each stack; each
        /// segment keeps its row's colour. Default [`OrderH::None`] (row
        /// order). R's `orderH`.
        pub order_h: OrderH => value OrderH,
        /// Draws on the current plot instead of starting a new one: no new
        /// plot or panel, no new coordinates; the names, legend and axis are
        /// still drawn. Default `false`. R's `add = TRUE`.
        pub add: bool => value bool,
        /// Whether to draw the titles. `None`: [`par(ann)`], unless `add` (R's
        /// default `ann = !add && par("ann")`).
        ///
        /// [`par(ann)`]: crate::par::Par::ann
        pub ann: Option<bool> => option bool,
        /// Main title, text or a plotmath expression, as for `plot()`.
        /// `None`: none. Titles are drawn when `ann` is (by default
        /// [`par(ann)`], and not with `add`).
        ///
        /// [`par(ann)`]: crate::par::Par::ann
        pub main: Option<Label> => option Label,
        /// Subtitle. `None`: none.
        pub sub: Option<Label> => option Label,
        /// Label of the x axis. `None`: none.
        pub xlab: Option<Label> => option Label,
        /// Label of the y axis. `None`: none.
        pub ylab: Option<Label> => option Label,
        /// Size of the main title, relative to [`par(cex)`]. `None`:
        /// [`par(cex.main)`]. R's `cex.main`.
        ///
        /// [`par(cex)`]: crate::par::Par::cex
        /// [`par(cex.main)`]: crate::par::Par::cex_main
        pub cex_main: Option<f64> => number f64,
        /// Colour of the main title. `None`: [`par(col.main)`]. R's `col.main`.
        ///
        /// [`par(col.main)`]: crate::par::Par::col_main
        pub col_main: Option<ColorSpec> => option ColorSpec,
        /// Font of the main title. `None`: [`par(font.main)`]. R's `font.main`.
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
        /// Font of the subtitle. `None`: [`par(font.sub)`]. R's `font.sub`.
        ///
        /// [`par(font.sub)`]: crate::par::Par::font_sub
        pub font_sub: Option<FontFace> => option FontFace,
        /// Size of the axis labels `xlab` and `ylab`, relative to
        /// [`par(cex)`]. `None`: [`par(cex.lab)`]. R's `cex.lab`.
        ///
        /// [`par(cex)`]: crate::par::Par::cex
        /// [`par(cex.lab)`]: crate::par::Par::cex_lab
        pub cex_lab: Option<f64> => number f64,
        /// Colour of `xlab` and `ylab`. `None`: [`par(col.lab)`]. R's
        /// `col.lab`.
        ///
        /// [`par(col.lab)`]: crate::par::Par::col_lab
        pub col_lab: Option<ColorSpec> => option ColorSpec,
        /// Font of `xlab` and `ylab`. `None`: [`par(font.lab)`]. R's
        /// `font.lab`.
        ///
        /// [`par(font.lab)`]: crate::par::Par::font_lab
        pub font_lab: Option<FontFace> => option FontFace,
        /// Draws the value axis. Default `true`.
        pub axes: bool => value bool,
        /// Draws the names. Default `true`.
        pub axisnames: bool => value bool,
        /// Size of the names. `None`: [`par(cex.axis)`]. R's `cex.names`.
        ///
        /// [`par(cex.axis)`]: crate::par::Par::cex_axis
        pub cex_names: Option<f64> => number f64,
        /// Line type of the names axis's line and ticks. `None`: none drawn
        /// (R's 0). R's `axis.lty`.
        pub axis_lty: Option<LineType> => option LineType,
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
        /// Margin lines of the axis titles, labels and line, for the axes
        /// and titles. `None`: [`par(mgp)`].
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
        /// Minimum gap between axis labels. `None`: R's default. R's
        /// `gap.axis`.
        pub gap_axis: Option<f64> => number f64,
        /// Width of the axes' line and ticks (not the bars' borders, as in
        /// R). `None`: 1, `axis()`'s default.
        pub lwd: Option<f64> => number f64,
        /// Clipping of the bars, names and legend; the setter also takes
        /// `false`/`true`. `None`: [`Xpd::Figure`] (R's `TRUE`).
        pub xpd: Option<Xpd> => option Xpd,
        /// `false` works out the bars' centres and returns them without
        /// drawing. `None`: `true`.
        pub plot: Option<bool> => option bool,
    }
}

impl Default for BarplotOptions {
    fn default() -> Self {
        BarplotOptions {
            deferred_error: None,
            names: None,
            col: Vec::new(),
            border: None,
            density: Vec::new(),
            angle: Vec::new(),
            width: Vec::new(),
            space: None,
            beside: false,
            horiz: false,
            legend_text: None,
            legend_pos: None,
            args_legend: LegendOptions::default(),
            xlim: None,
            ylim: None,
            xlog: false,
            ylog: false,
            offset: Vec::new(),
            order_h: OrderH::None,
            add: false,
            ann: None,
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
            axisnames: true,
            cex_names: None,
            axis_lty: None,
            las: None,
            cex_axis: None,
            col_axis: None,
            font_axis: None,
            mgp: None,
            tck: None,
            tcl: None,
            gap_axis: None,
            lwd: None,
            xpd: None,
            plot: None,
        }
    }
}

/// The order of the segments within each stack of a stacked bar chart:
/// row order, or sorted by size.
///
/// Each segment keeps its row's colour. Setters also take R's names or a
/// unique prefix (`.order_h("decr")`); any other string is an error of the
/// call.
///
/// This is R's `barplot(orderH = )`.
///
/// # Examples
///
/// ```
/// use rs_graphics::prelude::*;
///
/// # let mut dev = Device::null().open().unwrap();
/// let columns = vec![vec![1, 4, 2], vec![3, 1, 5]];
/// dev.barplot().height(&columns).order_h(OrderH::Decr).draw();
/// dev.barplot().height(&columns).order_h("incr").draw();
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum OrderH {
    /// `"none"`: in row order (the default).
    #[default]
    None,
    /// `"incr"`: smallest segment at the base.
    Incr,
    /// `"decr"`: largest segment at the base.
    Decr,
}

impl OrderH {
    /// Reads an order from its name: `"none"`, `"incr"` or `"decr"`, or a
    /// unique prefix of one; `None` for anything else.
    ///
    /// This matches names as R's `match.arg()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use rs_graphics::prelude::*;
    ///
    /// assert_eq!(OrderH::parse("d"), Some(OrderH::Decr));
    /// assert_eq!(OrderH::parse("up"), None);
    /// ```
    pub fn parse(s: &str) -> Option<OrderH> {
        let all = [
            ("none", OrderH::None),
            ("incr", OrderH::Incr),
            ("decr", OrderH::Decr),
        ];
        let mut hits = all
            .iter()
            .filter(|(name, _)| !s.is_empty() && name.starts_with(s));
        match (hits.next(), hits.next()) {
            (Some(&(_, o)), None) => Some(o),
            _ => None,
        }
    }
}

from_str_via_parse!(OrderH, "orderH");
from_r_string!(OrderH, "orderH", "\"none\", \"incr\" or \"decr\"");

/// The heights of a bar chart: a vector (one bar per value) or a matrix
/// given as its columns.
///
/// The `height` argument of
/// [`Graphics::barplot`](crate::graphics::Graphics::barplot). Built with
/// `.into()` (which the `height` setter does for you) from a vector of
/// numbers of any type (`f64` or integers; a slice, array or `Vec`, by
/// reference or by value, or a range, see [`Numbers`]) or from a matrix
/// given as its columns (`Vec<Vec<T>>`, a slice or array of `Vec<T>`).
///
/// This is R's vector or matrix `height`.
///
/// # Examples
///
/// ```
/// use rs_graphics::prelude::*;
///
/// assert_eq!(BarHeights::from([3, 5, 2]), BarHeights::Vector(vec![3.0, 5.0, 2.0]));
/// // Two columns (stacks) of three rows (series) each.
/// let columns = vec![vec![1.0, 4.0, 2.0], vec![3.0, 1.0, 5.0]];
/// assert_eq!(BarHeights::from(&columns), BarHeights::Matrix(columns.clone()));
/// # let mut dev = Device::null().open().unwrap();
/// let mids = dev.barplot().height(&columns).beside(true).draw();
/// assert_eq!(mids.len(), 6);
/// ```
#[derive(Clone, Debug, PartialEq)]
pub enum BarHeights {
    /// One bar per value (R's `vectorInput` case).
    Vector(Vec<f64>),
    /// A matrix as its *columns*: each inner `Vec` is one column, i.e. one
    /// group or stack, and its entries are the rows, i.e. the series. All
    /// columns must have the same length.
    Matrix(Vec<Vec<f64>>),
}

impl<'a, N: Numbers<'a>> From<N> for BarHeights {
    /// A vector of any numbers, by reference or by value, or a range (see
    /// [`Numbers`]).
    fn from(v: N) -> Self {
        BarHeights::Vector(v.into_numbers().into_owned())
    }
}

fn columns<T: Num>(m: &[Vec<T>]) -> Vec<Vec<f64>> {
    m.iter()
        .map(|c| c.iter().map(|v| v.to_f64()).collect())
        .collect()
}

impl<T: Num> From<Vec<Vec<T>>> for BarHeights {
    /// A matrix given as its columns.
    fn from(m: Vec<Vec<T>>) -> Self {
        BarHeights::Matrix(columns(&m))
    }
}

impl<T: Num> From<&Vec<Vec<T>>> for BarHeights {
    /// A matrix given as its columns.
    fn from(m: &Vec<Vec<T>>) -> Self {
        BarHeights::Matrix(columns(m))
    }
}

impl<T: Num> From<&[Vec<T>]> for BarHeights {
    /// A matrix given as its columns.
    fn from(m: &[Vec<T>]) -> Self {
        BarHeights::Matrix(columns(m))
    }
}

impl<T: Num, const N: usize> From<[Vec<T>; N]> for BarHeights {
    /// A matrix given as its columns.
    fn from(m: [Vec<T>; N]) -> Self {
        BarHeights::Matrix(columns(&m))
    }
}

impl<T: Num, const N: usize> From<&[Vec<T>; N]> for BarHeights {
    /// A matrix given as its columns.
    fn from(m: &[Vec<T>; N]) -> Self {
        BarHeights::Matrix(columns(m))
    }
}

/// Everything `barplot()` works out before drawing: bar positions, the
/// bars themselves, colours, axis limits and where the names go. Needs no
/// device, so a recorded figure can return the same bar centres as
/// [`Graphics::barplot`](crate::graphics::Graphics::barplot) (and fails on the same invalid input).
pub(crate) struct BarLayout {
    beside: bool,
    w_l: Vec<f64>,
    w_r: Vec<f64>,
    /// Bar centres (R's `w.m`): what `barplot()` returns.
    pub(crate) w_m: Vec<f64>,
    /// (position index, base, top, colour index) per bar.
    bars: Vec<(usize, f64, f64, usize)>,
    colors: Vec<ColorSpec>,
    xlim: (f64, f64),
    ylim: (f64, f64),
    /// Where `names` go along the category axis, if given.
    names_at: Option<Vec<f64>>,
}

/// See [`BarLayout`].
pub(crate) fn bar_layout(heights: &BarHeights, opts: &BarplotOptions) -> Result<BarLayout, Error> {
    let (columns, vector_input): (&[Vec<f64>], bool) = match heights {
        BarHeights::Vector(v) => {
            if v.is_empty() {
                return Err(Error::invalid("'height' must be non-empty"));
            }
            (std::slice::from_ref(v), true)
        }
        BarHeights::Matrix(columns) => {
            if columns.is_empty() {
                return Err(Error::invalid("'height' must be non-empty"));
            }
            let nr = columns[0].len();
            if nr == 0 || columns.iter().any(|c| c.len() != nr) {
                return Err(Error::invalid(
                    "matrix columns must be non-empty and of equal length",
                ));
            }
            (columns, false)
        }
    };
    let nc = columns.len();
    let nr = columns[0].len();
    let beside = vector_input || opts.beside;
    let horiz = opts.horiz;
    let width: &[f64] = if opts.width.is_empty() {
        &[1.0]
    } else {
        &opts.width
    };

    // --- geometry along the category axis (w.l / w.m / w.r) ---
    let mean_width = width.iter().sum::<f64>() / width.len() as f64;
    let space_in = opts
        .space
        .clone()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            if !vector_input && beside {
                vec![0.0, 1.0]
            } else {
                vec![0.2]
            }
        });
    let space: Vec<f64> = space_in.iter().map(|s| s * mean_width).collect();

    let nbars = if beside { nr * nc } else { nc };
    let space_v: Vec<f64> = if beside && space.len() == 2 && !vector_input {
        (0..nc)
            .flat_map(|_| std::iter::once(space[1]).chain(std::iter::repeat_n(space[0], nr - 1)))
            .collect()
    } else {
        rep_len(&space, nbars)
    };
    let base_n = if beside { nr } else { nc };
    let width_v: Vec<f64> = (0..nbars)
        .map(|k| width[(k % base_n) % width.len()])
        .collect();

    let mut w_l = vec![0.0; nbars];
    let mut w_r = vec![0.0; nbars];
    let mut w_m = vec![0.0; nbars];
    let mut cum = 0.0;
    for k in 0..nbars {
        cum += space_v[k] + width_v[k];
        w_r[k] = cum;
        w_m[k] = cum - width_v[k] / 2.0;
        w_l[k] = w_m[k] - width_v[k] / 2.0;
    }

    // `offset`, recycled to the length of `width`: one per row (beside)
    // or per column (stacked). `offset_of(i, j)` is that of row i, column j.
    let offset = if opts.offset.is_empty() {
        vec![0.0]
    } else {
        rep_len(&opts.offset, base_n)
    };
    let offset_of = |i: usize, j: usize| {
        let k = if beside { i } else { j };
        offset[k % offset.len()]
    };

    if (opts.xlog || opts.ylog) && !opts.density.is_empty() {
        return Err(Error::invalid(
            "Cannot use shading lines in bars when log scale is used",
        ));
    }

    // --- log scale in the data direction: where bars start ---
    let log_dat = (opts.xlog && horiz) || (opts.ylog && !horiz);
    let rectbase = if log_dat {
        let min_h = columns
            .iter()
            .flatten()
            .copied()
            .filter(|h| !h.is_nan())
            .fold(f64::INFINITY, f64::min);
        let min_h_off = columns
            .iter()
            .enumerate()
            .flat_map(|(j, c)| c.iter().enumerate().map(move |(i, &h)| (i, j, h)))
            .map(|(i, j, h)| h + offset_of(i, j))
            .filter(|h| !h.is_nan())
            .fold(f64::INFINITY, f64::min);
        if min_h_off <= 0.0 {
            return Err(Error::invalid(
                "log scale error: at least one 'height + offset' value <= 0",
            ));
        }
        for (log, lim, name) in [
            (opts.xlog, opts.xlim, "xlim"),
            (opts.ylog, opts.ylim, "ylim"),
        ] {
            if let (true, Some(l)) = (log, lim) {
                if l.0.min(l.1) <= 0.0 {
                    return Err(Error::invalid(format!("log scale error: '{name}' <= 0")));
                }
            }
        }
        match (horiz, opts.ylim, opts.xlim) {
            (false, Some(l), _) if opts.ylog => l.0,
            (true, _, Some(l)) if opts.xlog => l.0,
            _ => 0.9 * min_h,
        }
    } else {
        0.0
    };

    // --- bars: (position index, base, top, colour index) ---
    let mut bars: Vec<(usize, f64, f64, usize)> = Vec::new();
    let mut extent_lo = f64::INFINITY;
    let mut extent_hi = f64::NEG_INFINITY;
    let mut note = |h: f64, off: f64| {
        // `range(rAdj, height + offset)` with `rAdj = offset + 0.9 * height`
        // on a log value axis, else `offset - 0.01 * height`.
        let adj = off + if log_dat { 0.9 * h } else { -0.01 * h };
        extent_lo = extent_lo.min(adj).min(h + off);
        extent_hi = extent_hi.max(adj).max(h + off);
    };
    if beside {
        for (j, col) in columns.iter().enumerate() {
            for (i, &h) in col.iter().enumerate() {
                let k = j * nr + i;
                let off = offset_of(i, j);
                bars.push((k, rectbase + off, h + off, k));
                note(h, off);
            }
        }
    } else {
        // `rbind(rectbase, apply(height, 2, cumsum))`: the first segment
        // starts at `rectbase`, the sums themselves start from 0. With
        // `orderH`, each column's segments are summed in sorted order
        // (`order()`: stable, NaN last) and keep their row's colour.
        for (j, col) in columns.iter().enumerate() {
            let off = offset_of(0, j);
            note(rectbase, off);
            let mut rows: Vec<usize> = (0..nr).collect();
            if opts.order_h != OrderH::None {
                let decr = opts.order_h == OrderH::Decr;
                rows.sort_by(|&a, &b| {
                    let (x, y) = (col[a], col[b]);
                    match (x.is_nan(), y.is_nan()) {
                        (true, true) => std::cmp::Ordering::Equal,
                        (true, false) => std::cmp::Ordering::Greater,
                        (false, true) => std::cmp::Ordering::Less,
                        _ if decr => y.partial_cmp(&x).unwrap(),
                        _ => x.partial_cmp(&y).unwrap(),
                    }
                });
            }
            let mut acc = 0.0;
            for (n, &i) in rows.iter().enumerate() {
                let base = if n == 0 { rectbase } else { acc };
                acc += col[i];
                bars.push((j, base + off, acc + off, i));
                note(acc, off);
            }
        }
    }

    let colors: Vec<ColorSpec> = if !opts.col.is_empty() {
        opts.col.clone()
    } else if vector_input {
        vec![ColorSpec::Color(Color::parse("grey").unwrap())]
    } else {
        gray_colors(nr)
    };

    let cat_lim = (
        w_l.iter().cloned().fold(f64::INFINITY, f64::min),
        w_r.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
    );
    let val_lim = (extent_lo, extent_hi);
    let (xlim, ylim) = if horiz {
        (opts.xlim.unwrap_or(val_lim), opts.ylim.unwrap_or(cat_lim))
    } else {
        (opts.xlim.unwrap_or(cat_lim), opts.ylim.unwrap_or(val_lim))
    };

    let names_at = opts
        .names
        .as_ref()
        .map(|names| {
            if names.len() == w_m.len() {
                Ok(w_m.clone())
            } else if beside && names.len() == nc {
                // colMeans(w.m): the centre of each group of bars.
                Ok((0..nc)
                    .map(|j| w_m[j * nr..(j + 1) * nr].iter().sum::<f64>() / nr as f64)
                    .collect())
            } else {
                Err(Error::invalid("incorrect number of names"))
            }
        })
        .transpose()?;

    Ok(BarLayout {
        beside,
        w_l,
        w_r,
        w_m,
        bars,
        colors,
        xlim,
        ylim,
        names_at,
    })
}

impl<B: Backend> Device<B> {
    /// Draws R's `barplot()`; the public entry is
    /// [`Graphics::barplot`](crate::graphics::Graphics::barplot), whose docs
    /// describe it.
    pub(crate) fn barplot_impl(
        &mut self,
        heights: impl Into<BarHeights>,
        opts: &BarplotOptions,
    ) -> Result<Vec<f64>, Error> {
        let layout = bar_layout(&heights.into(), opts)?;
        if !opts.plot.unwrap_or(true) {
            return Ok(layout.w_m);
        }
        // R's `opar <- par(yaxs = "i")` (`xaxs` when `horiz`), then
        // `par(xpd = TRUE)`, restored `on.exit`.
        let mut dev = self.par_scope(|now, saved| {
            now.xaxs = saved.xaxs;
            now.yaxs = saved.yaxs;
            now.xpd = saved.xpd;
        });
        dev.draw_barplot(layout, opts)
    }

    /// Draws `layout`; returns its bar centres.
    fn draw_barplot(
        &mut self,
        layout: BarLayout,
        opts: &BarplotOptions,
    ) -> Result<Vec<f64>, Error> {
        let BarLayout {
            beside,
            w_l,
            w_r,
            w_m,
            bars,
            colors,
            xlim,
            ylim,
            names_at,
        } = layout;
        let horiz = opts.horiz;

        // barplot() sets the *value* axis to `"i"` (exact fit, no 4%
        // padding), leaving the categorical axis at its regular setting;
        // `barplot_impl` restores it.
        if horiz {
            self.par.xaxs = AxisStyle::Exact;
        } else {
            self.par.yaxs = AxisStyle::Exact;
        }
        if !opts.add {
            self.plot_new_impl()?;
            let window = PlotWindowOptions {
                xlog: opts.xlog,
                ylog: opts.ylog,
                ..PlotWindowOptions::default()
            };
            self.plot_window_impl(xlim, ylim, &window);
        }

        // R draws everything with `par(xpd = xpd)`, `TRUE` by default
        // (restored by `barplot_impl`).
        self.par.xpd = opts.xpd.unwrap_or(Xpd::Figure);

        // One `rect()` call for all bars beside each other, one per stack
        // otherwise, as R (which matters for the order shading lines and
        // borders are drawn in); `col`, `density` and `angle` per bar.
        let border = vec![opts.border.unwrap_or(ColorSpec::Color(self.par.fg))];
        let mut start = 0;
        while start < bars.len() {
            let end = if beside {
                bars.len()
            } else {
                start
                    + bars[start..]
                        .iter()
                        .take_while(|b| b.0 == bars[start].0)
                        .count()
            };
            let group = &bars[start..end];
            let (mut x0, mut y0, mut x1, mut y1) = (vec![], vec![], vec![], vec![]);
            for &(k, base, top, _) in group {
                let (lo, hi) = (w_l[k], w_r[k]);
                if horiz {
                    x0.push(base);
                    y0.push(lo);
                    x1.push(top);
                    y1.push(hi);
                } else {
                    x0.push(lo);
                    y0.push(base);
                    x1.push(hi);
                    y1.push(top);
                }
            }
            let style = RectOptions {
                col: group.iter().map(|b| colors[b.3 % colors.len()]).collect(),
                border: border.clone(),
                density: per_bar(&opts.density, group),
                angle: per_bar(&opts.angle, group),
                ..RectOptions::default()
            };
            self.rect_impl(&x0, &y0, &x1, &y1, &style)?;
            start = end;
        }

        // R passes `...` to both axes; the value axis gets `cex.axis`, the
        // names axis `cex.names` instead.
        let inline = InlineAxis {
            las: opts.las,
            cex_axis: None,
            col_axis: opts.col_axis,
            font_axis: opts.font_axis,
            mgp: opts.mgp,
            tck: opts.tck,
            tcl: opts.tcl,
            gap_axis: opts.gap_axis,
            lwd: opts.lwd,
            ..InlineAxis::default()
        };
        if let (true, Some(names), Some(at)) = (opts.axisnames, &opts.names, names_at) {
            // `axis(side, at = at, labels = names.arg, lty = axis.lty,
            // cex.axis = cex.names, ...)`: with R's default `axis.lty = 0`,
            // the names only.
            let lty = opts.axis_lty.filter(|&l| l != LineType::BLANK);
            let names = AxisOptions {
                at: Some(at),
                labels: Some(names.iter().cloned().map(Some).collect()),
                tick: lty.is_some(),
                lty,
                cex_axis: opts.cex_names,
                ..AxisOptions::default()
            };
            self.axis_impl(if horiz { 2 } else { 1 }, &inline.apply(names))?;
        }

        if let Some(labels) = &opts.legend_text {
            let mut labels = labels.clone();
            let mut fill: Vec<ColorSpec> = (0..labels.len())
                .map(|i| colors[i % colors.len()])
                .collect();
            let mut density = rep_len(&opts.density, labels.len());
            let mut angle = rep_len(&opts.angle, labels.len());
            // (R reverses `density` and `angle` before `legend()` recycles
            // them, so with fewer values than labels they don't follow
            // the reversed labels; we recycle first, as for `fill`.)
            if (horiz && beside) || (!horiz && !beside) {
                labels.reverse();
                fill.reverse();
                density.reverse();
                angle.reverse();
            }
            // By default 0.1 inch in from the top-right corner of the plot
            // region. (On a log axis R passes the log10 `par("usr")` values
            // to `legend()` as data, which puts the legend near the bottom;
            // we don't copy that.)
            let pos = opts.legend_pos.unwrap_or_else(|| {
                let (ux0, ux1, uy0, uy1) = self.usr();
                let plt = self.plot_region();
                let data = |v: f64, log: bool| if log { 10f64.powf(v) } else { v };
                let x = data(ux1 - 0.1 * (ux1 - ux0) / plt.width(), self.xlog());
                let y = data(uy1 - 0.1 * (uy1 - uy0) / plt.height(), self.ylog());
                LegendPos::Xy(x, y)
            });
            // `args.legend1[names(args.legend)] <- args.legend`.
            let mut lopts = opts.args_legend.clone();
            lopts.xjust.get_or_insert(1.0);
            lopts.yjust.get_or_insert(1.0);
            if lopts.fill.is_empty() {
                lopts.fill = fill;
            }
            if lopts.density.is_empty() {
                lopts.density = density;
            }
            if lopts.angle.is_empty() {
                lopts.angle = angle;
            }
            let labels: Vec<Option<Label>> = labels.into_iter().map(Some).collect();
            self.legend_impl(pos, &labels, &lopts)?;
        }

        if opts.ann.unwrap_or(!opts.add && self.par.ann) {
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
                ..TitleOptions::default()
            });
        }

        if opts.axes {
            let value_axis = InlineAxis {
                cex_axis: opts.cex_axis,
                ..inline
            };
            self.axis_impl(
                if horiz { 1 } else { 2 },
                &value_axis.apply(AxisOptions::default()),
            )?;
        }

        Ok(w_m)
    }
}

/// `v` per bar of `group`, recycled as `col` is (by each bar's colour
/// index); empty stays empty.
fn per_bar<T: Copy>(v: &[T], group: &[(usize, f64, f64, usize)]) -> Vec<T> {
    if v.is_empty() {
        Vec::new()
    } else {
        group.iter().map(|b| v[b.3 % v.len()]).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_h_parses_like_match_arg() {
        assert_eq!(OrderH::parse("none"), Some(OrderH::None));
        assert_eq!(OrderH::parse("incr"), Some(OrderH::Incr));
        assert_eq!(OrderH::parse("d"), Some(OrderH::Decr));
        assert_eq!(OrderH::parse("in"), Some(OrderH::Incr));
        assert_eq!(OrderH::parse(""), None);
        assert_eq!(OrderH::parse("increasing"), None);
        assert_eq!("decr".parse::<OrderH>(), Ok(OrderH::Decr));
        assert!("up".parse::<OrderH>().is_err());
    }
}
