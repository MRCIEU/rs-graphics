// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Box plots: the options of
//! [`Graphics::boxplot`](crate::graphics::Graphics::boxplot) and the
//! statistics of each box ([`BoxStats`], [`boxplot_stats`]).
//!
//! For a list of numeric groups, with positions (`at`), widths, `notch`,
//! `varwidth`, `horizontal`, `add`, `outline`, log axes and the drawing
//! parameters of R's `bxp()`.
//!
//! This is R's `boxplot(x, ...)`, ported from
//! `boxplot.default`/`bxp`/`boxplot.stats` in
//! `r-source/src/library/graphics/R/boxplot.R`.

use crate::backend::{Backend, LineEnd};
use crate::color::{Color, ColorSpec};
use crate::device::{
    AxisOptions, BoxOptions, Device, PlotWindowOptions, PolygonOptions, SegmentsOptions,
    TitleOptions, XyOptions,
};
use crate::error::Error;
use crate::lty::LineType;
use crate::numbers::{Num, Numbers};
use crate::par::AxisType;
use crate::plotmath::Label;
use crate::recycle::nth;

use super::InlineAxis;

/// Tukey's five-number summary (`fivenum()` in R's `stats` package):
/// `[min, lower hinge, median, upper hinge, max]`. This is *not* the same
/// as `quantile(type = 7)` - R's `boxplot.stats()` specifically uses
/// `fivenum()`.
fn fivenum(sorted: &[f64]) -> [f64; 5] {
    let n = sorted.len() as f64;
    let n4 = ((n + 3.0) / 2.0).floor() / 2.0;
    let d = [1.0, n4, (n + 1.0) / 2.0, n + 1.0 - n4, n];
    let get = |idx: f64| -> f64 {
        let lo = idx.floor() as usize;
        let hi = idx.ceil() as usize;
        0.5 * (sorted[lo - 1] + sorted[hi - 1])
    };
    [get(d[0]), get(d[1]), get(d[2]), get(d[3]), get(d[4])]
}

/// The statistics of one box: whisker ends, hinges, median, outliers,
/// count and notch.
///
/// Computed by [`boxplot_stats`], and returned for each group by
/// [`Graphics::boxplot`](crate::graphics::Graphics::boxplot)'s `.draw()`.
/// For a group with no finite values the five statistics and `conf` are
/// `NaN` and there are no outliers.
///
/// This is R's `boxplot.stats()` result: `$stats`, `$out`, `$n` and
/// `$conf`.
///
/// # Examples
///
/// ```
/// use rs_graphics::prelude::*;
///
/// # let mut dev = Device::null().open().unwrap();
/// let stats = dev.boxplot().x(&[1, 2, 3, 4, 5, 100]).draw();
/// assert_eq!(stats[0].median, 3.5);
/// assert_eq!(stats[0].outliers, [100.0]);
/// ```
#[derive(Clone, Debug)]
pub struct BoxStats {
    /// The lower whisker's end: the smallest value that isn't an outlier.
    pub lower_whisker: f64,
    /// The lower hinge (Tukey's, close to the first quartile).
    pub q1: f64,
    /// The median.
    pub median: f64,
    /// The upper hinge (Tukey's, close to the third quartile).
    pub q3: f64,
    /// The upper whisker's end: the largest value that isn't an outlier.
    pub upper_whisker: f64,
    /// The values beyond the whiskers, in increasing order.
    pub outliers: Vec<f64>,
    /// The number of non-`NaN` values (`boxplot.stats()$n`).
    pub n: usize,
    /// The notch, `[low, high]`: `median -/+ 1.58 * IQR / sqrt(n)`
    /// (`boxplot.stats()$conf`).
    pub conf: [f64; 2],
}

