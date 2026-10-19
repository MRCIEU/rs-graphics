// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! `barplot(height, ...)`, ported from `barplot.default` in
//! `r-source/src/library/graphics/R/barplot.R`.
//!
//! [`Device::barplot`]. Supported: a plain vector (R's `vectorInput`
//! case) and a matrix (see [`BarHeights`]) drawn either stacked or
//! `beside`, vertical or
//! `horiz`ontal, with `width`/`space`, a per-series colour vector (default
//! `gray.colors(nrow)`), `names.arg`, `legend.text`, `xlim`/`ylim`, and log
//! axes (`xlog`/`ylog`).
//!
//! Not supported: hatched fills (`density`/`angle`), `offset`, `orderH`,
//! `add`, `args.legend`.

use crate::backend::Backend;
use crate::color::{Color, ColorSpec, gray_colors};
use crate::device::{
    AxisOptions, Device, LegendEntry, LegendOptions, LegendPos, PlotWindowOptions, RectOptions,
    TitleOptions,
};
use crate::par::AxisStyle;
use crate::plotmath::Label;

/// Options for [`Device::barplot`], covering the subset of
/// `barplot.default`'s arguments this crate implements.
#[derive(Clone, Debug)]
pub struct BarplotOptions {
    /// Labels drawn under each bar, or beside it if `horiz` (R's `names.arg`; R takes them from
    /// `names(height)`/`colnames(height)`), only shown if `Some`. For a
    /// `beside` matrix either one name per bar or one per column (centred
    /// under each group of bars).
    pub names: Option<Vec<Label>>,
    /// Fill colours, recycled over the bars of a vector / the rows of a
    /// matrix. `None` = R's default: `"grey"` for a vector,
    /// `gray.colors(nrow)` for a matrix.
    pub col: Option<Vec<ColorSpec>>,
    pub border: ColorSpec,
    /// Bar widths (recycled), default `1`.
    pub width: Vec<f64>,
    /// Space before each bar as a fraction of the mean bar width: one value,
    /// or `[within group, between groups]` for a `beside` matrix. `None` =
    /// R's default (`0.2`, or `[0, 1]` for a `beside` matrix).
    pub space: Option<Vec<f64>>,
    /// Matrix only: bars side by side instead of stacked.
    pub beside: bool,
    /// Horizontal bars.
    pub horiz: bool,
    /// Matrix only: legend labels, one per row (R's `legend.text`).
    pub legend_text: Option<Vec<Label>>,
    pub xlim: Option<(f64, f64)>,
    pub ylim: Option<(f64, f64)>,
    /// `log = "x"` / `"y"`. On a log value axis bars start at the lower
    /// limit given (`ylim`, or `xlim` if `horiz`), else at 0.9 times the
    /// smallest height, as in R.
    pub xlog: bool,
    pub ylog: bool,
    /// Titles, as for `plot()` (text or a plotmath expression).
    pub main: Option<Label>,
    pub sub: Option<Label>,
    pub xlab: Option<Label>,
    pub ylab: Option<Label>,
    pub axes: bool,
}

crate::setters::setters!(BarplotOptions {
    names: list Label,
    col: option_values ColorSpec,
    border: value ColorSpec,
    width: values f64,
    space: option_values f64,
    beside: value bool,
    horiz: value bool,
    legend_text: list Label,
    xlim: pair f64,
    ylim: pair f64,
    xlog: value bool,
    ylog: value bool,
    main: option Label,
    sub: option Label,
    xlab: option Label,
    ylab: option Label,
    axes: value bool,
});

impl Default for BarplotOptions {
    fn default() -> Self {
        BarplotOptions {
            names: None,
            col: None,
            border: ColorSpec::Index(1),
            width: vec![1.0],
            space: None,
            beside: false,
            horiz: false,
            legend_text: None,
            xlim: None,
            ylim: None,
            xlog: false,
            ylog: false,
            main: None,
            sub: None,
            xlab: None,
            ylab: None,
            axes: true,
        }
    }
}

/// The `height` argument of [`Device::barplot`]: R's vector or matrix.
/// Built with `.into()` from a vector of numbers (slice, array, `Vec`) or
/// from a matrix given as its columns (`Vec<Vec<f64>>`, a slice or array of
/// `Vec<f64>`).
#[derive(Clone, Debug, PartialEq)]
pub enum BarHeights {
    /// R's `vectorInput` case: one bar per value.
    Vector(Vec<f64>),
    /// A matrix as its *columns*: each inner `Vec` is one column, i.e. one
    /// group or stack, and its entries are the rows, i.e. the series. All
    /// columns must have the same length.
    Matrix(Vec<Vec<f64>>),
}

impl From<Vec<f64>> for BarHeights {
    fn from(v: Vec<f64>) -> Self {
        BarHeights::Vector(v)
    }
}

impl From<&Vec<f64>> for BarHeights {
    fn from(v: &Vec<f64>) -> Self {
        BarHeights::Vector(v.clone())
    }
}

impl From<&[f64]> for BarHeights {
    fn from(v: &[f64]) -> Self {
        BarHeights::Vector(v.to_vec())
    }
}

