// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Histograms: the options of [`Graphics::hist`](crate::graphics::Graphics::hist),
//! how its bins are chosen ([`HistBreaks`]), the labels above its bars
//! ([`HistLabels`]) and the bins it returns ([`HistResult`]).
//!
//! Supported: every form of `breaks` (edges, a number of classes, the
//! `"Sturges"`/`"Scott"`/`"FD"` rules, a function), `xlim`/`ylim`, R's
//! binning with `right`/`include.lowest`/`fuzz`, `labels` and per-bar
//! `col`/`border`/`lty`, shading lines (`density`/`angle`), and `plot =
//! FALSE`. Not supported: `panel.first` (see `panel_bg`), `sub`,
//! `warn.unused`; R's `probability` and `nclass` are `.freq(false)` and
//! `.breaks(n)`.
//!
//! This is R's `hist(x, ...)`, ported from `hist.default`/`plot.histogram`
//! in `r-source/src/library/graphics/R/hist.R`.

use crate::backend::Backend;
use crate::color::{Color, ColorSpec};
use crate::device::{
    AxisOptions, Device, PlotWindowOptions, RectOptions, TextOptions, TitleOptions,
};
use crate::error::{Error, from_r_string};
use crate::font::FontFace;
use crate::lty::LineType;
use crate::numbers::Num;
use crate::plotmath::Label;
use crate::pretty::pretty_r;

use super::{InlineAxis, data_range};

/// Sturges' rule (`nclass.Sturges()`): `ceiling(log2(length(x)) + 1)`.
fn nclass_sturges(n: usize) -> f64 {
    if n == 0 {
        return 1.0;
    }
    ((n as f64).log2() + 1.0).ceil()
}

/// R's `var(x)`: the sample variance, with `cov()`'s corrected mean; NaN
/// for fewer than two values.
fn var(x: &[f64]) -> f64 {
    let n = x.len() as f64;
    if x.len() < 2 {
        return f64::NAN;
    }
    let mut mean = x.iter().sum::<f64>() / n;
    mean += x.iter().map(|v| v - mean).sum::<f64>() / n;
    x.iter().map(|v| (v - mean) * (v - mean)).sum::<f64>() / (n - 1.0)
}

/// R's `quantile(x, p, type = 7)` of sorted `x`.
fn quantile7(sorted: &[f64], p: f64) -> f64 {
    let n = sorted.len();
    let index = 1.0 + (n.saturating_sub(1)) as f64 * p;
    let (lo, hi) = (index.floor(), index.ceil());
    let qs = sorted[lo as usize - 1];
    let x_hi = sorted[hi as usize - 1];
    if index > lo && x_hi != qs {
        let h = index - lo;
        (1.0 - h) * qs + h * x_hi
    } else {
        qs
    }
}

/// R's `signif(x, digits)` (`fprec()`: round half to even at `digits`
/// significant digits).
fn signif(x: f64, digits: i32) -> f64 {
    const MAX10E: i32 = f64::MAX_10_EXP;
    if !x.is_finite() || x == 0.0 {
        return x;
    }
    let (sgn, x) = (x.signum(), x.abs());
    let l10 = x.log10();
    if l10.abs() >= (MAX10E - 2) as f64 {
        return sgn * x;
    }
    let mut e10 = digits - 1 - l10.floor() as i32;
    let mut p10 = 1.0;
    if e10 > MAX10E {
        p10 = 10f64.powi(e10 - MAX10E);
        e10 = MAX10E;
    }
    if e10 > 0 {
        let pow10 = 10f64.powi(e10);
        sgn * ((x * pow10 * p10).round_ties_even() / pow10) / p10
    } else {
        let pow10 = 10f64.powi(-e10);
        sgn * (x / pow10).round_ties_even() * pow10
    }
}

/// `diff(range(x))`.
fn spread(x: &[f64]) -> f64 {
    let lo = x.iter().cloned().fold(f64::INFINITY, f64::min);
    let hi = x.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    hi - lo
}

/// Scott's rule (`nclass.scott()`).
fn nclass_scott(x: &[f64]) -> f64 {
    let h = 3.5 * var(x).sqrt() * (x.len() as f64).powf(-1.0 / 3.0);
    if h > 0.0 {
        (spread(x) / h).ceil().max(1.0)
    } else {
        1.0
    }
}