/// Computes the statistics of one box from the values `x`: hinges, median,
/// whiskers and outliers.
///
/// The hinges and median are Tukey's five-number summary of the finite
/// values (R's `fivenum()`, not `quantile()`). Values more than `coef`
/// times the inter-hinge range beyond a hinge are outliers, and the
/// whiskers extend to the most extreme values that aren't. With `coef <=
/// 0` (or an infinite range) there are no outliers and the whiskers reach
/// the extremes. `n` counts the non-`NaN` values (infinities included),
/// and the notch is `median -/+ 1.58 * IQR / sqrt(n)`. With no finite
/// values, the statistics are `NaN` (see [`BoxStats`]).
///
/// This is R's `boxplot.stats(x, coef)`.
///
/// # Examples
///
/// ```
/// use rs_graphics::prelude::*;
///
/// let s = boxplot_stats(&[1, 2, 3, 4, 5, 100], 1.5);
/// assert_eq!((s.q1, s.median, s.q3), (2.0, 3.5, 5.0));
/// assert_eq!((s.lower_whisker, s.upper_whisker), (1.0, 5.0));
/// assert_eq!(s.outliers, [100.0]);
/// assert_eq!(s.n, 6);
/// ```
pub fn boxplot_stats<'a>(x: impl Numbers<'a>, coef: impl Num) -> BoxStats {
    let (x, coef) = (x.into_numbers(), coef.to_f64());
    let x: &[f64] = &x;
    let n = x.iter().filter(|v| !v.is_nan()).count();
    let mut sorted: Vec<f64> = x.iter().copied().filter(|v| v.is_finite()).collect();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    if sorted.is_empty() {
        return BoxStats {
            lower_whisker: f64::NAN,
            q1: f64::NAN,
            median: f64::NAN,
            q3: f64::NAN,
            upper_whisker: f64::NAN,
            outliers: Vec::new(),
            n,
            conf: [f64::NAN, f64::NAN],
        };
    }
    let five = fivenum(&sorted);
    let iqr = five[3] - five[1];
    let mut lo = five[0];
    let mut hi = five[4];
    let mut outliers = Vec::new();
    if coef > 0.0 && iqr.is_finite() {
        let lower_fence = five[1] - coef * iqr;
        let upper_fence = five[3] + coef * iqr;
        let non_out: Vec<f64> = sorted
            .iter()
            .copied()
            .filter(|&v| v >= lower_fence && v <= upper_fence)
            .collect();
        outliers = sorted
            .iter()
            .copied()
            .filter(|&v| v < lower_fence || v > upper_fence)
            .collect();
        if !outliers.is_empty() && !non_out.is_empty() {
            lo = non_out.iter().cloned().fold(f64::INFINITY, f64::min);
            hi = non_out.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        }
    }
    let half_notch = 1.58 * iqr / (n as f64).sqrt();
    BoxStats {
        lower_whisker: lo,
        q1: five[1],
        median: five[2],
        q3: five[3],
        upper_whisker: hi,
        outliers,
        n,
        conf: [five[2] - half_notch, five[2] + half_notch],
    }
}

