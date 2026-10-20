// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! `hist(x, ...)`, ported from `hist.default`/`plot.histogram` in
//! `r-source/src/library/graphics/R/hist.R`, restricted to the default
//! `breaks = "Sturges"` rule, a number of classes or explicit bin edges (a
//! function and the `"fd"`/`"scott"` rules aren't implemented) and no
//! `include.lowest`/`right`/`fuzz` customization (R's defaults,
//! `include.lowest = TRUE`, `right = TRUE`, are always used).

use crate::backend::Backend;
use crate::color::{Color, ColorSpec};
use crate::device::{AxisOptions, Device, PlotWindowOptions, RectOptions, TitleOptions};
use crate::plotmath::Label;
use crate::pretty::pretty_r;

use super::InlineAxis;

/// Sturges' rule (`nclass.Sturges()`): `ceiling(log2(length(x)) + 1)`.
fn nclass_sturges(n: usize) -> i32 {
    if n == 0 {
        return 1;
    }
    ((n as f64).log2() + 1.0).ceil() as i32
}

/// Bin `data` (assumed sorted or not, finite-only) into intervals
/// `(breaks[i], breaks[i+1]]`, except the first bin which also includes
/// its left edge (`include.lowest = TRUE`, `right = TRUE`, R's defaults).
/// Uses a small fixed epsilon (`1e-9` of the break span, at least `1e-9`)
/// rather than R's `fuzz`-based boundary adjustment (`1e-7` of the median
/// bin width).
fn bin_counts(data: &[f64], breaks: &[f64]) -> Vec<f64> {
    let nb = breaks.len();
    let mut counts = vec![0.0; nb.saturating_sub(1)];
    if nb < 2 {
        return counts;
    }
    let span = (breaks[nb - 1] - breaks[0]).abs().max(1.0);
    let eps = span * 1e-9;
    for &v in data {
        for i in 0..nb - 1 {
            let lo = breaks[i];
            let hi = breaks[i + 1];
            let in_bin = if i == 0 {
                v >= lo - eps && v <= hi + eps
            } else {
                v > lo + eps && v <= hi + eps
            };
            if in_bin {
                counts[i] += 1.0;
                break;
            }
        }
    }
    counts
}

