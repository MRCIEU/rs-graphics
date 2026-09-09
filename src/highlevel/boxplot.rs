//! `boxplot(x, ...)`, ported from `boxplot.default`/`bxp`/`boxplot.stats`
//! in `r-source/src/library/graphics/R/boxplot.R`, restricted to a list
//! of numeric groups drawn as vertical boxes at `x = 1, 2, ..., n` (no
//! `horizontal`, `notch`, or `varwidth` support).

use crate::backend::Backend;
use crate::color::{Color, ColorSpec};
use crate::device::{Device, LineStyle};
use crate::lty::LineType;

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
}

/// `boxplot.stats(x, coef)`: five-number summary + whiskers extended to
/// the most extreme non-outlying data point, +/- `coef * IQR` beyond the
/// hinges defining an outlier.
pub fn boxplot_stats(x: &[f64], coef: f64) -> BoxStats {
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
    BoxStats {
        lower_whisker: lo,
        q1: five[1],
        median: five[2],
        q3: five[3],
        upper_whisker: hi,
        outliers,
    }
}

/// Options for [`boxplot`], covering the subset of `boxplot.default`'s/
/// `bxp`'s arguments this crate implements.
#[derive(Clone, Debug)]
pub struct BoxplotOptions<'a> {
    /// Category labels under each box (only drawn when there's more than
    /// one box, matching R's `show.names <- n > 1`).
    pub names: Option<Vec<String>>,
    pub col: ColorSpec,
    pub border: ColorSpec,
    /// The `range` argument (whisker/outlier IQR multiplier). R's default
    /// is `1.5`.
    pub range_coef: f64,
    pub main: Option<&'a str>,
    pub sub: Option<&'a str>,
    pub xlab: Option<&'a str>,
    pub ylab: Option<&'a str>,
    pub axes: bool,
}

impl<'a> Default for BoxplotOptions<'a> {
    fn default() -> Self {
        BoxplotOptions {
            names: None,
            col: ColorSpec::Color(Color::parse("lightgray").unwrap()),
            border: ColorSpec::Index(1),
            range_coef: 1.5,
            main: None,
            sub: None,
            xlab: None,
            ylab: None,
            axes: true,
        }
    }
}

/// `boxplot(x, ...)` where `x` is a slice of groups (R's `list`/formula
/// input reduced to its common case). A single group is `boxplot(vec![x])`.
pub fn boxplot<B: Backend>(
    dev: &mut Device<B>,
    groups: &[Vec<f64>],
    opts: &BoxplotOptions,
) -> Vec<BoxStats> {
    let n = groups.len();
    assert!(n > 0, "boxplot: need at least one group");
    let stats: Vec<BoxStats> = groups
        .iter()
        .map(|g| boxplot_stats(g, opts.range_coef))
        .collect();
    let at: Vec<f64> = (1..=n).map(|i| i as f64).collect();
    let xlim = (0.5, n as f64 + 0.5);

    let mut ylo = f64::INFINITY;
    let mut yhi = f64::NEG_INFINITY;
    for s in &stats {
        for &v in [s.lower_whisker, s.q1, s.median, s.q3, s.upper_whisker]
            .iter()
            .chain(s.outliers.iter())
        {
            if v.is_finite() {
                ylo = ylo.min(v);
                yhi = yhi.max(v);
            }
        }
    }
    let ylim = if ylo <= yhi { (ylo, yhi) } else { (0.0, 1.0) };

    // R's default `boxwex = 0.8 * quantile(diff(sort(at)), 0.10)`; since
    // `at = 1:n` the gaps are all `1`, so this always reduces to `0.8`
    // except for the single-box case, which uses `0.5 * boxwex` instead.
    let box_width = if n <= 1 { 0.4 } else { 0.8 };
    let half = box_width / 2.0;
    let cap_half = half * 0.5; // staplewex = 0.5

    dev.plot_new();
    dev.plot_window(xlim, ylim, false, false);

    let lwd = dev.par.lwd;
    let border = opts.border;
    let fill = opts.col;
    for (i, s) in stats.iter().enumerate() {
        let x = at[i];
        if s.lower_whisker.is_finite() && s.q1.is_finite() {
            dev.segments(
                &[x],
                &[s.lower_whisker],
                &[x],
                &[s.q1],
                LineStyle::new(border, lwd, LineType::from_code(2)), // whisklty = "dashed"
            );
            dev.segments(
                &[x - cap_half],
                &[s.lower_whisker],
                &[x + cap_half],
                &[s.lower_whisker],
                LineStyle::new(border, lwd, LineType::Pattern(&[])),
            );
        }
        if s.q3.is_finite() && s.upper_whisker.is_finite() {
            dev.segments(
                &[x],
                &[s.q3],
                &[x],
                &[s.upper_whisker],
                LineStyle::new(border, lwd, LineType::from_code(2)),
            );
            dev.segments(
                &[x - cap_half],
                &[s.upper_whisker],
                &[x + cap_half],
                &[s.upper_whisker],
                LineStyle::new(border, lwd, LineType::Pattern(&[])),
            );
        }
        if s.q1.is_finite() && s.q3.is_finite() {
            dev.polygon(
                &[x - half, x + half, x + half, x - half],
                &[s.q1, s.q1, s.q3, s.q3],
                fill,
                border,
                lwd,
                LineType::Pattern(&[]),
            );
        }
        if s.median.is_finite() {
            // medlwd = 3 * par(lwd)
            dev.segments(
                &[x - half],
                &[s.median],
                &[x + half],
                &[s.median],
                LineStyle::new(border, 3.0 * lwd, LineType::Pattern(&[])),
            );
        }
        if !s.outliers.is_empty() {
            let xs: Vec<f64> = s.outliers.iter().map(|_| x).collect();
            dev.points(&xs, &s.outliers, 1, border, ColorSpec::None, 1.0);
        }
    }

    if opts.axes {
        let fg = ColorSpec::Color(dev.par.fg);
        let (cex_axis, font_axis, las, lwd) = (
            dev.par.cex_axis,
            dev.par.font_axis,
            dev.par.las,
            dev.par.lwd,
        );
        if n > 1 {
            let labels = opts
                .names
                .clone()
                .unwrap_or_else(|| (1..=n).map(|i| i.to_string()).collect());
            dev.axis(
                1,
                Some(at.clone()),
                Some(labels),
                fg,
                cex_axis,
                font_axis,
                las,
            );
        }
        dev.axis(2, None, None, fg, cex_axis, font_axis, las);
        dev.box_plot(fg, lwd, LineType::Pattern(&[]));
    }

    if dev.par.ann {
        dev.title(opts.main, opts.sub, opts.xlab, opts.ylab);
    }

    stats
}
