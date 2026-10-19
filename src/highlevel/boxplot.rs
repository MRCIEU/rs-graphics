// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! `boxplot(x, ...)`, ported from `boxplot.default`/`bxp`/`boxplot.stats`
//! in `r-source/src/library/graphics/R/boxplot.R`, for a list of numeric
//! groups drawn at positions `1, 2, ..., n`, vertical or `horizontal`, with
//! `notch` and `varwidth`.

use crate::backend::{Backend, LineEnd};
use crate::color::{Color, ColorSpec};
use crate::device::{
    AxisOptions, BoxOptions, Device, PlotWindowOptions, PolygonOptions, SegmentsOptions,
    TitleOptions, XyOptions,
};
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

/// Options for [`Graphics::boxplot`](crate::graphics::Graphics::boxplot), covering the subset of `boxplot.default`'s/
/// `bxp`'s arguments this crate implements.
#[derive(Clone, Debug)]
pub struct BoxplotOptions {
    /// Group labels on the name axis (side 1, or 2 if `horizontal`);
    /// `None` gives `1, 2, ...`. Only drawn when there's more than one
    /// box, matching R's `show.names <- n > 1`.
    pub names: Option<Vec<Label>>,
    pub col: ColorSpec,
    pub border: ColorSpec,
    /// The `range` argument (whisker/outlier IQR multiplier). R's default
    /// is `1.5`.
    pub range: f64,
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

crate::setters::setters!(BoxplotOptions => [BoxplotCall] {
    names: list Label,
    col: value ColorSpec,
    border: value ColorSpec,
    range: value f64,
    main: option Label,
    sub: option Label,
    xlab: option Label,
    ylab: option Label,
    axes: value bool,
    notch: value bool,
    varwidth: value bool,
    horizontal: value bool,
});

impl Default for BoxplotOptions {
    fn default() -> Self {
        BoxplotOptions {
            names: None,
            col: ColorSpec::Color(Color::parse("lightgray").unwrap()),
            border: ColorSpec::Index(1),
            range: 1.5,
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

/// Each group's statistics, as [`Graphics::boxplot`](crate::graphics::Graphics::boxplot) returns them. Needs no
/// device, so a recorded plot can return the same result.
pub(crate) fn boxplot_all_stats(groups: &[Vec<f64>], opts: &BoxplotOptions) -> Vec<BoxStats> {
    assert!(!groups.is_empty(), "boxplot: need at least one group");
    groups
        .iter()
        .map(|g| boxplot_stats(g, opts.range))
        .collect()
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

        self.plot_new_impl();
        let window = PlotWindowOptions::default();
        if opts.horizontal {
            self.plot_window_impl(ylim, xlim, &window);
        } else {
            self.plot_window_impl(xlim, ylim, &window);
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
        let lwd = self.par.lwd;
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
            let box_fill = PolygonOptions {
                col: vec![fill],
                border: vec![ColorSpec::None],
                lty: vec![LineType::BLANK],
                lwd: Some(lwd),
                ..PolygonOptions::default()
            };
            self.polygon_impl(&bx, &by, &box_fill);
            // medlwd = 3 * par(lwd), with `lend = 1` (butt), "to avoid
            // overlap": round ends would reach past the box sides. With a
            // notch it spans only the notch.
            let wntch = notch_frac * half;
            let lend = std::mem::replace(&mut self.par.lend, LineEnd::Butt);
            let (mx, my) = xy(&[x - wntch, x + wntch], &[s.median, s.median]);
            let line = |lwd: f64, lty: LineType| SegmentsOptions {
                col: vec![border],
                lty: vec![lty],
                lwd: vec![lwd],
                ..SegmentsOptions::default()
            };
            let median = line(3.0 * lwd, LineType::SOLID);
            self.segments_impl(&mx[..1], &my[..1], &mx[1..], &my[1..], &median);
            self.par.lend = lend;
            // Whiskers run from their ends to the hinges (which matters for
            // where the dashes fall); whisklty = "dashed". Then the staples
            // (staplewex = 0.5).
            let ends = [(s.lower_whisker, s.q1), (s.upper_whisker, s.q3)];
            for &(w, h) in &ends {
                let (sx, sy) = xy(&[x, x], &[w, h]);
                let dashed = line(lwd, LineType::DASHED);
                self.segments_impl(&sx[..1], &sy[..1], &sx[1..], &sy[1..], &dashed);
            }
            let cap = half * 0.5;
            for &(w, _) in &ends {
                let (sx, sy) = xy(&[x - cap, x + cap], &[w, w]);
                let solid = line(lwd, LineType::SOLID);
                self.segments_impl(&sx[..1], &sy[..1], &sx[1..], &sy[1..], &solid);
            }
            let box_border = PolygonOptions {
                col: vec![ColorSpec::None],
                border: vec![border],
                lty: vec![LineType::SOLID],
                lwd: Some(lwd),
                ..PolygonOptions::default()
            };
            self.polygon_impl(&bx, &by, &box_border);
            if !s.outliers.is_empty() {
                let pos: Vec<f64> = s.outliers.iter().map(|_| x).collect();
                let (ox, oy) = xy(&pos, &s.outliers);
                let outliers = XyOptions {
                    pch: vec![1],
                    col: vec![border],
                    ..XyOptions::default()
                };
                self.points_impl(&ox, &oy, &outliers);
            }
        }

        // `bxp()`: names on side 1 (2 if horizontal), values on side 2 (1),
        // then the title, then the frame.
        let (name_side, value_side) = if opts.horizontal { (2, 1) } else { (1, 2) };
        if opts.axes {
            if n > 1 {
                let labels = opts
                    .names
                    .clone()
                    .unwrap_or_else(|| (1..=n).map(|i| Label::from(i.to_string())).collect());
                let names = AxisOptions {
                    at: Some(at.clone()),
                    labels: Some(labels.into_iter().map(Some).collect()),
                    ..AxisOptions::default()
                };
                self.axis_impl(name_side, &names);
            }
            self.axis_impl(value_side, &AxisOptions::default());
        }

        if self.par.ann {
            self.title_impl(&TitleOptions {
                main: opts.main.clone(),
                sub: opts.sub.clone(),
                xlab: opts.xlab.clone(),
                ylab: opts.ylab.clone(),
            });
        }

        if opts.axes {
            self.box_impl(&BoxOptions::default());
        }

        stats
    }
}