crate::setters::options! {
    /// Options for [`Graphics::boxplot`](crate::graphics::Graphics::boxplot):
    /// the arguments of R's `boxplot.default` and `bxp()`, including the
    /// drawing parameters R takes in `pars` or directly (`boxplot(x, medlwd =
    /// 2)`). Settings left unset are R's defaults, given on each setter.
    ///
    /// Like R's, the per-box values (`col`, `border`, `boxwex` and every
    /// `box*`/`med*`/`whisk*`/`staple*`/`out*` style) are recycled over the
    /// boxes. The style of each part falls back to the general `lty`, `lwd`,
    /// `pch`, `cex` and `bg` here, then to `par()`, as in `bxp()`. Of the
    /// title parameters, only the `cex.*` and `col.*` ones are passed to
    /// `title()`, as `bxp()` does (`font.main` and co. come from `par()`), and
    /// the frame follows [`par(bty)`] (`bxp()`'s `box()` takes no `bty`).
    ///
    /// [`par(bty)`]: crate::par::Par::bty
    #[derive(Clone, Debug)]
    pub struct BoxplotOptions => [BoxplotCall] {
        /// Group labels on the name axis (side 1, or 2 if `horizontal`).
        /// Unset: `1, 2, ...`; a `None` element (R's `NA`) leaves that label
        /// out. Drawn only when there's more than one box, unless
        /// `show_names` says otherwise.
        pub names: Option<Vec<Option<Label>>> => labels Label,
        /// Box fill colours, recycled over the boxes. Empty: `"lightgray"`.
        /// Passed to R's `bxp()` as `boxfill`.
        pub col: Vec<ColorSpec> => values ColorSpec,
        /// Colours of the box outlines, median, whiskers, staples and
        /// outliers, recycled over the boxes. Empty: [`par(fg)`].
        ///
        /// [`par(fg)`]: crate::par::Par::fg
        pub border: Vec<ColorSpec> => values ColorSpec,
        /// How far the whiskers reach: to the most extreme point within
        /// `range` times the box length (IQR); points beyond are outliers.
        /// `None`: 1.5.
        pub range: Option<f64> => number f64,
        /// Box positions on the name axis, one per group. `None`: `1, 2,
        /// ..., n`.
        pub at: Option<Vec<f64>> => option_numbers f64,
        /// Relative box widths, one per group, scaled so the widest is
        /// `boxwex`. `None`: equal widths, or by group size with `varwidth`;
        /// a single box is half of `boxwex` wide (as in R).
        pub width: Option<Vec<f64>> => option_numbers f64,
        /// Draws on the current plot instead of starting a new one. Default
        /// `false`. R's `add = TRUE`.
        pub add: bool => value bool,
        /// Draws the outliers and includes them in the default value range.
        /// Default `true`.
        pub outline: bool => value bool,
        /// Makes the x axis logarithmic: the position axis, or the value axis
        /// when `horizontal`. Default `false`. R's `log = "x"`.
        pub xlog: bool => value bool,
        /// Makes the y axis logarithmic: the value axis, or the position axis
        /// when `horizontal`. Default `false`. R's `log = "y"`.
        pub ylog: bool => value bool,
        /// Limits of the position axis. `None`: `range(at) + c(-0.5, 0.5)`.
        pub xlim: Option<(f64, f64)> => pair f64,
        /// Limits of the value axis, also when `horizontal` (as in R).
        /// `None`: the range of the statistics, plus the outliers (unless
        /// `outline` is off) and the notches (with `notch`).
        pub ylim: Option<(f64, f64)> => pair f64,
        /// Main title, text or a plotmath expression, as for `plot()`.
        /// `None`: none.
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
        pub col_main: Option<ColorSpec> => option_value ColorSpec,
        /// Size of the subtitle, relative to [`par(cex)`]. `None`:
        /// [`par(cex.sub)`]. R's `cex.sub`.
        ///
        /// [`par(cex)`]: crate::par::Par::cex
        /// [`par(cex.sub)`]: crate::par::Par::cex_sub
        pub cex_sub: Option<f64> => number f64,
        /// Colour of the subtitle. `None`: [`par(col.sub)`]. R's `col.sub`.
        ///
        /// [`par(col.sub)`]: crate::par::Par::col_sub
        pub col_sub: Option<ColorSpec> => option_value ColorSpec,
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
        pub col_lab: Option<ColorSpec> => option_value ColorSpec,
        /// Whether to draw the titles. `None`: unless `add` (R's default
        /// `ann = !add`, not [`par(ann)`]).
        ///
        /// [`par(ann)`]: crate::par::Par::ann
        pub ann: Option<bool> => option bool,
        /// Draws the axes and the box around the plot, also with `add` (as
        /// in R). Default `true`.
        pub axes: bool => value bool,
        /// Draws notched boxes: each box narrows to half its width at the
        /// median, between [`BoxStats::conf`]'s limits (R's `notch.frac =
        /// 0.5`). Default `false`.
        pub notch: bool => value bool,
        /// Makes box widths proportional to the square root of the group
        /// size. Default `false`.
        pub varwidth: bool => value bool,
        /// Draws horizontal boxes: group names on side 2, values on side 1.
        /// Default `false`.
        pub horizontal: bool => value bool,

        /// Box widths, in units of the position axis (of its natural log on a
        /// log axis). Empty: 0.8 (R's `boxplot()` default, whatever `at` is).
        pub boxwex: Vec<f64> => numbers f64,
        /// Staple width as a fraction of the box width. Empty: 0.5.
        pub staplewex: Vec<f64> => numbers f64,
        /// Width of the outlier lines (see `outlty`) as a fraction of the box
        /// width. Empty: 0.5.
        pub outwex: Vec<f64> => numbers f64,

        /// Default line type of the box, median and staples, and of the
        /// whiskers. Empty: [`par(lty)`] (the whiskers: dashed). `None`: that
        /// box's outline, median, whiskers and staples aren't drawn (unless
        /// their own type is given).
        ///
        /// [`par(lty)`]: crate::par::Par::lty
        pub lty: Vec<Option<LineType>> => na_values LineType,
        /// Default line width of every part (the median: 3 times it). Empty:
        /// [`par(lwd)`]. `None`: none of that box's lines or outliers are drawn
        /// (unless their own width is given).
        ///
        /// [`par(lwd)`]: crate::par::Par::lwd
        pub lwd: Vec<Option<f64>> => na_values f64,
        /// Default outlier symbol. Empty: [`par(pch)`]. `None`: that box's
        /// outliers aren't drawn (unless `outpch` is given).
        ///
        /// [`par(pch)`]: crate::par::Par::pch
        pub pch: Vec<Option<i32>> => pch Option<i32>,
        /// Default size of the median and outlier symbols. Empty: 1.
        /// `None`: that box's median and outlier symbols aren't drawn
        /// (unless their own size is given).
        pub cex: Vec<Option<f64>> => na_values f64,
        /// Default fill of the median and outlier symbols (21 to 25). Empty:
        /// [`par(bg)`].
        ///
        /// [`par(bg)`]: crate::par::Par::bg
        pub bg: Vec<ColorSpec> => values ColorSpec,

        /// Line type of the box outline. Empty: `lty`; `None`: no outline.
        pub boxlty: Vec<Option<LineType>> => na_values LineType,
        /// Line width of the box outline. Empty: `lwd`; `None`: no outline.
        pub boxlwd: Vec<Option<f64>> => na_values f64,
        /// Colour of the box outline. Empty: `border`.
        pub boxcol: Vec<ColorSpec> => values ColorSpec,
        /// Line type of the median. Empty: `lty`; `None`: no median line.
        pub medlty: Vec<Option<LineType>> => na_values LineType,
        /// Line width of the median. Empty: 3 times `lwd`; `None`: no median
        /// line.
        pub medlwd: Vec<Option<f64>> => na_values f64,
        /// Symbol drawn at the median. Empty: none; `None`: no median symbol.
        pub medpch: Vec<Option<i32>> => pch Option<i32>,
        /// Size of the median symbol. Empty: `cex`; `None`: no median symbol.
        pub medcex: Vec<Option<f64>> => na_values f64,
        /// Colour of the median line and symbol. Empty: `border`.
        pub medcol: Vec<ColorSpec> => values ColorSpec,
        /// Fill of the median symbol. Empty: `bg`.
        pub medbg: Vec<ColorSpec> => values ColorSpec,
        /// Line type of the whiskers. Empty: `lty`, else dashed; `None`: no
        /// whiskers.
        pub whisklty: Vec<Option<LineType>> => na_values LineType,
        /// Line width of the whiskers. Empty: `lwd`; `None`: no whiskers.
        pub whisklwd: Vec<Option<f64>> => na_values f64,
        /// Colour of the whiskers. Empty: `border`.
        pub whiskcol: Vec<ColorSpec> => values ColorSpec,
        /// Line type of the staples. Empty: `lty`; `None`: no staples.
        pub staplelty: Vec<Option<LineType>> => na_values LineType,
        /// Line width of the staples. Empty: `lwd`; `None`: no staples.
        pub staplelwd: Vec<Option<f64>> => na_values f64,
        /// Colour of the staples. Empty: `border`.
        pub staplecol: Vec<ColorSpec> => values ColorSpec,
        /// Line type of a short line across each outlier (`outwex` wide).
        /// Empty: blank (no line); `None`: no outlier lines.
        pub outlty: Vec<Option<LineType>> => na_values LineType,
        /// Line width of the outlier lines and symbols. Empty: `lwd`;
        /// `None`: no outlier lines or symbols.
        pub outlwd: Vec<Option<f64>> => na_values f64,
        /// Outlier symbol. Empty: `pch`; `None`: no outlier symbols.
        pub outpch: Vec<Option<i32>> => pch Option<i32>,
        /// Size of the outlier symbols. Empty: `cex`; `None`: no outlier
        /// symbols.
        pub outcex: Vec<Option<f64>> => na_values f64,
        /// Colour of the outlier lines and symbols. Empty: `border`.
        pub outcol: Vec<ColorSpec> => values ColorSpec,
        /// Fill of the outlier symbols. Empty: `bg`.
        pub outbg: Vec<ColorSpec> => values ColorSpec,
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
        pub col_axis: Option<ColorSpec> => option_value ColorSpec,
        /// Minimum gap between axis labels. `None`: R's default. R's
        /// `gap.axis`.
        pub gap_axis: Option<f64> => number f64,
        /// Type of the x axis: `AxisType::None` leaves it out. `None`:
        /// [`par(xaxt)`].
        ///
        /// [`par(xaxt)`]: crate::par::Par::xaxt
        pub xaxt: Option<AxisType> => option AxisType,
        /// Type of the y axis, as `xaxt`. `None`: [`par(yaxt)`].
        ///
        /// [`par(yaxt)`]: crate::par::Par::yaxt
        pub yaxt: Option<AxisType> => option AxisType,
        /// Default tick positions of the x axis (used by the value axis when
        /// `horizontal`). `None`: [`par(xaxp)`].
        ///
        /// [`par(xaxp)`]: crate::par::Par::xaxp
        pub xaxp: Option<[f64; 3]> => array [f64; 3],
        /// Default tick positions of the y axis, as `xaxp`. `None`:
        /// [`par(yaxp)`].
        ///
        /// [`par(yaxp)`]: crate::par::Par::yaxp
        pub yaxp: Option<[f64; 3]> => array [f64; 3],
        /// Draws the box around the plot region. `None`: `axes`. R's
        /// `frame.plot`.
        pub frame_plot: Option<bool> => option bool,
        /// Labels the boxes with `names`. `None`: when there is more than one
        /// box. R's `show.names`.
        pub show_names: Option<bool> => option bool,
        /// Options not in R ([`crate::extra`]).
        pub extra: crate::extra::PlotExtras => no_setter,
    }
}

