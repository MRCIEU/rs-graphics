// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! `boxplot(x, ...)`, ported from `boxplot.default`/`bxp`/`boxplot.stats`
//! in `r-source/src/library/graphics/R/boxplot.R`, for a list of numeric
//! groups: positions (`at`), widths, `notch`, `varwidth`, `horizontal`,
//! `add`, `outline`, log axes and `bxp()`'s drawing parameters.

use crate::backend::{Backend, LineEnd};
use crate::color::{Color, ColorSpec};
use crate::device::{
    AxisOptions, BoxOptions, Device, PlotWindowOptions, PolygonOptions, SegmentsOptions,
    TitleOptions, XyOptions,
};
use crate::lty::LineType;
use crate::numbers::Numbers;
use crate::par::AxisType;
use crate::plotmath::Label;

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

/// A single box's statistics, matching `boxplot.stats()$stats` plus
/// `$out` (the values `boxplot.stats()` reports as outliers).
#[derive(Clone, Debug)]
pub struct BoxStats {
    pub lower_whisker: f64,
    pub q1: f64,
    pub median: f64,
    pub q3: f64,
    pub upper_whisker: f64,
    pub outliers: Vec<f64>,
    /// The number of non-`NaN` values (`boxplot.stats()$n`).
    pub n: usize,
    /// The notch: `median -/+ 1.58 * IQR / sqrt(n)` (`boxplot.stats()$conf`).
    pub conf: (f64, f64),
}

/// `boxplot.stats(x, coef)`: five-number summary + whiskers extended to
/// the most extreme non-outlying data point, +/- `coef * IQR` beyond the
/// hinges defining an outlier.
pub fn boxplot_stats<'a>(x: impl Numbers<'a>, coef: impl Into<f64>) -> BoxStats {
    let (x, coef) = (x.into_numbers(), coef.into());
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
            conf: (f64::NAN, f64::NAN),
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
        conf: (five[2] - half_notch, five[2] + half_notch),
    }
}