/// The Freedman-Diaconis rule (`nclass.FD()`, `digits = 5`).
fn nclass_fd(x: &[f64]) -> f64 {
    let mut rounded: Vec<f64> = x.iter().map(|&v| signif(v, 5)).collect();
    rounded.sort_by(f64::total_cmp);
    let mut h = 2.0 * (quantile7(&rounded, 0.75) - quantile7(&rounded, 0.25));
    if h == 0.0 {
        // Try quantiles 1/8, 1/16, ... 1/512.
        let mut al = 0.25;
        while h == 0.0 {
            al /= 2.0;
            if al < 1.0 / 512.0 {
                break;
            }
            h = (quantile7(&rounded, 1.0 - al) - quantile7(&rounded, al)) / (1.0 - 2.0 * al);
        }
    }
    if h == 0.0 {
        h = 3.5 * var(x).sqrt();
    }
    if h > 0.0 {
        (spread(x) / h * (x.len() as f64).powf(1.0 / 3.0)).ceil()
    } else {
        1.0
    }
}

/// A function of the data giving edges or a number of classes.
type BreaksFn = std::sync::Arc<dyn Fn(&[f64]) -> Vec<f64> + Send + Sync>;

/// How a histogram's bins are chosen: a rule, a suggested number of
/// classes, the bin edges, or a function of the data.
///
/// A rule or a number of classes `k` gives the edges `pretty(range(x), n =
/// k, min.n = 1)`; edges are used as given (after sorting). Setters take
/// a rule's name (`.breaks("FD")`, see below; an unknown name is an error of
/// the call), a number (`.breaks(20)`) or several numbers as edges
/// (`.breaks([0, 2, 5, 10])`; one value in an array, `Vec` or slice is a
/// number of classes, as R's length rule).
///
/// From a string (`"FD".parse::<HistBreaks>()` or `TryFrom<&str>`): a rule's
/// name, `"Sturges"`, `"Scott"`, `"FD"` or `"Freedman-Diaconis"`, in any case
/// or as a unique prefix (R's `match.arg(tolower(breaks), ..)`). Anything
/// else is an error.
///
/// This is R's `hist(breaks = )`.
///
/// # Examples
///
/// ```
/// use rs_graphics::prelude::*;
///
/// # let mut dev = Device::null().open().unwrap();
/// let x = [1, 2, 2, 3, 3, 3];
/// let h = dev.hist().x(&x).breaks([0, 1, 2, 3]).draw();
/// assert_eq!(h.counts, [1.0, 2.0, 3.0]);
/// let h = dev.hist().x(&x).breaks(HistBreaks::Scott).draw();
/// assert!(h.equidist);
/// assert!(matches!("fd".parse::<HistBreaks>(), Ok(HistBreaks::FreedmanDiaconis)));
/// assert!("F".parse::<HistBreaks>().is_err()); // "fd" or "freedman-diaconis"
/// ```
#[derive(Clone)]
pub enum HistBreaks {
    /// `"Sturges"` (R's default): `ceiling(log2(n) + 1)` classes.
    Sturges,
    /// `"Scott"`: classes of width `3.5 * sd(x) * n^(-1/3)`.
    Scott,
    /// `"FD"` / `"Freedman-Diaconis"`: classes of width `2 * IQR(x) *
    /// n^(-1/3)` (on `x` rounded to 5 significant digits).
    FreedmanDiaconis,
    /// A suggested number of classes (R's `breaks = 20`).
    Count(f64),
    /// Bin edges (sorted before use, as R does).
    Edges(Vec<f64>),
    /// A function of the finite data returning edges, or one value, a
    /// number of classes (R's `breaks = function(x) ...`); see
    /// [`HistBreaks::function`].
    Function(BreaksFn),
}

impl HistBreaks {
    /// Bins chosen by a function of the finite data, which returns the
    /// edges, or one value, a number of classes.
    ///
    /// This is R's `breaks = function(x) ...`.
    ///
    /// # Examples
    ///
    /// ```
    /// use rs_graphics::prelude::*;
    ///
    /// # let mut dev = Device::null().open().unwrap();
    /// // Edges at the data's minimum, middle and maximum.
    /// let edges = HistBreaks::function(|x| {
    ///     let lo = x.iter().copied().fold(f64::INFINITY, f64::min);
    ///     let hi = x.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    ///     vec![lo, (lo + hi) / 2.0, hi]
    /// });
    /// let h = dev.hist().x(&[5, 2, 3, 1, 4]).breaks(edges).draw();
    /// assert_eq!(h.breaks, [1.0, 3.0, 5.0]);
    /// ```
    pub fn function(f: impl Fn(&[f64]) -> Vec<f64> + Send + Sync + 'static) -> Self {
        HistBreaks::Function(std::sync::Arc::new(f))
    }