impl Default for BoxplotOptions {
    fn default() -> Self {
        BoxplotOptions {
            deferred_error: None,
            extra: Default::default(),
            names: None,
            col: Vec::new(),
            border: Vec::new(),
            range: None,
            at: None,
            width: None,
            add: false,
            outline: true,
            xlog: false,
            ylog: false,
            xlim: None,
            ylim: None,
            main: None,
            sub: None,
            xlab: None,
            ylab: None,
            cex_main: None,
            col_main: None,
            cex_sub: None,
            col_sub: None,
            cex_lab: None,
            col_lab: None,
            ann: None,
            axes: true,
            notch: false,
            varwidth: false,
            horizontal: false,
            boxwex: Vec::new(),
            staplewex: Vec::new(),
            outwex: Vec::new(),
            lty: Vec::new(),
            lwd: Vec::new(),
            pch: Vec::new(),
            cex: Vec::new(),
            bg: Vec::new(),
            boxlty: Vec::new(),
            boxlwd: Vec::new(),
            boxcol: Vec::new(),
            medlty: Vec::new(),
            medlwd: Vec::new(),
            medpch: Vec::new(),
            medcex: Vec::new(),
            medcol: Vec::new(),
            medbg: Vec::new(),
            whisklty: Vec::new(),
            whisklwd: Vec::new(),
            whiskcol: Vec::new(),
            staplelty: Vec::new(),
            staplelwd: Vec::new(),
            staplecol: Vec::new(),
            outlty: Vec::new(),
            outlwd: Vec::new(),
            outpch: Vec::new(),
            outcex: Vec::new(),
            outcol: Vec::new(),
            outbg: Vec::new(),
            las: None,
            cex_axis: None,
            col_axis: None,
            gap_axis: None,
            xaxt: None,
            yaxt: None,
            xaxp: None,
            yaxp: None,
            frame_plot: None,
            show_names: None,
        }
    }
}