impl<const N: usize> From<[f64; N]> for BarHeights {
    fn from(v: [f64; N]) -> Self {
        BarHeights::Vector(v.to_vec())
    }
}

impl<const N: usize> From<&[f64; N]> for BarHeights {
    fn from(v: &[f64; N]) -> Self {
        BarHeights::Vector(v.to_vec())
    }
}

impl From<Vec<Vec<f64>>> for BarHeights {
    fn from(m: Vec<Vec<f64>>) -> Self {
        BarHeights::Matrix(m)
    }
}

impl From<&Vec<Vec<f64>>> for BarHeights {
    fn from(m: &Vec<Vec<f64>>) -> Self {
        BarHeights::Matrix(m.clone())
    }
}

impl From<&[Vec<f64>]> for BarHeights {
    fn from(m: &[Vec<f64>]) -> Self {
        BarHeights::Matrix(m.to_vec())
    }
}

impl<const N: usize> From<[Vec<f64>; N]> for BarHeights {
    fn from(m: [Vec<f64>; N]) -> Self {
        BarHeights::Matrix(m.to_vec())
    }
}

impl<const N: usize> From<&[Vec<f64>; N]> for BarHeights {
    fn from(m: &[Vec<f64>; N]) -> Self {
        BarHeights::Matrix(m.to_vec())
    }
}

fn recycle(v: &[f64], n: usize) -> Vec<f64> {
    (0..n).map(|k| v[k % v.len()]).collect()
}

impl<B: Backend> Device<B> {
    /// `barplot(height, ...)`. A high-level plot: it starts a new plot with [`Device::plot_new`], so on a multi-panel
    /// page it moves to the next panel.
    /// `heights` is a
    /// vector (one bar per value) or a matrix given as its columns (see
    /// [`BarHeights`]), drawn stacked unless `opts.beside`.
    ///
    /// Returns the bar-centre coordinates along the category axis (R's
    /// `w.m`, the value `barplot()` returns invisibly), useful for
    /// overlaying annotations. For a matrix they are flattened column-major
    /// (for `beside`: one centre per bar, `nrow` per group; otherwise one
    /// per column).
    pub fn barplot(&mut self, heights: impl Into<BarHeights>, opts: &BarplotOptions) -> Vec<f64> {
        match heights.into() {
            BarHeights::Vector(v) => {
                assert!(!v.is_empty(), "barplot: 'height' must be non-empty");
                self.barplot_impl(&[v], true, opts)
            }
            BarHeights::Matrix(columns) => {
                assert!(!columns.is_empty(), "barplot: 'height' must be non-empty");
                let nr = columns[0].len();
                assert!(
                    nr > 0 && columns.iter().all(|c| c.len() == nr),
                    "barplot: matrix columns must be non-empty and of equal length"
                );
                self.barplot_impl(&columns, false, opts)
            }
        }
    }