    /// A rule's name or a unique prefix, `None` otherwise; the public routes
    /// are `FromStr`/`TryFrom<&str>`.
    pub(crate) fn parse(s: &str) -> Option<HistBreaks> {
        const ALL: [&str; 4] = ["sturges", "fd", "freedman-diaconis", "scott"];
        let s = s.to_lowercase();
        let name = ALL.iter().find(|&&a| a == s).copied().or_else(|| {
            let mut it = ALL.iter().filter(|a| !s.is_empty() && a.starts_with(&s));
            match (it.next(), it.next()) {
                (Some(&a), None) => Some(a),
                _ => None,
            }
        })?;
        Some(match name {
            "sturges" => HistBreaks::Sturges,
            "scott" => HistBreaks::Scott,
            _ => HistBreaks::FreedmanDiaconis,
        })
    }
}

impl std::fmt::Debug for HistBreaks {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HistBreaks::Sturges => write!(f, "Sturges"),
            HistBreaks::Scott => write!(f, "Scott"),
            HistBreaks::FreedmanDiaconis => write!(f, "FreedmanDiaconis"),
            HistBreaks::Count(k) => write!(f, "Count({k})"),
            HistBreaks::Edges(e) => write!(f, "Edges({e:?})"),
            HistBreaks::Function(_) => write!(f, "Function(..)"),
        }
    }
}

from_r_string!(
    HistBreaks,
    "histogram breaks rule",
    "\"Sturges\", \"Scott\", \"FD\" or \"Freedman-Diaconis\""
);

impl From<f64> for HistBreaks {
    /// A number of classes.
    fn from(k: f64) -> Self {
        HistBreaks::Count(k)
    }
}

impl From<i32> for HistBreaks {
    /// A number of classes.
    fn from(k: i32) -> Self {
        HistBreaks::Count(k.into())
    }
}

/// Values: one is a number of classes, more are edges (R's length rule).
fn values_to_breaks(v: Vec<f64>) -> HistBreaks {
    match v.as_slice() {
        [k] => HistBreaks::Count(*k),
        _ => HistBreaks::Edges(v),
    }
}

impl<T: Num, const N: usize> From<[T; N]> for HistBreaks {
    /// Edges (or, for one value, a number of classes).
    fn from(v: [T; N]) -> Self {
        values_to_breaks(v.iter().map(|x| x.to_f64()).collect())
    }
}

impl<T: Num> From<Vec<T>> for HistBreaks {
    /// Edges (or, for one value, a number of classes).
    fn from(v: Vec<T>) -> Self {
        values_to_breaks(v.iter().map(|x| x.to_f64()).collect())
    }
}

impl<T: Num> From<&[T]> for HistBreaks {
    /// Edges (or, for one value, a number of classes).
    fn from(v: &[T]) -> Self {
        values_to_breaks(v.iter().map(|x| x.to_f64()).collect())
    }
}