/// Options for [`Graphics::hist`](crate::graphics::Graphics::hist), covering the subset of `hist.default`'s
/// and `plot.histogram`'s arguments this crate implements. Unset fields
/// are R's defaults, given for each.
#[derive(Clone, Debug)]
pub struct HistOptions {
    /// R's `breaks`: two or more values are the bin edges (sorted, as R
    /// does); one value is a suggested number of bins, as R's `breaks =
    /// 20` (edges from `pretty()`). `None` (or empty): Sturges' rule +
    /// `pretty()`, R's default `breaks = "Sturges"`.
    ///
    /// # Panics
    /// When drawn (or recorded), if the one value isn't finite or is less
    /// than 1 (R: "invalid number of 'breaks'").
    pub breaks: Option<Vec<f64>>,
    /// `true`: plot counts; `false`: plot densities (`count / (n * bin
    /// width)`). `None`: counts if the bins are equally wide, as R.
    pub freq: Option<bool>,
    /// Bar fill; `None`: `"lightgray"` (R's default).
    pub col: Option<ColorSpec>,
    /// Bar borders; `None`: `par(fg)` (R's default).
    pub border: Option<ColorSpec>,
    /// Main title, as for `plot()` (text or a plotmath expression);
    /// `None`: "Histogram of x".
    pub main: Option<Label>,
    /// x axis label; `None`: "x".
    pub xlab: Option<Label>,
    /// y axis label; `None`: "Frequency" or "Density".
    pub ylab: Option<Label>,
    /// Size of `main`, relative to `par(cex)` (R's `cex.main`, passed
    /// to `title()`); `None`: `par(cex.main)`.
    pub cex_main: Option<f64>,
    /// Colour of `main` (R's `col.main`, passed to `title()`);
    /// `None`: `par(col.main)`.
    pub col_main: Option<ColorSpec>,
    /// Font of `main` (1 plain, 2 bold, 3 italic, 4 bold italic; R's
    /// `font.main`, passed to `title()`); `None`: `par(font.main)`.
    pub font_main: Option<i32>,
    /// Size of `xlab` and `ylab`, relative to `par(cex)` (R's `cex.lab`, passed
    /// to `title()`); `None`: `par(cex.lab)`.
    pub cex_lab: Option<f64>,
    /// Colour of `xlab` and `ylab` (R's `col.lab`, passed to `title()`);
    /// `None`: `par(col.lab)`.
    pub col_lab: Option<ColorSpec>,
    /// Font of `xlab` and `ylab` (1 plain, 2 bold, 3 italic, 4 bold italic; R's
    /// `font.lab`, passed to `title()`); `None`: `par(font.lab)`.
    pub font_lab: Option<i32>,
    /// Draw the titles (R's `ann`, which defaults to `TRUE`, not
    /// `par(ann)`); default `true`.
    pub ann: bool,
    /// Draw the axes; default `true`.
    pub axes: bool,
    /// Log x axis (R's `log = "x"`); default `false`.
    pub xlog: bool,
    /// Log y axis (R's `log = "y"`); default `false`. As in R, it still
    /// takes its range from 0, which R "corrects" to about 1e-307 (with a
    /// warning).
    pub ylog: bool,
    /// `add = TRUE`: draw the bars on the current plot, without a new
    /// plot, window, title or axes (so `main`/`xlab`/`ylab`/`axes`/`xlog`/
    /// `ylog` are ignored). Default `false`.
    pub add: bool,
    /// Axis label orientation (R's inline `las`, passed to `axis()`): 0
    /// parallel to the axis, 1 horizontal, 2 perpendicular, 3 vertical;
    /// `None`: `par(las)`.
    pub las: Option<i32>,
    /// Axis label size (R's inline `cex.axis`); `None`: `par(cex.axis)`.
    pub cex_axis: Option<f64>,
    /// Axis label colour (R's inline `col.axis`); `None`: `par(col.axis)`.
    pub col_axis: Option<ColorSpec>,
    /// Axis label font (R's inline `font.axis`); `None`: `par(font.axis)`.
    pub font_axis: Option<i32>,
    /// Margin lines of the axis titles, labels and line (R's inline `mgp`,
    /// passed to `axis()` and `title()`); `None`: `par(mgp)`.
    pub mgp: Option<[f64; 3]>,
    /// Tick length as a fraction of the plot region (R's inline `tck`);
    /// `None`: `par(tck)`.
    pub tck: Option<f64>,
    /// Tick length in margin lines (R's inline `tcl`); `None`: `par(tcl)`.
    pub tcl: Option<f64>,
    /// Minimum gap between axis labels (R's inline `gap.axis`); `None`:
    /// R's default.
    pub gap_axis: Option<f64>,
}

crate::setters::setters!(HistOptions => [HistCall] {
    breaks: option_values f64,
    freq: option bool,
    col: option ColorSpec,
    border: option ColorSpec,
    main: option Label,
    xlab: option Label,
    ylab: option Label,
    cex_main: option f64,
    col_main: option ColorSpec,
    font_main: option i32,
    cex_lab: option f64,
    col_lab: option ColorSpec,
    font_lab: option i32,
    ann: value bool,
    axes: value bool,
    xlog: value bool,
    ylog: value bool,
    add: value bool,
    las: option i32,
    cex_axis: option f64,
    col_axis: option ColorSpec,
    font_axis: option i32,
    mgp: array [f64; 3],
    tck: option f64,
    tcl: option f64,
    gap_axis: option f64,
});

impl Default for HistOptions {
    fn default() -> Self {
        HistOptions {
            breaks: None,
            freq: None,
            col: None,
            border: None,
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
        }
    }
}

/// The `"histogram"` object R's `hist()` returns: bin edges, counts,
/// density, and bin midpoints.
#[derive(Clone, Debug)]
pub struct HistResult {
    pub breaks: Vec<f64>,
    pub counts: Vec<f64>,
    pub density: Vec<f64>,
    pub mids: Vec<f64>,
}