/// Options for [`Graphics::boxplot`](crate::graphics::Graphics::boxplot):
/// the arguments of R's `boxplot.default` and `bxp()`, including the
/// drawing parameters R takes in `pars` or directly (`boxplot(x, medlwd =
/// 2)`). Unset fields are R's defaults, given for each.
///
/// Like R's, the per-box values (`col`, `border`, `boxwex` and every
/// `box*`/`med*`/`whisk*`/`staple*`/`out*` style) are recycled over the
/// boxes. The style of each part falls back to the general `lty`, `lwd`,
/// `pch`, `cex` and `bg` here, then to `par()`, as in `bxp()`. Of the
/// title parameters, only the `cex.*` and `col.*` ones are passed to
/// `title()`, as `bxp()` does (`font.main` and co. come from `par()`), and
/// the frame follows `par(bty)` (`bxp()`'s `box()` takes no `bty`).
#[derive(Clone, Debug)]
pub struct BoxplotOptions {
    /// Group labels on the name axis (side 1, or 2 if `horizontal`);
    /// `None` gives `1, 2, ...`. Only drawn when there's more than one
    /// box, matching R's `show.names <- n > 1`.
    pub names: Option<Vec<Label>>,
    /// Box fills (R's `col`, passed to `bxp()` as `boxfill`), recycled
    /// over the boxes; empty: `"lightgray"`.
    pub col: Vec<ColorSpec>,
    /// Colours of the box outlines, median, whiskers, staples and
    /// outliers, recycled over the boxes; empty: `par(fg)`.
    pub border: Vec<ColorSpec>,
    /// Whiskers reach the most extreme point within `range` times the
    /// box length (IQR) of the box; points beyond are outliers. `None`:
    /// 1.5 (R's default).
    pub range: Option<f64>,
    /// Box positions on the name axis, one per group; `None`: `1, 2,
    /// ..., n`.
    pub at: Option<Vec<f64>>,
    /// Relative box widths, one per group (scaled so the widest is
    /// `boxwex`); `None`: equal widths, or by group size with `varwidth`;
    /// a single box is half of `boxwex` wide (as `bxp()`).
    pub width: Option<Vec<f64>>,
    /// Draw on the current plot (R's `add = TRUE`) instead of starting a
    /// new one. Default `false`.
    pub add: bool,
    /// Draw outliers (and include them in the default value range).
    /// Default `true`.
    pub outline: bool,
    /// Log x axis (R's `log = "x"`): the position axis, or the value axis
    /// when `horizontal`. Default `false`.
    pub xlog: bool,
    /// Log y axis (R's `log = "y"`): the value axis, or the position axis
    /// when `horizontal`. Default `false`.
    pub ylog: bool,
    /// Limits of the position axis; `None`: `range(at) + c(-0.5, 0.5)`.
    pub xlim: Option<(f64, f64)>,
    /// Limits of the value axis (also when `horizontal`, as in R);
    /// `None`: the range of the statistics, plus the outliers (unless
    /// `outline` is off) and the notches (with `notch`).
    pub ylim: Option<(f64, f64)>,
    /// Main title, as for `plot()` (text or a plotmath expression);
    /// `None`: none.
    pub main: Option<Label>,
    /// Subtitle; `None`: none.
    pub sub: Option<Label>,
    /// x axis label; `None`: none.
    pub xlab: Option<Label>,
    /// y axis label; `None`: none.
    pub ylab: Option<Label>,
    /// Size of `main`, relative to `par(cex)` (R's `cex.main`, passed
    /// to `title()`); `None`: `par(cex.main)`.
    pub cex_main: Option<f64>,
    /// Colour of `main` (R's `col.main`, passed to `title()`);
    /// `None`: `par(col.main)`.
    pub col_main: Option<ColorSpec>,
    /// Size of `sub`, relative to `par(cex)` (R's `cex.sub`, passed
    /// to `title()`); `None`: `par(cex.sub)`.
    pub cex_sub: Option<f64>,
    /// Colour of `sub` (R's `col.sub`, passed to `title()`);
    /// `None`: `par(col.sub)`.
    pub col_sub: Option<ColorSpec>,
    /// Size of `xlab` and `ylab`, relative to `par(cex)` (R's `cex.lab`, passed
    /// to `title()`); `None`: `par(cex.lab)`.
    pub cex_lab: Option<f64>,
    /// Colour of `xlab` and `ylab` (R's `col.lab`, passed to `title()`);
    /// `None`: `par(col.lab)`.
    pub col_lab: Option<ColorSpec>,
    /// Draw the titles (R's `ann`); `None`: unless `add` (R's default
    /// `ann = !add`, not `par(ann)`).
    pub ann: Option<bool>,
    /// Draw the axes and the box around the plot (also with `add`, as in
    /// R); default `true`.
    pub axes: bool,
    /// Notched boxes: the box narrows to half its width at the median,
    /// between [`BoxStats::conf`]'s limits (R's `notch.frac = 0.5`).
    /// Default `false`.
    pub notch: bool,
    /// Box widths proportional to the square root of the group size.
    /// Default `false`.
    pub varwidth: bool,
    /// Horizontal boxes: group names on side 2, values on side 1. Default
    /// `false`.
    pub horizontal: bool,

    /// Box widths, in units of the position axis (of its natural log on a
    /// log axis); empty: 0.8 (R's `boxplot()` default, whatever `at` is).
    pub boxwex: Vec<f64>,
    /// Staple width as a fraction of the box width; empty: 0.5.
    pub staplewex: Vec<f64>,
    /// Width of the outlier lines (see `outlty`) as a fraction of the box
    /// width; empty: 0.5.
    pub outwex: Vec<f64>,