/// `hist.default`'s counts: the edges moved by `fuzz` times a typical bin
/// width (so values "effectively on" an edge fall the intended way), then
/// R's `C_bincount()` with `right` (bins `(a, b]`, else `[a, b)`) and
/// `include_lowest` (the first bin closed on the left, or with `right =
/// false` the last closed on the right).
///
/// # Errors
/// If some values fall outside the edges (R: "some 'x' not counted; maybe
/// 'breaks' do not span range of 'x'").
fn bin_counts(
    data: &[f64],
    breaks: &[f64],
    right: bool,
    include_lowest: bool,
    fuzz: f64,
) -> Result<Vec<f64>, Error> {
    let nb = breaks.len();
    let mut counts = vec![0.0; nb.saturating_sub(1)];
    if nb < 2 {
        return Ok(counts);
    }
    // `diddle <- fuzz * if (nB > 5) median(h) else if (nB <= 3)
    // diff(range(x)) else min(h[h > 0])`.
    let h: Vec<f64> = breaks.windows(2).map(|w| w[1] - w[0]).collect();
    let typical = if nb > 5 {
        let mut sorted = h.clone();
        sorted.sort_by(f64::total_cmp);
        let m = sorted.len();
        if m % 2 == 1 {
            sorted[m / 2]
        } else {
            0.5 * (sorted[m / 2 - 1] + sorted[m / 2])
        }
    } else if nb <= 3 {
        spread(data)
    } else {
        h.iter()
            .copied()
            .filter(|&w| w > 0.0)
            .fold(f64::INFINITY, f64::min)
    };
    let diddle = fuzz * typical;
    let fuzzy: Vec<f64> = (0..nb)
        .map(|i| {
            let d = if right {
                if i == 0 && include_lowest {
                    -diddle
                } else {
                    diddle
                }
            } else if i == nb - 1 && include_lowest {
                diddle
            } else {
                -diddle
            };
            breaks[i] + d
        })
        .collect();
    // `C_bincount()`.
    let last = nb - 1;
    let mut counted = 0;
    for &x in data {
        if fuzzy[0] <= x && (x < fuzzy[last] || (x == fuzzy[last] && include_lowest)) {
            let (mut lo, mut hi) = (0, last);
            while hi - lo >= 2 {
                let mid = (hi + lo) / 2;
                if x > fuzzy[mid] || (!right && x == fuzzy[mid]) {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            counts[lo] += 1.0;
            counted += 1;
        }
    }
    if counted != data.len() {
        return Err(Error::invalid(
            "some 'x' not counted; maybe 'breaks' do not span range of 'x'",
        ));
    }
    Ok(counts)
}

/// What a histogram writes above each bar: nothing, each bar's count (or
/// density), or given labels.
///
/// Setters take `true`/`false` ([`Values`](HistLabels::Values) / none) or
/// an array or `Vec` of labels.
///
/// This is R's `hist(labels = )`.
///
/// # Examples
///
/// ```
/// use rs_graphics::prelude::*;
///
/// # let mut dev = Device::null().open().unwrap();
/// let x = [1, 2, 2, 3, 3, 3];
/// dev.hist().x(&x).breaks([0, 1, 2, 3]).labels(true).draw();
/// dev.hist().x(&x).breaks([0, 1, 2, 3]).labels(["low", "mid", "high"]).draw();
/// ```
#[derive(Clone, Debug, Default, PartialEq)]
pub enum HistLabels {
    /// Nothing (R's `labels = FALSE`, the default).
    #[default]
    None,
    /// Each bar's count, or its density to 3 decimals (R's `labels = TRUE`).
    Values,
    /// These labels, one per bar (R's character `labels`).
    Text(Vec<Label>),
}

impl From<bool> for HistLabels {
    /// `true`: [`HistLabels::Values`]; `false`: none.
    fn from(b: bool) -> Self {
        if b {
            HistLabels::Values
        } else {
            HistLabels::None
        }
    }
}

impl<L: Into<Label>, const N: usize> From<[L; N]> for HistLabels {
    /// These labels.
    fn from(v: [L; N]) -> Self {
        HistLabels::Text(v.into_iter().map(Into::into).collect())
    }
}

impl<L: Into<Label>> From<Vec<L>> for HistLabels {
    /// These labels.
    fn from(v: Vec<L>) -> Self {
        HistLabels::Text(v.into_iter().map(Into::into).collect())
    }
}

crate::setters::options! {
    /// Options for [`Graphics::hist`](crate::graphics::Graphics::hist), covering the subset of `hist.default`'s
    /// and `plot.histogram`'s arguments this crate implements. Settings left
    /// unset are R's defaults, given on each setter.
    #[derive(Clone, Debug)]
    pub struct HistOptions => [HistCall] {
        /// How to choose the bins: edges (`.breaks([0, 2, 5, 10])`, sorted
        /// as R does), a suggested number of classes (`.breaks(20)`, giving
        /// the edges `pretty(range(x), n = classes, min.n = 1)`), a rule
        /// (`.breaks("FD")`, `"Scott"`, `"Sturges"`) or a function of the
        /// data ([`HistBreaks::function`]). `None`: `"Sturges"`, R's
        /// default. A number of classes that isn't finite or is less than 1
        /// (R: "invalid number of 'breaks'") or an unknown rule name is an
        /// error of the call.
        pub breaks: Option<HistBreaks> => option HistBreaks,
        /// Whether to plot counts (`true`) or densities (`false`:
        /// `count / (n * bin width)`). `None`: counts if the bins are equally
        /// wide, as R.
        pub freq: Option<bool> => option bool,
        /// x axis limits. `None`: the range of the breaks.
        pub xlim: Option<(f64, f64)> => pair f64,
        /// y axis limits. `None`: 0 to the tallest bar.
        pub ylim: Option<(f64, f64)> => pair f64,
        /// Bar fills, recycled over the bars. Empty: `"lightgray"` (R's
        /// default).
        pub col: Vec<ColorSpec> => values ColorSpec,
        /// Bar borders, recycled over the bars. Empty: [`par(fg)`], or with
        /// shading the bar's `col` (R's `border = NULL`).
        ///
        /// [`par(fg)`]: crate::par::Par::fg
        pub border: Vec<ColorSpec> => values ColorSpec,
        /// Line types of the bar borders, recycled over the bars. Empty:
        /// [`par(lty)`]; `None`: that bar has no border and no shading lines
        /// (its fill stays).
        ///
        /// [`par(lty)`]: crate::par::Par::lty
        pub lty: Vec<Option<LineType>> => na_values LineType,
        /// Shading lines per inch in the bars, recycled over the bars; the
        /// lines take the bar's `col` (see [`RectOptions::density`]). `None`
        /// or a negative value fills the bar instead, 0 leaves it empty.
        /// Empty: no shading.
        pub density: Vec<Option<f64>> => na_values f64,
        /// Angle of the shading lines in degrees, recycled as `density`.
        /// Empty: 45.
        pub angle: Vec<f64> => numbers f64,
        /// Labels above the bars: [`HistLabels::Values`] writes each bar's
        /// count (or, with densities, its density to 3 decimals), `Text` the
        /// given labels. Default none.
        pub labels: HistLabels => value HistLabels,
        /// Whether bins are closed on the right, `(a, b]`, rather than `[a,
        /// b)`. `None`: `true`.
        pub right: Option<bool> => option bool,
        /// Whether the first bin is also closed on the left (the last on the
        /// right when not `right`). `None`: `true`; always `true` for
        /// computed breaks, as R (which warns). R's `include.lowest`.
        pub include_lowest: Option<bool> => option bool,
        /// How far, as a fraction of a typical bin width, values on an edge
        /// may stray and still count as on it. `None`: `1e-7`.
        pub fuzz: Option<f64> => number f64,
        /// Main title, as for `plot()` (text or a plotmath expression).
        /// `None`: "Histogram of x".
        pub main: Option<Label> => option Label,
        /// x axis label. `None`: "x".
        pub xlab: Option<Label> => option Label,
        /// y axis label. `None`: "Frequency" or "Density".
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
        pub col_main: Option<ColorSpec> => option_value ColorSpec,
        /// Font of the main title (a [`FontFace`]). `None`:
        /// [`par(font.main)`]. R's `font.main`.
        ///
        /// [`par(font.main)`]: crate::par::Par::font_main
        pub font_main: Option<FontFace> => option FontFace,
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
        pub col_lab: Option<ColorSpec> => option_value ColorSpec,
        /// Font of the axis titles (`xlab` and `ylab`; a [`FontFace`]).
        /// `None`: [`par(font.lab)`]. R's `font.lab`.
        ///
        /// [`par(font.lab)`]: crate::par::Par::font_lab
        pub font_lab: Option<FontFace> => option FontFace,
        /// Whether to draw the titles. Default `true`, whatever [`par(ann)`]
        /// is, as R's `ann`.
        ///
        /// [`par(ann)`]: crate::par::Par::ann
        pub ann: bool => value bool,
        /// Whether to draw the axes. Default `true`.
        pub axes: bool => value bool,
        /// Makes the x axis logarithmic. Default `false`. R's `log = "x"`.
        pub xlog: bool => value bool,
        /// Makes the y axis logarithmic. Default `false`. As in R, the range
        /// still starts from 0, which R "corrects" to about 1e-307 (with a
        /// warning). R's `log = "y"`.
        pub ylog: bool => value bool,
        /// Draws the bars on the current plot, without a new plot, window,
        /// title or axes (so `main`, `xlab`, `ylab`, `axes`, `xlog`, `ylog`,
        /// `xlim`, `ylim`, `ann` and `panel_bg` are ignored). Default `false`.
        pub add: bool => value bool,
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
        /// Minimum gap between axis labels, in "m" widths (labels parallel
        /// to the axis) or "M" heights (perpendicular). `None`: 1 or 0.25.
        /// R's `gap.axis`.
        pub gap_axis: Option<f64> => number f64,
        /// Width of the axes' line and ticks (only the axes: the bars are
        /// unaffected). `None`: 1, `axis()`'s default.
        pub lwd: Option<f64> => number f64,
        /// Whether to draw: `false` only returns the bins. `None`: `true`.
        pub plot: Option<bool> => option bool,
        /// Options not in R ([`crate::extra`]).
        pub extra: crate::extra::PlotExtras => no_setter,
    }
}

impl Default for HistOptions {
    fn default() -> Self {
        HistOptions {
            deferred_error: None,
            extra: Default::default(),
            breaks: None,
            freq: None,
            xlim: None,
            ylim: None,
            col: Vec::new(),
            border: Vec::new(),
            lty: Vec::new(),
            density: Vec::new(),
            angle: Vec::new(),
            labels: HistLabels::None,
            right: None,
            include_lowest: None,
            fuzz: None,
            main: None,
            xlab: None,
            ylab: None,
            cex_main: None,
            col_main: None,
            font_main: None,
            cex_lab: None,
            col_lab: None,
            font_lab: None,
            ann: true,
            axes: true,
            xlog: false,
            ylog: false,
            add: false,
            las: None,
            cex_axis: None,
            col_axis: None,
            font_axis: None,
            mgp: None,
            tck: None,
            tcl: None,
            gap_axis: None,
            lwd: None,
            plot: None,
        }
    }
}

/// The bins of a histogram: edges, counts, densities, mid-points, and
/// whether the bins are equally wide.
///
/// Returned by [`Graphics::hist`](crate::graphics::Graphics::hist)'s
/// `.draw()` (also with `.plot(false)`, which draws nothing).
///
/// This is the `"histogram"` object R's `hist()` returns.
///
/// # Examples
///
/// ```
/// use rs_graphics::prelude::*;
///
/// # let mut dev = Device::null().open().unwrap();
/// let h = dev.hist().x(&[1, 2, 2, 3, 3, 3]).breaks([0, 1, 2, 3]).plot(false).draw();
/// assert_eq!(h.mids, [0.5, 1.5, 2.5]);
/// assert_eq!(h.density, [1.0 / 6.0, 2.0 / 6.0, 3.0 / 6.0]);
/// ```
#[derive(Clone, Debug)]
pub struct HistResult {
    /// The bin edges, one more than the bins.
    pub breaks: Vec<f64>,
    /// The number of finite values in each bin.
    pub counts: Vec<f64>,
    /// Each bin's density: its count divided by the number of finite
    /// values and the bin's width (0 for an empty input or zero width).
    pub density: Vec<f64>,
    /// The mid-point of each bin.
    pub mids: Vec<f64>,
    /// R's `equidist`: the bins came from a number of classes or rule, or
    /// the given edges are equally spaced (to `1e-7` of the mean width);
    /// `freq`'s default.
    pub equidist: bool,
}

/// The bins of `hist(x, plot = FALSE)`: breaks, counts, densities and
/// mid-points. Needs no device, so a recorded figure can return the same
/// result as [`Graphics::hist`](crate::graphics::Graphics::hist).
pub(crate) fn hist_bins(x: &[f64], opts: &HistOptions) -> Result<HistResult, Error> {
    let data: Vec<f64> = x.iter().copied().filter(|v| v.is_finite()).collect();
    let n = data.len();

    // `hist.default`: edges (from the user or a function) are sorted and
    // used as they are; otherwise a number of classes (given, or from a
    // rule) becomes `pretty(range(x), n = classes, min.n = 1)`.
    let given = match &opts.breaks {
        None | Some(HistBreaks::Sturges) => HistBreaks::Count(nclass_sturges(n)),
        Some(HistBreaks::Scott) => HistBreaks::Count(nclass_scott(&data)),
        Some(HistBreaks::FreedmanDiaconis) => HistBreaks::Count(nclass_fd(&data)),
        Some(HistBreaks::Function(f)) => values_to_breaks(f(&data)),
        Some(b) => b.clone(),
    };
    let (breaks, use_br) = match given {
        HistBreaks::Edges(mut edges) if edges.len() > 1 => {
            edges.sort_by(f64::total_cmp);
            (edges, true)
        }
        HistBreaks::Count(k) => {
            if !(k.is_finite() && k >= 1.0) {
                return Err(Error::invalid("invalid number of 'breaks'"));
            }
            // R caps it (with a warning) so that pretty() gets an integer;
            // pretty() truncates it.
            let nclass = k.min(1e6) as i32;
            let (lo, hi) = data_range(&data);
            (pretty_r(lo, hi, nclass, 1), false)
        }
        // Fewer than two edges (none given): Sturges.
        _ => {
            let (lo, hi) = data_range(&data);
            (pretty_r(lo, hi, nclass_sturges(n) as i32, 1), false)
        }
    };

    // R forces `include.lowest` (with a warning) unless edges were given.
    let include_lowest = !use_br || opts.include_lowest.unwrap_or(true);
    let counts = bin_counts(
        &data,
        &breaks,
        opts.right.unwrap_or(true),
        include_lowest,
        opts.fuzz.unwrap_or(1e-7),
    )?;
    let widths: Vec<f64> = breaks.windows(2).map(|w| w[1] - w[0]).collect();
    let density: Vec<f64> = counts
        .iter()
        .zip(&widths)
        .map(|(&c, &w)| {
            if n > 0 && w > 0.0 {
                c / (n as f64 * w)
            } else {
                0.0
            }
        })
        .collect();
    let mids: Vec<f64> = breaks.windows(2).map(|w| 0.5 * (w[0] + w[1])).collect();
    // `equidist <- !use.br || diff(range(h)) < 1e-7 * mean(h)`.
    let (lo, hi) = widths
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), &w| {
            (a.min(w), b.max(w))
        });
    let mean = widths.iter().sum::<f64>() / widths.len() as f64;
    let equidist = !use_br || hi - lo < 1e-7 * mean;

    Ok(HistResult {
        breaks,
        counts,
        density,
        mids,
        equidist,
    })
}

