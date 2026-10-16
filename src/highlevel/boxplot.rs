//! `boxplot(x, ...)`, ported from `boxplot.default`/`bxp`/`boxplot.stats`
//! in `r-source/src/library/graphics/R/boxplot.R`, for a list of numeric
//! groups drawn at positions `1, 2, ..., n`, vertical or `horizontal`, with
//! `notch` and `varwidth`.

use crate::backend::{Backend, LineEnd};
use crate::color::{Color, ColorSpec};
use crate::device::{Device, LineStyle, TitleOptions};
use crate::lty::LineType;
use crate::plotmath::Label;

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
pub fn boxplot_stats(x: &[f64], coef: f64) -> BoxStats {
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

/// Options for [`boxplot`], covering the subset of `boxplot.default`'s/
/// `bxp`'s arguments this crate implements.
#[derive(Clone, Debug)]
pub struct BoxplotOptions {
    /// Category labels under each box (only drawn when there's more than
    /// one box, matching R's `show.names <- n > 1`).
    pub names: Option<Vec<String>>,
    pub col: ColorSpec,
    pub border: ColorSpec,
    /// The `range` argument (whisker/outlier IQR multiplier). R's default
    /// is `1.5`.
    pub range_coef: f64,
    /// Titles, as for `plot()` (text or a plotmath expression).
    pub main: Option<Label>,
    pub sub: Option<Label>,
    pub xlab: Option<Label>,
    pub ylab: Option<Label>,
    pub axes: bool,
    /// Notched boxes: the box narrows to half its width at the median,
    /// between [`BoxStats::conf`]'s limits (R's `notch.frac = 0.5`).
    pub notch: bool,
    /// Box widths proportional to the square root of the group size.
    pub varwidth: bool,
    /// Horizontal boxes: group names on side 2, values on side 1.
    pub horizontal: bool,
}

impl Default for BoxplotOptions {
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
            notch: false,
            varwidth: false,
            horizontal: false,
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
    if opts.notch {
        for s in &stats {
            for v in [s.conf.0, s.conf.1] {
                if v.is_finite() {
                    ylo = ylo.min(v);
                    yhi = yhi.max(v);
                }
            }
        }
    }
    let ylim = if ylo <= yhi { (ylo, yhi) } else { (0.0, 1.0) };

    // R's default `boxwex = 0.8 * quantile(diff(sort(at)), 0.10)`; since
    // `at = 1:n` the gaps are all `1`, so this is always `0.8`. Widths are
    // halved for a single box, or scaled by `sqrt(n / max(n))` for
    // `varwidth`.
    let boxwex = 0.8;
    let max_n = stats.iter().map(|s| s.n).max().unwrap_or(0) as f64;
    let widths: Vec<f64> = stats
        .iter()
        .map(|s| {
            if opts.varwidth {
                boxwex * (s.n as f64 / max_n).sqrt()
            } else if n == 1 {
                0.5 * boxwex
            } else {
                boxwex
            }
        })
        .collect();

    dev.plot_new();
    if opts.horizontal {
        dev.plot_window(ylim, xlim, false, false);
    } else {
        dev.plot_window(xlim, ylim, false, false);
    }

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
    let lwd = dev.par.lwd;
    let border = opts.border;
    let fill = opts.col;
    let notch_frac = if opts.notch { 0.5 } else { 1.0 };
    for (i, s) in stats.iter().enumerate() {
        let x = at[i];
        let half = widths[i] / 2.0;
        let five = [s.lower_whisker, s.q1, s.median, s.q3, s.upper_whisker];
        if five.iter().any(|v| v.is_nan()) {
            continue;
        }
        // In `bxp()`'s order: box fill, median, whiskers, staples, then the
        // box border on top, then outliers.
        let (box_pos, box_val): (Vec<f64>, Vec<f64>) = if opts.notch {
            let (c0, c1) = s.conf;
            let f = notch_frac;
            (
                [-1.0, 1.0, 1.0, f, 1.0, 1.0, -1.0, -1.0, -f, -1.0]
                    .iter()
                    .map(|w| x + half * w)
                    .collect(),
                vec![s.q1, s.q1, c0, s.median, c1, s.q3, s.q3, c1, s.median, c0],
            )
        } else {
            (
                vec![x - half, x + half, x + half, x - half],
                vec![s.q1, s.q1, s.q3, s.q3],
            )
        };
        let (bx, by) = xy(&box_pos, &box_val);
        dev.polygon(&bx, &by, fill, ColorSpec::None, lwd, LineType::Blank);
        // medlwd = 3 * par(lwd), with `lend = 1` (butt), "to avoid
        // overlap": round ends would reach past the box sides. With a
        // notch it spans only the notch.
        let wntch = notch_frac * half;
        let lend = std::mem::replace(&mut dev.par.lend, LineEnd::Butt);
        let (mx, my) = xy(&[x - wntch, x + wntch], &[s.median, s.median]);
        let median = LineStyle::new(border, 3.0 * lwd, LineType::Pattern(&[]));
        dev.segments(&mx[..1], &my[..1], &mx[1..], &my[1..], median);
        dev.par.lend = lend;
        // Whiskers run from their ends to the hinges (which matters for
        // where the dashes fall); whisklty = "dashed". Then the staples
        // (staplewex = 0.5).
        let ends = [(s.lower_whisker, s.q1), (s.upper_whisker, s.q3)];
        for &(w, h) in &ends {
            let (sx, sy) = xy(&[x, x], &[w, h]);
            let dashed = LineStyle::new(border, lwd, LineType::from_code(2));
            dev.segments(&sx[..1], &sy[..1], &sx[1..], &sy[1..], dashed);
        }
        let cap = half * 0.5;
        for &(w, _) in &ends {
            let (sx, sy) = xy(&[x - cap, x + cap], &[w, w]);
            let solid = LineStyle::new(border, lwd, LineType::Pattern(&[]));
            dev.segments(&sx[..1], &sy[..1], &sx[1..], &sy[1..], solid);
        }
        dev.polygon(&bx, &by, ColorSpec::None, border, lwd, LineType::Pattern(&[]));
        if !s.outliers.is_empty() {
            let pos: Vec<f64> = s.outliers.iter().map(|_| x).collect();
            let (ox, oy) = xy(&pos, &s.outliers);
            dev.points(&ox, &oy, 1, border, ColorSpec::None, 1.0);
        }
    }

    // `bxp()`: names on side 1 (2 if horizontal), values on side 2 (1),
    // then the title, then the frame.
    let (name_side, value_side) = if opts.horizontal { (2, 1) } else { (1, 2) };
    let fg = ColorSpec::Color(dev.par.fg);
    let (cex_axis, font_axis, las) = (dev.par.cex_axis, dev.par.font_axis, dev.par.las);
    if opts.axes {
        if n > 1 {
            let labels = opts
                .names
                .clone()
                .unwrap_or_else(|| (1..=n).map(|i| i.to_string()).collect());
            dev.axis(
                name_side,
                Some(at.clone()),
                Some(labels),
                fg,
                cex_axis,
                font_axis,
                las,
            );
        }
        dev.axis(value_side, None, None, fg, cex_axis, font_axis, las);
    }

    if dev.par.ann {
        dev.title_with(&TitleOptions {
            main: opts.main.clone(),
            sub: opts.sub.clone(),
            xlab: opts.xlab.clone(),
            ylab: opts.ylab.clone(),
        });
    }

    if opts.axes {
        dev.box_plot(fg, dev.par.lwd, LineType::Pattern(&[]));
    }

    stats
}