    /// Default line type of the box, median and staples, and of the
    /// whiskers; empty: `par(lty)` (the whiskers: dashed).
    pub lty: Vec<LineType>,
    /// Default line width of every part (the median: 3 times it); empty:
    /// `par(lwd)`.
    pub lwd: Vec<f64>,
    /// Default outlier symbol; empty: `par(pch)`.
    pub pch: Vec<i32>,
    /// Default size of the median and outlier symbols; empty: 1.
    pub cex: Vec<f64>,
    /// Default fill of the median and outlier symbols (21 to 25); empty:
    /// `par(bg)`.
    pub bg: Vec<ColorSpec>,

    /// Line type of the box outline; empty: `lty`.
    pub boxlty: Vec<LineType>,
    /// Line width of the box outline; empty: `lwd`.
    pub boxlwd: Vec<f64>,
    /// Colour of the box outline; empty: `border`.
    pub boxcol: Vec<ColorSpec>,
    /// Line type of the median; empty: `lty`.
    pub medlty: Vec<LineType>,
    /// Line width of the median; empty: 3 times `lwd`.
    pub medlwd: Vec<f64>,
    /// A symbol drawn at the median; empty: none.
    pub medpch: Vec<i32>,
    /// Size of the median symbol; empty: `cex`.
    pub medcex: Vec<f64>,
    /// Colour of the median line and symbol; empty: `border`.
    pub medcol: Vec<ColorSpec>,
    /// Fill of the median symbol; empty: `bg`.
    pub medbg: Vec<ColorSpec>,
    /// Line type of the whiskers; empty: `lty`, else dashed.
    pub whisklty: Vec<LineType>,
    /// Line width of the whiskers; empty: `lwd`.
    pub whisklwd: Vec<f64>,
    /// Colour of the whiskers; empty: `border`.
    pub whiskcol: Vec<ColorSpec>,
    /// Line type of the staples; empty: `lty`.
    pub staplelty: Vec<LineType>,
    /// Line width of the staples; empty: `lwd`.
    pub staplelwd: Vec<f64>,
    /// Colour of the staples; empty: `border`.
    pub staplecol: Vec<ColorSpec>,
    /// Line type of a short line across each outlier (`outwex` wide);
    /// empty: blank (no line).
    pub outlty: Vec<LineType>,
    /// Line width of the outlier lines and symbols; empty: `lwd`.
    pub outlwd: Vec<f64>,
    /// Outlier symbol; empty: `pch`.
    pub outpch: Vec<i32>,
    /// Size of the outlier symbols; empty: `cex`.
    pub outcex: Vec<f64>,
    /// Colour of the outlier lines and symbols; empty: `border`.
    pub outcol: Vec<ColorSpec>,
    /// Fill of the outlier symbols; empty: `bg`.
    pub outbg: Vec<ColorSpec>,
    /// Axis label orientation (R's inline `las`, passed to `axis()`): 0
    /// parallel to the axis, 1 horizontal, 2 perpendicular, 3 vertical;
    /// `None`: `par(las)`.
    pub las: Option<i32>,
    /// Axis label size (R's inline `cex.axis`); `None`: `par(cex.axis)`.
    pub cex_axis: Option<f64>,
    /// Axis label colour (R's inline `col.axis`); `None`: `par(col.axis)`.
    pub col_axis: Option<ColorSpec>,
    /// Minimum gap between axis labels (R's inline `gap.axis`); `None`:
    /// R's default.
    pub gap_axis: Option<f64>,
    /// `AxisType::None` leaves out the x axis (R's `xaxt`, which `bxp()`
    /// passes to its axes); `None`: `par(xaxt)`.
    pub xaxt: Option<AxisType>,
    /// As `xaxt`, for the y axis; `None`: `par(yaxt)`.
    pub yaxt: Option<AxisType>,
    /// The default ticks of the x axis (R's `xaxp`; used by the value axis
    /// when `horizontal`); `None`: `par(xaxp)`.
    pub xaxp: Option<[f64; 3]>,
    /// As `xaxp`, for the y axis; `None`: `par(yaxp)`.
    pub yaxp: Option<[f64; 3]>,
}