impl<B: Backend> Device<B> {
    /// Draws R's `hist()`; the public entry is
    /// [`Graphics::hist`](crate::graphics::Graphics::hist), whose docs
    /// describe it.
    pub(crate) fn hist_impl(&mut self, x: &[f64], opts: &HistOptions) -> Result<HistResult, Error> {
        if !opts.plot.unwrap_or(true) {
            return hist_bins(x, opts);
        }
        let HistResult {
            breaks,
            counts,
            density,
            mids,
            equidist,
        } = hist_bins(x, opts)?;
        let nb = breaks.len();

        // R's `freq = NULL`: counts if the bins are equally wide.
        let freq = opts.freq.unwrap_or(equidist);
        let heights: Vec<f64> = if freq {
            counts.clone()
        } else {
            density.clone()
        };
        let ymax = heights.iter().cloned().fold(0.0_f64, f64::max);

        // `plot.histogram()`: window, title, axes (unless `add`), then the
        // bars.
        if !opts.add {
            self.plot_new_impl()?;
            let window = PlotWindowOptions {
                xlog: opts.xlog,
                ylog: opts.ylog,
                ..PlotWindowOptions::default()
            };
            // `xlim = range(breaks)`, `ylim = range(y, 0)` unless given.
            let xlim = opts.xlim.unwrap_or((breaks[0], breaks[nb - 1]));
            let ylim = opts.ylim.unwrap_or((0.0, ymax));
            self.plot_window_impl(xlim, ylim, &window);
            crate::extra::panel_bg(self, &opts.extra)?;

            if opts.ann {
                let or = |l: &Option<Label>, default: &str| {
                    Some(l.clone().unwrap_or_else(|| Label::from(default)))
                };
                let ylab = if freq { "Frequency" } else { "Density" };
                self.title_impl(&TitleOptions {
                    main: or(&opts.main, "Histogram of x"),
                    sub: None,
                    xlab: or(&opts.xlab, "x"),
                    ylab: or(&opts.ylab, ylab),
                    cex_main: opts.cex_main,
                    col_main: opts.col_main,
                    font_main: opts.font_main,
                    cex_lab: opts.cex_lab,
                    col_lab: opts.col_lab,
                    font_lab: opts.font_lab,
                    mgp: opts.mgp,
                    ..TitleOptions::default()
                });
            }

            if opts.axes {
                // `axis(1, ...)`, `axis(2, at = yt, ...)`.
                let inline = InlineAxis {
                    las: opts.las,
                    cex_axis: opts.cex_axis,
                    col_axis: opts.col_axis,
                    font_axis: opts.font_axis,
                    mgp: opts.mgp,
                    tck: opts.tck,
                    tcl: opts.tcl,
                    gap_axis: opts.gap_axis,
                    lwd: opts.lwd,
                    ..InlineAxis::default()
                };
                let x_axis = crate::extra::rotated_axis(&opts.extra, AxisOptions::default());
                self.axis_impl(1, &inline.apply(x_axis))?;
                // `yt[yt %% 1 == 0]` for counts: whole numbers only.
                let yticks = if freq {
                    self.ax_ticks(2)
                        .into_iter()
                        .filter(|v| v % 1.0 == 0.0)
                        .collect()
                } else {
                    self.ax_ticks(2)
                };
                let y_axis = AxisOptions {
                    at: Some(yticks),
                    ..AxisOptions::default()
                };
                self.axis_impl(2, &inline.apply(y_axis))?;
            }
        }

        // `rect(breaks[-nB], 0, breaks[-1L], y, col = col, border = border,
        // angle = angle, density = density, lty = lty)`.
        let or = |v: &[ColorSpec], default: ColorSpec| {
            if v.is_empty() {
                vec![default]
            } else {
                v.to_vec()
            }
        };
        let bars = RectOptions {
            col: or(
                &opts.col,
                ColorSpec::Color(Color::parse("lightgray").unwrap()),
            ),
            // `border = NULL`: `rect()`'s default (`par(fg)`, or with
            // shading the bar's `col`).
            border: opts.border.clone(),
            lty: opts.lty.clone(),
            density: opts.density.clone(),
            angle: opts.angle.clone(),
            ..RectOptions::default()
        };
        self.rect_impl(&breaks[..nb - 1], &[0.0], &breaks[1..], &heights, &bars)?;
        // `text(x$mids, y, labels, adj = c(0.5, -0.5))`: counts, or
        // densities rounded to 3 decimals, or the given labels.
        let labels: Vec<Label> = match &opts.labels {
            HistLabels::None => Vec::new(),
            HistLabels::Values => heights
                .iter()
                .map(|&v| {
                    let v = if freq {
                        v
                    } else {
                        (v * 1000.0).round() / 1000.0
                    };
                    Label::from(format!("{v}"))
                })
                .collect(),
            HistLabels::Text(t) => t.clone(),
        };
        if !labels.is_empty() {
            let at = TextOptions {
                adj: Some(0.5),
                vadj: Some(-0.5),
                ..TextOptions::default()
            };
            let labels: Vec<Option<Label>> = labels.into_iter().map(Some).collect();
            self.text_impl(&mids, &heights, &labels, &at)?;
        }

        Ok(HistResult {
            breaks,
            counts,
            density,
            mids,
            equidist,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Values from R 4.6: `sprintf("%.17g", signif(v, 5))` and
    /// `quantile(x, p, type = 7)`, written out in full.
    #[test]
    #[allow(clippy::excessive_precision)]
    fn signif_and_quantile_match_r() {
        let cases = [
            (1.23455, 1.2345999999999999),
            (2.5e-7, 2.4999999999999999e-07),
            (123456789.0, 123460000.0),
            (-0.000123455, -0.00012344999999999999),
            (1.00005, 1.0001),
            (99999.5, 100000.0),
            (0.15, 0.14999999999999999),
            (12345.65, 12346.0),
            (7.65432198, 7.6543000000000001),
            // An exact half rounds to even (`nearbyint`).
            (12344.5, 12344.0),
        ];
        for (v, r) in cases {
            assert_eq!(signif(v, 5), r, "signif({v}, 5)");
        }
        assert_eq!(signif(2.5, 1), 2.0);
        let x = [0.1, 0.7, 1.3, 2.9, 3.3, 10.1, 11.0];
        let cases = [
            (0.25, 1.0),
            (0.75, 6.6999999999999993),
            (1.0 / 8.0, 0.54999999999999993),
            (7.0 / 8.0, 10.324999999999999),
            (1.0 / 3.0, 1.3),
        ];
        for (p, r) in cases {
            assert_eq!(quantile7(&x, p), r, "quantile({p})");
        }
        // Here `(1 - h) * lo + h * hi` and `lo + h * (hi - lo)` differ.
        let x = [0.092, 0.223, 1.022, 3.174, 5.912, 6.495, 8.812];
        assert_eq!(quantile7(&x, 0.3), 0.86219999999999997);
        // `signif()` makes these all 1, so FD falls back to Scott's width.
        let tiny: Vec<f64> = (1..=20).map(|i| 1.0 + i as f64 * 1e-7).collect();
        assert_eq!(nclass_fd(&tiny), 3.0);
        // R's `nclass.FD()` gives 13 here; without `signif()` it would be 12.
        let x = [
            1.4796339, 1.4452033, 1.341357, 1.4070209, 1.9657181, 0.4703397,
        ];
        assert_eq!(nclass_fd(&x), 13.0);
    }
}