    fn barplot_impl(
        &mut self,
        columns: &[Vec<f64>],
        vector_input: bool,
        opts: &BarplotOptions,
    ) -> Vec<f64> {
        let nc = columns.len();
        let nr = columns[0].len();
        let beside = vector_input || opts.beside;
        let horiz = opts.horiz;
        assert!(!opts.width.is_empty(), "barplot: 'width' must be non-empty");

        // --- geometry along the category axis (w.l / w.m / w.r) ---
        let mean_width = opts.width.iter().sum::<f64>() / opts.width.len() as f64;
        let space_in = opts.space.clone().unwrap_or_else(|| {
            if !vector_input && beside {
                vec![0.0, 1.0]
            } else {
                vec![0.2]
            }
        });
        assert!(!space_in.is_empty(), "barplot: 'space' must be non-empty");
        let space: Vec<f64> = space_in.iter().map(|s| s * mean_width).collect();

        let nbars = if beside { nr * nc } else { nc };
        let space_v: Vec<f64> = if beside && space.len() == 2 && !vector_input {
            (0..nc)
                .flat_map(|_| {
                    std::iter::once(space[1]).chain(std::iter::repeat_n(space[0], nr - 1))
                })
                .collect()
        } else {
            recycle(&space, nbars)
        };
        let base_n = if beside { nr } else { nc };
        let width_v: Vec<f64> = (0..nbars)
            .map(|k| opts.width[(k % base_n) % opts.width.len()])
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

        // --- log scale in the data direction: where bars start ---
        let log_dat = (opts.xlog && horiz) || (opts.ylog && !horiz);
        let rectbase = if log_dat {
            let min_h = columns
                .iter()
                .flatten()
                .copied()
                .filter(|h| !h.is_nan())
                .fold(f64::INFINITY, f64::min);
            assert!(
                min_h > 0.0,
                "barplot: log scale error: at least one 'height + offset' value <= 0"
            );
            for (log, lim, name) in [
                (opts.xlog, opts.xlim, "xlim"),
                (opts.ylog, opts.ylim, "ylim"),
            ] {
                if let (true, Some(l)) = (log, lim) {
                    assert!(
                        l.0.min(l.1) > 0.0,
                        "barplot: log scale error: '{name}' <= 0"
                    );
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
        let mut note = |h: f64| {
            // `range(rAdj, height)` with `rAdj = 0.9 * height` on a log value
            // axis, else `-0.01 * height`.
            let adj = if log_dat { 0.9 * h } else { -0.01 * h };
            extent_lo = extent_lo.min(adj).min(h);
            extent_hi = extent_hi.max(adj).max(h);
        };
        if beside {
            for (j, col) in columns.iter().enumerate() {
                for (i, &h) in col.iter().enumerate() {
                    let k = j * nr + i;
                    bars.push((k, rectbase, h, k));
                    note(h);
                }
            }
        } else {
            // `rbind(rectbase, apply(height, 2, cumsum))`: the first segment
            // starts at `rectbase`, the sums themselves start from 0.
            note(rectbase);
            for (j, col) in columns.iter().enumerate() {
                let mut acc = 0.0;
                for (i, &h) in col.iter().enumerate() {
                    let base = if i == 0 { rectbase } else { acc };
                    acc += h;
                    bars.push((j, base, acc, i));
                    note(acc);
                }
            }
        }

        let colors: Vec<ColorSpec> = match &opts.col {
            Some(c) if !c.is_empty() => c.clone(),
            _ if vector_input => vec![ColorSpec::Color(Color::parse("grey").unwrap())],
            _ => gray_colors(nr),
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

        // barplot() temporarily sets the *value* axis to `"i"` (exact fit, no
        // 4% padding), leaving the categorical axis at its regular setting.
        let (saved_xaxs, saved_yaxs) = (self.par.xaxs, self.par.yaxs);
        if horiz {
            self.par.xaxs = AxisStyle::Exact;
        } else {
            self.par.yaxs = AxisStyle::Exact;
        }
        self.plot_new();
        let window = PlotWindowOptions {
            xlog: opts.xlog,
            ylog: opts.ylog,
            ..PlotWindowOptions::default()
        };
        self.plot_window(xlim, ylim, &window);
        self.par.xaxs = saved_xaxs;
        self.par.yaxs = saved_yaxs;

        // R draws everything with `par(xpd = TRUE)`.
        let saved_xpd = self.par.xpd;
        self.par.xpd = true;

        for &(k, base, top, ci) in &bars {
            let style = RectOptions {
                col: vec![colors[ci % colors.len()]],
                border: vec![opts.border],
                ..RectOptions::default()
            };
            if horiz {
                self.rect(&[base], &[w_l[k]], &[top], &[w_r[k]], &style);
            } else {
                self.rect(&[w_l[k]], &[base], &[w_r[k]], &[top], &style);
            }
        }

        if let Some(names) = &opts.names {
            let at: Vec<f64> = if names.len() == w_m.len() {
                w_m.clone()
            } else if beside && names.len() == nc {
                // colMeans(w.m): the centre of each group of bars.
                (0..nc)
                    .map(|j| w_m[j * nr..(j + 1) * nr].iter().sum::<f64>() / nr as f64)
                    .collect()
            } else {
                panic!("barplot: incorrect number of names");
            };
            // `axis(side, at = at, labels = names.arg, lty = 0)`: names only.
            let names = AxisOptions {
                at: Some(at),
                labels: Some(names.iter().cloned().map(Some).collect()),
                tick: false,
                ..AxisOptions::default()
            };
            self.axis(if horiz { 2 } else { 1 }, &names);
        }

        if let Some(labels) = &opts.legend_text {
            let mut entries: Vec<LegendEntry> = labels
                .iter()
                .enumerate()
                .map(|(i, l)| LegendEntry::filled(l.clone(), colors[i % colors.len()]))
                .collect();
            if (horiz && beside) || (!horiz && !beside) {
                entries.reverse();
            }
            // 0.1 inch in from the top-right corner of the plot region. (On a
            // log axis R passes the log10 `par("usr")` values to `legend()` as
            // data, which puts the legend near the bottom; we don't copy that.)
            let (ux0, ux1, uy0, uy1) = self.usr();
            let plt = self.plot_region();
            let data = |v: f64, log: bool| if log { 10f64.powf(v) } else { v };
            let x = data(ux1 - 0.1 * (ux1 - ux0) / plt.width(), opts.xlog);
            let y = data(uy1 - 0.1 * (uy1 - uy0) / plt.height(), opts.ylog);
            let lopts = LegendOptions {
                xjust: Some(1.0),
                yjust: Some(1.0),
                ..LegendOptions::default()
            };
            self.legend(LegendPos::Xy(x, y), &entries, &lopts);
        }

        if self.par.ann {
            self.title(&TitleOptions {
                main: opts.main.clone(),
                sub: opts.sub.clone(),
                xlab: opts.xlab.clone(),
                ylab: opts.ylab.clone(),
            });
        }

        if opts.axes {
            self.axis(if horiz { 1 } else { 2 }, &AxisOptions::default());
        }

        self.par.xpd = saved_xpd;
        w_m
    }
}