crate::setters::setters!(BoxplotOptions => [BoxplotCall] {
    names: list Label,
    col: values ColorSpec,
    border: values ColorSpec,
    range: option f64,
    at: option_values f64,
    width: option_values f64,
    add: value bool,
    outline: value bool,
    xlog: value bool,
    ylog: value bool,
    xlim: pair f64,
    ylim: pair f64,
    main: option Label,
    sub: option Label,
    xlab: option Label,
    ylab: option Label,
    cex_main: option f64,
    col_main: option ColorSpec,
    cex_sub: option f64,
    col_sub: option ColorSpec,
    cex_lab: option f64,
    col_lab: option ColorSpec,
    ann: option bool,
    axes: value bool,
    notch: value bool,
    varwidth: value bool,
    horizontal: value bool,
    boxwex: values f64,
    staplewex: values f64,
    outwex: values f64,
    lty: values LineType,
    lwd: values f64,
    pch: values i32,
    cex: values f64,
    bg: values ColorSpec,
    boxlty: values LineType,
    boxlwd: values f64,
    boxcol: values ColorSpec,
    medlty: values LineType,
    medlwd: values f64,
    medpch: values i32,
    medcex: values f64,
    medcol: values ColorSpec,
    medbg: values ColorSpec,
    whisklty: values LineType,
    whisklwd: values f64,
    whiskcol: values ColorSpec,
    staplelty: values LineType,
    staplelwd: values f64,
    staplecol: values ColorSpec,
    outlty: values LineType,
    outlwd: values f64,
    outpch: values i32,
    outcex: values f64,
    outcol: values ColorSpec,
    outbg: values ColorSpec,
    las: option i32,
    cex_axis: option f64,
    col_axis: option ColorSpec,
    gap_axis: option f64,
    xaxt: option AxisType,
    yaxt: option AxisType,
    xaxp: array [f64; 3],
    yaxp: array [f64; 3],
});

impl Default for BoxplotOptions {
    fn default() -> Self {
        BoxplotOptions {
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
        }
    }
}

/// Each group's statistics, as [`Graphics::boxplot`](crate::graphics::Graphics::boxplot) returns them. Needs no
/// device, so a recorded figure can return the same result. Checks the
/// arguments as R does, so recording panics where drawing would.
pub(crate) fn boxplot_all_stats(groups: &[Vec<f64>], opts: &BoxplotOptions) -> Vec<BoxStats> {
    let n = groups.len();
    assert!(n > 0, "boxplot: need at least one group");
    if let Some(at) = &opts.at {
        assert!(
            at.len() == n,
            "boxplot: 'at' must have the same length as the number of groups, i.e. {n}"
        );
    }
    if let Some(w) = &opts.width {
        assert!(
            w.len() == n && w.iter().all(|&v| v > 0.0),
            "boxplot: invalid boxplot widths"
        );
    }
    groups
        .iter()
        .map(|g| boxplot_stats(g, opts.range.unwrap_or(1.5)))
        .collect()
}

/// R's `pcycle()`: `v`'s `i`-th value, recycled, or `None` if `v` is empty.
fn cycle<T: Copy>(v: &[T], i: usize) -> Option<T> {
    (!v.is_empty()).then(|| v[i % v.len()])
}