/// The bins of `hist(x, plot = FALSE)`: breaks, counts, densities and
/// mid-points. Needs no device, so a recorded figure can return the same
/// result as [`Graphics::hist`](crate::graphics::Graphics::hist).
pub(crate) fn hist_bins(x: &[f64], opts: &HistOptions) -> HistResult {
    let data: Vec<f64> = x.iter().copied().filter(|v| v.is_finite()).collect();
    let n = data.len();

    // `hist.default`: two or more values are the edges (sorted); one value
    // is a number of classes, and none is Sturges' rule; either way the
    // edges are `pretty(range(x), n = classes, min.n = 1)`.
    let breaks = match opts.breaks.as_deref() {
        Some(edges) if edges.len() > 1 => {
            let mut edges = edges.to_vec();
            edges.sort_by(f64::total_cmp);
            edges
        }
        classes => {
            let nclass = match classes {
                Some(&[k]) => {
                    assert!(k.is_finite() && k >= 1.0, "invalid number of 'breaks'");
                    // R caps it (with a warning) so that pretty() gets an
                    // integer; pretty() truncates it.
                    k.min(1e6) as i32
                }
                _ => nclass_sturges(n).max(1),
            };
            let lo = data.iter().cloned().fold(f64::INFINITY, f64::min);
            let hi = data.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            let (lo, hi) = if lo.is_finite() && hi.is_finite() && lo < hi {
                (lo, hi)
            } else {
                (0.0, 1.0)
            };
            pretty_r(lo, hi, nclass, 1)
        }
    };

    let counts = bin_counts(&data, &breaks);
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

    HistResult {
        breaks,
        counts,
        density,
        mids,
    }
}

impl<B: Backend> Device<B> {
    /// Draws R's `hist()`; the public entry is
    /// [`Graphics::hist`](crate::graphics::Graphics::hist), whose docs
    /// describe it.
    pub(crate) fn hist_impl(&mut self, x: &[f64], opts: &HistOptions) -> HistResult {
        let HistResult {
            breaks,
            counts,
            density,
            mids,
        } = hist_bins(x, opts);
        let nb = breaks.len();

        // R's `freq = NULL`: counts if the bins are equally wide
        // (`equidist`; always for computed breaks).
        let freq = opts.freq.unwrap_or_else(|| {
            let given = opts.breaks.as_ref().is_some_and(|b| b.len() > 1);
            let widths: Vec<f64> = breaks.windows(2).map(|w| w[1] - w[0]).collect();
            let (lo, hi) = widths
                .iter()
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), &w| {
                    (a.min(w), b.max(w))
                });
            let mean = widths.iter().sum::<f64>() / widths.len() as f64;
            !given || hi - lo < 1e-7 * mean
        });
        let heights: Vec<f64> = if freq {
            counts.clone()
        } else {
            density.clone()
        };
        let ymax = heights.iter().cloned().fold(0.0_f64, f64::max);

        // `plot.histogram()`: window, title, axes (unless `add`), then the
        // bars.
        if !opts.add {
            self.plot_new_impl();
            let window = PlotWindowOptions {
                xlog: opts.xlog,
                ylog: opts.ylog,
                ..PlotWindowOptions::default()
            };
            self.plot_window_impl((breaks[0], breaks[nb - 1]), (0.0, ymax), &window);

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
                };
                self.axis_impl(1, &inline.apply(AxisOptions::default()));
                // `yt[yt %% 1 == 0]` for counts: whole numbers only.
                let yticks = if freq {
                    self.default_ticks(2)
                        .into_iter()
                        .filter(|v| v % 1.0 == 0.0)
                        .collect()
                } else {
                    self.default_ticks(2)
                };
                let y_axis = AxisOptions {
                    at: Some(yticks),
                    ..AxisOptions::default()
                };
                self.axis_impl(2, &inline.apply(y_axis));
            }
        }

        // `rect(breaks[-nB], 0, breaks[-1L], y, col = col, border = border)`.
        let bars = RectOptions {
            col: vec![
                opts.col
                    .unwrap_or_else(|| ColorSpec::Color(Color::parse("lightgray").unwrap())),
            ],
            border: vec![opts.border.unwrap_or(ColorSpec::Color(self.par.fg))],
            ..RectOptions::default()
        };
        self.rect_impl(&breaks[..nb - 1], &[0.0], &breaks[1..], &heights, &bars);

        HistResult {
            breaks,
            counts,
            density,
            mids,
        }
    }
}
