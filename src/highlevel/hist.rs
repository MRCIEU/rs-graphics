// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! `hist(x, ...)`, ported from `hist.default`/`plot.histogram` in
//! `r-source/src/library/graphics/R/hist.R`, restricted to the default
//! `breaks = "Sturges"` rule or explicit bin edges (a number of classes, a
//! function, and the `"fd"`/`"scott"` rules aren't implemented) and no
//! `include.lowest`/`right`/`fuzz` customization (R's defaults,
//! `include.lowest = TRUE`, `right = TRUE`, are always used).

use crate::backend::Backend;
use crate::color::{Color, ColorSpec};
use crate::device::{AxisOptions, Device, PlotWindowOptions, RectOptions, TitleOptions};
use crate::plotmath::Label;
use crate::pretty::pretty_r;

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

/// Options for [`Device::hist`], covering the subset of `hist.default`'s
/// arguments this crate implements.
#[derive(Clone, Debug)]
pub struct HistOptions {
    /// If `Some`, used verbatim as the bin edges (must be increasing,
    /// length >= 2). If `None`, computed via Sturges' rule + `pretty()`,
    /// matching R's default `breaks = "Sturges"`.
    pub breaks: Option<Vec<f64>>,
    /// `freq = TRUE` (the default here, even for unequal `breaks`, where
    /// R defaults to `FALSE`): plot counts.
    /// `freq = FALSE`: plot density (`count / (n * bin width)`).
    pub freq: bool,
    pub col: ColorSpec,
    pub border: ColorSpec,
    /// Titles, as for `plot()` (text or a plotmath expression); `None`
    /// gives R's defaults ("Histogram of x", "x", "Frequency"/"Density").
    pub main: Option<Label>,
    pub xlab: Option<Label>,
    pub ylab: Option<Label>,
    pub axes: bool,
    /// `log = "x"` / `"y"`. As in R, a log y axis still takes its range
    /// from 0, which R "corrects" to about 1e-307 (with a warning).
    pub xlog: bool,
    pub ylog: bool,
    /// `add = TRUE`: draw the bars on the current plot, without a new
    /// plot, window, title or axes (so `main`/`xlab`/`ylab`/`axes`/`xlog`/
    /// `ylog` are ignored).
    pub add: bool,
}

crate::setters::setters!(HistOptions {
    breaks: option_values f64,
    freq: value bool,
    col: value ColorSpec,
    border: value ColorSpec,
    main: option Label,
    xlab: option Label,
    ylab: option Label,
    axes: value bool,
    xlog: value bool,
    ylog: value bool,
    add: value bool,
});

impl Default for HistOptions {
    fn default() -> Self {
        HistOptions {
            breaks: None,
            freq: true,
            col: ColorSpec::Color(Color::parse("lightgray").unwrap()),
            border: ColorSpec::Index(1),
            main: None,
            xlab: None,
            ylab: None,
            axes: true,
            xlog: false,
            ylog: false,
            add: false,
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
/// mid-points. Needs no device, so a recorded plot can return the same
/// result as [`Device::hist`].
pub(crate) fn hist_bins(x: &[f64], opts: &HistOptions) -> HistResult {
    let data: Vec<f64> = x.iter().copied().filter(|v| v.is_finite()).collect();
    let n = data.len();

    let breaks = opts.breaks.clone().unwrap_or_else(|| {
        let lo = data.iter().cloned().fold(f64::INFINITY, f64::min);
        let hi = data.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let (lo, hi) = if lo.is_finite() && hi.is_finite() && lo < hi {
            (lo, hi)
        } else {
            (0.0, 1.0)
        };
        let nclass = nclass_sturges(n).max(1);
        pretty_r(lo, hi, nclass, 1)
    });

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
    /// `hist(x, ...)`. A high-level plot: it starts a new plot with [`Device::plot_new`], so on a multi-panel
    /// page it moves to the next panel,
    /// unless `opts.add` (R's `add = TRUE`), which draws the bars on the
    /// current plot. Returns the bins and counts (R's `"histogram"`
    /// object).
    pub fn hist(&mut self, x: &[f64], opts: &HistOptions) -> HistResult {
        let HistResult {
            breaks,
            counts,
            density,
            mids,
        } = hist_bins(x, opts);
        let nb = breaks.len();

        let heights: Vec<f64> = if opts.freq {
            counts.clone()
        } else {
            density.clone()
        };
        let ymax = heights.iter().cloned().fold(0.0_f64, f64::max);

        // `plot.histogram()`: window, title, axes (unless `add`), then the
        // bars.
        if !opts.add {
            self.plot_new();
            let window = PlotWindowOptions {
                xlog: opts.xlog,
                ylog: opts.ylog,
                ..PlotWindowOptions::default()
            };
            self.plot_window((breaks[0], breaks[nb - 1]), (0.0, ymax), &window);

            if self.par.ann {
                let or = |l: &Option<Label>, default: &str| {
                    Some(l.clone().unwrap_or_else(|| Label::from(default)))
                };
                let ylab = if opts.freq { "Frequency" } else { "Density" };
                self.title(&TitleOptions {
                    main: or(&opts.main, "Histogram of x"),
                    sub: None,
                    xlab: or(&opts.xlab, "x"),
                    ylab: or(&opts.ylab, ylab),
                });
            }

            if opts.axes {
                self.axis(1, &AxisOptions::default());
                // `yt[yt %% 1 == 0]` for counts: whole numbers only.
                let yticks = if opts.freq {
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
                self.axis(2, &y_axis);
            }
        }

        // `rect(breaks[-nB], 0, breaks[-1L], y, col = col, border = border)`.
        let bars = RectOptions {
            col: vec![opts.col],
            border: vec![opts.border],
            ..RectOptions::default()
        };
        self.rect(&breaks[..nb - 1], &[0.0], &breaks[1..], &heights, &bars);

        HistResult {
            breaks,
            counts,
            density,
            mids,
        }
    }
}