/// Each group's statistics, as [`Graphics::boxplot`](crate::graphics::Graphics::boxplot) returns them. Needs no
/// device, so a recorded figure can return the same result. Checks the
/// arguments as R does, so recording fails where drawing would.
pub(crate) fn boxplot_all_stats(
    groups: &[Vec<f64>],
    opts: &BoxplotOptions,
) -> Result<Vec<BoxStats>, Error> {
    let n = groups.len();
    if n == 0 {
        return Err(Error::invalid("need at least one group"));
    }
    if opts.at.as_ref().is_some_and(|at| at.len() != n) {
        return Err(Error::invalid(format!(
            "'at' must have same length as 'z$n', i.e. {n}"
        )));
    }
    if (opts.width.as_ref()).is_some_and(|w| w.len() != n || w.iter().any(|&v| v.is_nan() || v <= 0.0)) {
        return Err(Error::invalid("invalid boxplot widths"));
    }
    Ok(groups
        .iter()
        .map(|g| boxplot_stats(g, opts.range.unwrap_or(1.5)))
        .collect())
}

impl<B: Backend> Device<B> {
    /// Draws R's `boxplot()`; the public entry is
    /// [`Graphics::boxplot`](crate::graphics::Graphics::boxplot), whose docs
    /// describe it.
    pub(crate) fn boxplot_impl(
        &mut self,
        groups: &[Vec<f64>],
        opts: &BoxplotOptions,
    ) -> Result<Vec<BoxStats>, Error> {
        let n = groups.len();
        let stats = boxplot_all_stats(groups, opts)?;
        let at: Vec<f64> = opts
            .at
            .clone()
            .unwrap_or_else(|| (1..=n).map(|i| i as f64).collect());

        let ylim = opts.ylim.unwrap_or_else(|| {
            let mut lo = f64::INFINITY;
            let mut hi = f64::NEG_INFINITY;
            for s in &stats {
                let five = [s.lower_whisker, s.q1, s.median, s.q3, s.upper_whisker];
                let out = if opts.outline { &s.outliers[..] } else { &[] };
                let conf = if opts.notch { s.conf.to_vec() } else { vec![] };
                for &v in five.iter().chain(out).chain(&conf) {
                    if v.is_finite() {
                        lo = lo.min(v);
                        hi = hi.max(v);
                    }
                }
            }
            if lo <= hi { (lo, hi) } else { (0.0, 1.0) }
        });

        if !opts.add {
            let xlim = opts.xlim.unwrap_or_else(|| {
                let finite = at.iter().copied().filter(|v| v.is_finite());
                let lo = finite.clone().fold(f64::INFINITY, f64::min);
                let hi = finite.fold(f64::NEG_INFINITY, f64::max);
                (lo - 0.5, hi + 0.5)
            });
            self.plot_new_impl()?;
            let window = PlotWindowOptions {
                xlog: opts.xlog,
                ylog: opts.ylog,
                ..PlotWindowOptions::default()
            };
            if opts.horizontal {
                self.plot_window_impl(ylim, xlim, &window);
            } else {
                self.plot_window_impl(xlim, ylim, &window);
            }
        }
        crate::extra::panel_bg(self, &opts.extra)?;
        // Whether the position axis is logarithmic (from the device, as R's
        // `par()`, so it also holds with `add`).
        let pos_log = if opts.horizontal {
            self.ylog()
        } else {
            self.xlog()
        };

        // `boxplot()` passes `bxp()` `pars = list(boxwex = 0.8, ...)`, so
        // `bxp()`'s own default (from the gaps between `at`) never applies.
        let boxwex = |i: usize| nth(&opts.boxwex, i).unwrap_or(0.8);
        let max_n = stats.iter().map(|s| s.n).max().unwrap_or(0) as f64;
        let widths: Vec<f64> = (0..n)
            .map(|i| match &opts.width {
                Some(w) => {
                    let max = w.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                    boxwex(i) * w[i] / max
                }
                None if opts.varwidth => boxwex(i) * (stats[i].n as f64 / max_n).sqrt(),
                None if n == 1 => 0.5 * boxwex(i),
                None => boxwex(i),
            })
            .collect();

        // `bxp()`'s `xypolygon`/`xysegments`/`xypoints`: (position, value) in,
        // swapped for horizontal boxes.
        let horiz = opts.horizontal;
        let xy = |pos: &[f64], val: &[f64]| {
            if horiz {
                (val.to_vec(), pos.to_vec())
            } else {
                (pos.to_vec(), val.to_vec())
            }
        };
        // `xP()`: `x + w`, or `x * exp(w)` on a log position axis.
        let xp = |x: f64, w: f64| if pos_log { x * w.exp() } else { x + w };

        // `bxp()`'s per-box parameters (`pcycle`): the specific value, else
        // the general one, else `par()`. For style vectors the result is an
        // `Option` too: `None` (R's `NA`) leaves that part of the box out.
        let par_lty = self.par.lty;
        let par_lwd = self.par.lwd;
        let par_pch = self.par.pch;
        let par_bg = ColorSpec::Color(self.par.bg);
        let fg = ColorSpec::Color(self.par.fg);
        let lightgray = ColorSpec::Color(Color::parse("lightgray").unwrap());
        let lty = |i| nth(&opts.lty, i);
        let lwd = |i| nth(&opts.lwd, i).unwrap_or(Some(par_lwd));
        let cex = |i| nth(&opts.cex, i).unwrap_or(Some(1.0));
        let bg = |i| nth(&opts.bg, i).unwrap_or(par_bg);
        let border = |i| nth(&opts.border, i).unwrap_or(fg);
        let notch_frac = if opts.notch { 0.5 } else { 1.0 };
        for (i, s) in stats.iter().enumerate() {
            let x = at[i];
            let wid = widths[i] / 2.0;
            let five = [s.lower_whisker, s.q1, s.median, s.q3, s.upper_whisker];
            if five.iter().any(|v| v.is_nan()) {
                continue;
            }
            let line = |col: ColorSpec, lty: Option<LineType>, lwd: Option<f64>| SegmentsOptions {
                col: vec![col],
                lty: vec![lty],
                lwd: vec![lwd],
                ..SegmentsOptions::default()
            };
            // In `bxp()`'s order: box fill, median (line, then symbol),
            // whiskers, staples, then the box border on top, then outliers.
            let (box_pos, box_val): (Vec<f64>, Vec<f64>) = if opts.notch {
                let [c0, c1] = s.conf;
                let f = notch_frac;
                (
                    [-1.0, 1.0, 1.0, f, 1.0, 1.0, -1.0, -1.0, -f, -1.0]
                        .iter()
                        .map(|w| xp(x, wid * w))
                        .collect(),
                    vec![s.q1, s.q1, c0, s.median, c1, s.q3, s.q3, c1, s.median, c0],
                )
            } else {
                (
                    [-1.0, 1.0, 1.0, -1.0]
                        .iter()
                        .map(|w| xp(x, wid * w))
                        .collect(),
                    vec![s.q1, s.q1, s.q3, s.q3],
                )
            };
            let (bx, by) = xy(&box_pos, &box_val);
            let box_fill = PolygonOptions {
                col: vec![nth(&opts.col, i).unwrap_or(lightgray)],
                border: vec![ColorSpec::None],
                lty: vec![Some(LineType::BLANK)],
                ..PolygonOptions::default()
            };
            self.polygon_impl(&bx, &by, &box_fill)?;

            // The median, with `lend = 1` (butt), "to avoid overlap": round
            // ends would reach past the box sides. With a notch it spans
            // only the notch.
            let medcol = nth(&opts.medcol, i).unwrap_or(border(i));
            let wntch = notch_frac * wid;
            let (mx, my) = xy(&[xp(x, -wntch), xp(x, wntch)], &[s.median, s.median]);
            let median = SegmentsOptions {
                lend: Some(LineEnd::Butt),
                ..line(
                    medcol,
                    nth(&opts.medlty, i).or(lty(i)).unwrap_or(Some(par_lty)),
                    nth(&opts.medlwd, i).unwrap_or(lwd(i).map(|l| 3.0 * l)),
                )
            };
            self.segments_impl(&mx[..1], &my[..1], &mx[1..], &my[1..], &median)?;
            if let Some(pch) = nth(&opts.medpch, i) {
                let (px, py) = xy(&[x], &[s.median]);
                let symbol = XyOptions {
                    pch: vec![pch],
                    cex: vec![nth(&opts.medcex, i).unwrap_or(cex(i))],
                    col: vec![medcol],
                    bg: vec![nth(&opts.medbg, i).unwrap_or(bg(i))],
                    ..XyOptions::default()
                };
                self.points_impl(&px, &py, &symbol)?;
            }

            // Whiskers run from their ends to the hinges (which matters for
            // where the dashes fall), then the staples.
            let ends = [(s.lower_whisker, s.q1), (s.upper_whisker, s.q3)];
            let whisker = line(
                nth(&opts.whiskcol, i).unwrap_or(border(i)),
                nth(&opts.whisklty, i)
                    .or(lty(i))
                    .unwrap_or(Some(LineType::DASHED)),
                nth(&opts.whisklwd, i).unwrap_or(lwd(i)),
            );
            for &(w, h) in &ends {
                let (sx, sy) = xy(&[x, x], &[w, h]);
                self.segments_impl(&sx[..1], &sy[..1], &sx[1..], &sy[1..], &whisker)?;
            }
            let cap = wid * nth(&opts.staplewex, i).unwrap_or(0.5);
            let staple = line(
                nth(&opts.staplecol, i).unwrap_or(border(i)),
                nth(&opts.staplelty, i).or(lty(i)).unwrap_or(Some(par_lty)),
                nth(&opts.staplelwd, i).unwrap_or(lwd(i)),
            );
            for &(w, _) in &ends {
                let (sx, sy) = xy(&[xp(x, -cap), xp(x, cap)], &[w, w]);
                self.segments_impl(&sx[..1], &sy[..1], &sx[1..], &sy[1..], &staple)?;
            }
            if let Some(boxlwd) = nth(&opts.boxlwd, i).unwrap_or(lwd(i)) {
                let box_border = PolygonOptions {
                    col: vec![ColorSpec::None],
                    border: vec![nth(&opts.boxcol, i).unwrap_or(border(i))],
                    lty: vec![nth(&opts.boxlty, i).or(lty(i)).unwrap_or(Some(par_lty))],
                    lwd: Some(boxlwd),
                    ..PolygonOptions::default()
                };
                self.polygon_impl(&bx, &by, &box_border)?;
            }

            if opts.outline && !s.outliers.is_empty() {
                let outcol = nth(&opts.outcol, i).unwrap_or(border(i));
                let outlwd = nth(&opts.outlwd, i).unwrap_or(lwd(i));
                // A line across each outlier; blank (nothing) by default. R
                // computes its ends as `x -/+ w` even on a log position axis;
                // we use `xP()`, as for every other part.
                let outlty = nth(&opts.outlty, i).unwrap_or(Some(LineType::BLANK));
                if let Some(outlty) = outlty.filter(|&l| l != LineType::BLANK) {
                    let half = wid * nth(&opts.outwex, i).unwrap_or(0.5);
                    let k = s.outliers.len();
                    let (x0, y0) = xy(&vec![xp(x, -half); k], &s.outliers);
                    let (x1, y1) = xy(&vec![xp(x, half); k], &s.outliers);
                    self.segments_impl(&x0, &y0, &x1, &y1, &line(outcol, Some(outlty), outlwd))?;
                }
                let pos: Vec<f64> = s.outliers.iter().map(|_| x).collect();
                let (ox, oy) = xy(&pos, &s.outliers);
                // R's default `outcex = par(cex)` is then multiplied by
                // par(cex) again, squaring it; we apply it once, as for any
                // point.
                let outliers = XyOptions {
                    pch: vec![
                        nth(&opts.outpch, i)
                            .or(nth(&opts.pch, i))
                            .unwrap_or(Some(par_pch)),
                    ],
                    cex: vec![nth(&opts.outcex, i).unwrap_or(cex(i))],
                    col: vec![outcol],
                    bg: vec![nth(&opts.outbg, i).unwrap_or(bg(i))],
                    lwd: vec![outlwd],
                    ..XyOptions::default()
                };
                self.points_impl(&ox, &oy, &outliers)?;
            }
        }

        // `bxp()`: names on side 1 (2 if horizontal), values on side 2 (1),
        // then the title, then the frame.
        let (name_side, value_side) = if opts.horizontal { (2, 1) } else { (1, 2) };
        if opts.axes {
            // `bxp()` passes only these of its `pars` to the axes, with
            // `xaxt`/`xaxp` reaching the x axis and `yaxt`/`yaxp` the y axis.
            let inline = InlineAxis {
                las: opts.las,
                cex_axis: opts.cex_axis,
                col_axis: opts.col_axis,
                gap_axis: opts.gap_axis,
                ..InlineAxis::default()
            };
            let side_pars = |side: i32| {
                let (axt, axp) = if side == 1 {
                    (opts.xaxt, opts.xaxp)
                } else {
                    (opts.yaxt, opts.yaxp)
                };
                AxisOptions {
                    axt,
                    axp,
                    ..AxisOptions::default()
                }
            };
            if opts.show_names.unwrap_or(n > 1) {
                let labels = opts
                    .names
                    .clone()
                    .unwrap_or_else(|| (1..=n).map(|i| Some(Label::from(i.to_string()))).collect());
                let names = AxisOptions {
                    at: Some(at.clone()),
                    labels: Some(labels),
                    ..side_pars(name_side)
                };
                let names = crate::extra::rotated_axis(&opts.extra, names);
                self.axis_impl(name_side, &inline.apply(names))?;
            }
            self.axis_impl(value_side, &inline.apply(side_pars(value_side)))?;
        }

        if opts.ann.unwrap_or(!opts.add) {
            self.title_impl(&TitleOptions {
                main: opts.main.clone(),
                sub: opts.sub.clone(),
                xlab: opts.xlab.clone(),
                ylab: opts.ylab.clone(),
                cex_main: opts.cex_main,
                col_main: opts.col_main,
                cex_sub: opts.cex_sub,
                col_sub: opts.col_sub,
                cex_lab: opts.cex_lab,
                col_lab: opts.col_lab,
                ..TitleOptions::default()
            });
        }

        if opts.frame_plot.unwrap_or(opts.axes) {
            self.box_impl(&BoxOptions::default())?;
        }

        Ok(stats)
    }
}
