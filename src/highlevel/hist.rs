//! `hist(x, ...)`, ported from `hist.default`/`plot.histogram` in
//! `r-source/src/library/graphics/R/hist.R`, restricted to the default
//! `breaks = "Sturges"` rule (numeric/vector/function `breaks` and the
//! `"fd"`/`"scott"` rules aren't implemented) and no `include.lowest`/
//! `right`/`fuzz` customization (R's defaults, `include.lowest = TRUE`,
//! `right = TRUE`, are always used).

use crate::backend::Backend;
use crate::color::{Color, ColorSpec};
use crate::device::Device;
use crate::lty::LineType;
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
/// Uses a small fixed epsilon rather than R's `fuzz`-based boundary
/// adjustment (see the crate README for this simplification).
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

/// Options for [`hist`], covering the subset of `hist.default`'s
/// arguments this crate implements.
#[derive(Clone, Debug)]
pub struct HistOptions<'a> {
    /// If `Some`, used verbatim as the bin edges (must be increasing,
    /// length >= 2). If `None`, computed via Sturges' rule + `pretty()`,
    /// matching R's default `breaks = "Sturges"`.
    pub breaks: Option<Vec<f64>>,
    /// `freq = TRUE` (the default for equal-width bins): plot counts.
    /// `freq = FALSE`: plot density (`count / (n * bin width)`).
    pub freq: bool,
    pub col: ColorSpec,
    pub border: ColorSpec,
    pub main: Option<&'a str>,
    pub xlab: Option<&'a str>,
    pub ylab: Option<&'a str>,
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

impl<'a> Default for HistOptions<'a> {
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

/// `hist(x, ...)`.
pub fn hist<B: Backend>(dev: &mut Device<B>, x: &[f64], opts: &HistOptions) -> HistResult {
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
    let nb = breaks.len();

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

    let heights: Vec<f64> = if opts.freq {
        counts.clone()
    } else {
        density.clone()
    };
    let ymax = heights.iter().cloned().fold(0.0_f64, f64::max);

    // `plot.histogram()`: window, title, axes (unless `add`), then the
    // bars.
    if !opts.add {
        dev.plot_new();
        dev.plot_window((breaks[0], breaks[nb - 1]), (0.0, ymax), opts.xlog, opts.ylog);

        if dev.par.ann {
            let ylab = opts
                .ylab
                .unwrap_or(if opts.freq { "Frequency" } else { "Density" });
            dev.title(
                Some(opts.main.unwrap_or("Histogram of x")),
                None,
                Some(opts.xlab.unwrap_or("x")),
                Some(ylab),
            );
        }

        if opts.axes {
            let fg = ColorSpec::Color(dev.par.fg);
            let (cex_axis, font_axis, las) = (dev.par.cex_axis, dev.par.font_axis, dev.par.las);
            dev.axis(1, None, None, fg, cex_axis, font_axis, las);
            // `yt[yt %% 1 == 0]` for counts: whole numbers only.
            let yticks = if opts.freq {
                dev.default_ticks(2)
                    .into_iter()
                    .filter(|v| v % 1.0 == 0.0)
                    .collect()
            } else {
                dev.default_ticks(2)
            };
            dev.axis(2, Some(yticks), None, fg, cex_axis, font_axis, las);
        }
    }

    for i in 0..heights.len() {
        dev.rect(
            breaks[i],
            0.0,
            breaks[i + 1],
            heights[i],
            opts.col,
            opts.border,
            dev.par.lwd,
            LineType::Pattern(&[]),
        );
    }

    HistResult {
        breaks,
        counts,
        density,
        mids,
    }
}