impl<B: Backend> Device<B> {
    /// Draws R's `boxplot()`; the public entry is
    /// [`Graphics::boxplot`](crate::graphics::Graphics::boxplot), whose docs
    /// describe it.
    pub(crate) fn boxplot_impl(
        &mut self,
        groups: &[Vec<f64>],
        opts: &BoxplotOptions,
    ) -> Vec<BoxStats> {
        let n = groups.len();
        let stats = boxplot_all_stats(groups, opts);
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
                let conf = if opts.notch {
                    vec![s.conf.0, s.conf.1]
                } else {
                    vec![]
                };
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
            self.plot_new_impl();
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
        // Whether the position axis is logarithmic (from the device, as R's
        // `par()`, so it also holds with `add`).
        let pos_log = if opts.horizontal {
            self.ylog()
        } else {
            self.xlog()
        };

        // `boxplot()` passes `bxp()` `pars = list(boxwex = 0.8, ...)`, so
        // `bxp()`'s own default (from the gaps between `at`) never applies.
        let boxwex = |i: usize| cycle(&opts.boxwex, i).unwrap_or(0.8);
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
        // the general one, else `par()`.
        let par_lty = self.par.lty;
        let par_lwd = self.par.lwd;
        let par_pch = self.par.pch;
        let par_bg = ColorSpec::Color(self.par.bg);
        let fg = ColorSpec::Color(self.par.fg);
        let lightgray = ColorSpec::Color(Color::parse("lightgray").unwrap());
        let lty = |i| cycle(&opts.lty, i);
        let lwd = |i| cycle(&opts.lwd, i).unwrap_or(par_lwd);
        let cex = |i| cycle(&opts.cex, i).unwrap_or(1.0);
        let bg = |i| cycle(&opts.bg, i).unwrap_or(par_bg);
        let border = |i| cycle(&opts.border, i).unwrap_or(fg);
        let notch_frac = if opts.notch { 0.5 } else { 1.0 };
        for (i, s) in stats.iter().enumerate() {
            let x = at[i];
            let wid = widths[i] / 2.0;
            let five = [s.lower_whisker, s.q1, s.median, s.q3, s.upper_whisker];
            if five.iter().any(|v| v.is_nan()) {
                continue;
            }
            let line = |col: ColorSpec, lty: LineType, lwd: f64| SegmentsOptions {
                col: vec![col],
                lty: vec![lty],
                lwd: vec![lwd],
                ..SegmentsOptions::default()
            };
            // In `bxp()`'s order: box fill, median (line, then symbol),
            // whiskers, staples, then the box border on top, then outliers.
            let (box_pos, box_val): (Vec<f64>, Vec<f64>) = if opts.notch {
                let (c0, c1) = s.conf;
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
                col: vec![cycle(&opts.col, i).unwrap_or(lightgray)],
                border: vec![ColorSpec::None],
                lty: vec![LineType::BLANK],
                ..PolygonOptions::default()
            };
            self.polygon_impl(&bx, &by, &box_fill);

            // The median, with `lend = 1` (butt), "to avoid overlap": round
            // ends would reach past the box sides. With a notch it spans
            // only the notch.
            let medcol = cycle(&opts.medcol, i).unwrap_or(border(i));
            let wntch = notch_frac * wid;
            let (mx, my) = xy(&[xp(x, -wntch), xp(x, wntch)], &[s.median, s.median]);
            let median = SegmentsOptions {
                lend: Some(LineEnd::Butt),
                ..line(
                    medcol,
                    cycle(&opts.medlty, i).or(lty(i)).unwrap_or(par_lty),
                    cycle(&opts.medlwd, i).unwrap_or(3.0 * lwd(i)),
                )
            };
            self.segments_impl(&mx[..1], &my[..1], &mx[1..], &my[1..], &median);
            if let Some(pch) = cycle(&opts.medpch, i) {
                let (px, py) = xy(&[x], &[s.median]);
                let symbol = XyOptions {
                    pch: vec![pch],
                    cex: vec![cycle(&opts.medcex, i).unwrap_or(cex(i))],
                    col: vec![medcol],
                    bg: vec![cycle(&opts.medbg, i).unwrap_or(bg(i))],
                    ..XyOptions::default()
                };
                self.points_impl(&px, &py, &symbol);
            }

            // Whiskers run from their ends to the hinges (which matters for
            // where the dashes fall), then the staples.
            let ends = [(s.lower_whisker, s.q1), (s.upper_whisker, s.q3)];
            let whisker = line(
                cycle(&opts.whiskcol, i).unwrap_or(border(i)),
                cycle(&opts.whisklty, i)
                    .or(lty(i))
                    .unwrap_or(LineType::DASHED),
                cycle(&opts.whisklwd, i).unwrap_or(lwd(i)),
            );
            for &(w, h) in &ends {
                let (sx, sy) = xy(&[x, x], &[w, h]);
                self.segments_impl(&sx[..1], &sy[..1], &sx[1..], &sy[1..], &whisker);
            }
            let cap = wid * cycle(&opts.staplewex, i).unwrap_or(0.5);
            let staple = line(
                cycle(&opts.staplecol, i).unwrap_or(border(i)),
                cycle(&opts.staplelty, i).or(lty(i)).unwrap_or(par_lty),
                cycle(&opts.staplelwd, i).unwrap_or(lwd(i)),
            );
            for &(w, _) in &ends {
                let (sx, sy) = xy(&[xp(x, -cap), xp(x, cap)], &[w, w]);
                self.segments_impl(&sx[..1], &sy[..1], &sx[1..], &sy[1..], &staple);
            }
            let box_border = PolygonOptions {
                col: vec![ColorSpec::None],
                border: vec![cycle(&opts.boxcol, i).unwrap_or(border(i))],
                lty: vec![cycle(&opts.boxlty, i).or(lty(i)).unwrap_or(par_lty)],
                lwd: Some(cycle(&opts.boxlwd, i).unwrap_or(lwd(i))),
                ..PolygonOptions::default()
            };
            self.polygon_impl(&bx, &by, &box_border);

            if opts.outline && !s.outliers.is_empty() {
                let outcol = cycle(&opts.outcol, i).unwrap_or(border(i));
                let outlwd = cycle(&opts.outlwd, i).unwrap_or(lwd(i));
                // A line across each outlier; blank (nothing) by default. R
                // computes its ends as `x -/+ w` even on a log position axis;
                // we use `xP()`, as for every other part.
                let outlty = cycle(&opts.outlty, i).unwrap_or(LineType::BLANK);
                if outlty != LineType::BLANK {
                    let half = wid * cycle(&opts.outwex, i).unwrap_or(0.5);
                    let k = s.outliers.len();
                    let (x0, y0) = xy(&vec![xp(x, -half); k], &s.outliers);
                    let (x1, y1) = xy(&vec![xp(x, half); k], &s.outliers);
                    self.segments_impl(&x0, &y0, &x1, &y1, &line(outcol, outlty, outlwd));
                }
                let pos: Vec<f64> = s.outliers.iter().map(|_| x).collect();
                let (ox, oy) = xy(&pos, &s.outliers);
                // R's default `outcex = par(cex)` is then multiplied by
                // par(cex) again, squaring it; we apply it once, as for any
                // point.
                let outliers = XyOptions {
                    pch: vec![
                        cycle(&opts.outpch, i)
                            .or(cycle(&opts.pch, i))
                            .unwrap_or(par_pch),
                    ],
                    cex: vec![cycle(&opts.outcex, i).unwrap_or(cex(i))],
                    col: vec![outcol],
                    bg: vec![cycle(&opts.outbg, i).unwrap_or(bg(i))],
                    lwd: vec![outlwd],
                    ..XyOptions::default()
                };
                self.points_impl(&ox, &oy, &outliers);
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
            if n > 1 {
                let labels = opts
                    .names
                    .clone()
                    .unwrap_or_else(|| (1..=n).map(|i| Label::from(i.to_string())).collect());
                let names = AxisOptions {
                    at: Some(at.clone()),
                    labels: Some(labels.into_iter().map(Some).collect()),
                    ..side_pars(name_side)
                };
                self.axis_impl(name_side, &inline.apply(names));
            }
            self.axis_impl(value_side, &inline.apply(side_pars(value_side)));
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

        if opts.axes {
            self.box_impl(&BoxOptions::default());
        }

        stats
    }
}
