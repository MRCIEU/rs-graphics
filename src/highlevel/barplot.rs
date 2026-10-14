//! `barplot(height, ...)`, ported from `barplot.default` in
//! `r-source/src/library/graphics/R/barplot.R`.
//!
//! Supported: a plain vector ([`barplot`], R's `vectorInput` case) and a
//! matrix ([`barplot_matrix`]) drawn either stacked or `beside`, vertical or
//! `horiz`ontal, with `width`/`space`, a per-series colour vector (default
//! `gray.colors(nrow)`), `names.arg`, `legend.text`, `xlim`/`ylim`.
//!
//! Not supported: log axes, hatched fills (`density`/`angle`), `offset`,
//! `orderH`, `add`, `args.legend`.

use crate::backend::Backend;
use crate::color::{Color, ColorSpec};
use crate::device::{Device, LegendEntry, LegendOptions, LegendPos};
use crate::lty::LineType;
use crate::par::AxisStyle;

/// Options for [`barplot`] / [`barplot_matrix`], covering the subset of
/// `barplot.default`'s arguments this crate implements.
#[derive(Clone, Debug)]
pub struct BarplotOptions<'a> {
    /// Labels drawn under each bar (R's `names.arg`; R takes them from
    /// `names(height)`/`colnames(height)`), only shown if `Some`. For a
    /// `beside` matrix either one name per bar or one per column (centred
    /// under each group of bars).
    pub names: Option<Vec<String>>,
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
    pub legend_text: Option<Vec<String>>,
    pub xlim: Option<(f64, f64)>,
    pub ylim: Option<(f64, f64)>,
    pub main: Option<&'a str>,
    pub sub: Option<&'a str>,
    pub xlab: Option<&'a str>,
    pub ylab: Option<&'a str>,
    pub axes: bool,
}

impl<'a> Default for BarplotOptions<'a> {
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
            main: None,
            sub: None,
            xlab: None,
            ylab: None,
            axes: true,
        }
    }
}

/// R's `gray.colors(n)` (`start = 0.3`, `end = 0.9`, `gamma = 2.2`).
pub fn gray_colors(n: usize) -> Vec<ColorSpec> {
    let (gamma, a, b) = (2.2_f64, 0.3_f64.powf(2.2), 0.9_f64.powf(2.2));
    (0..n)
        .map(|i| {
            let t = if n > 1 {
                i as f64 / (n - 1) as f64
            } else {
                0.0
            };
            let level = (a + (b - a) * t).powf(1.0 / gamma);
            let v = (255.0 * level + 0.5) as u8;
            ColorSpec::Color(Color::rgb(v, v, v))
        })
        .collect()
}

/// `barplot(height, ...)` for a plain vector. Returns the bar-centre
/// coordinates along the category axis (R's `w.m`, the value `barplot()`
/// returns invisibly), useful for overlaying annotations.
pub fn barplot<B: Backend>(
    dev: &mut Device<B>,
    heights: &[f64],
    opts: &BarplotOptions,
) -> Vec<f64> {
    assert!(!heights.is_empty(), "barplot: 'height' must be non-empty");
    barplot_impl(dev, &[heights.to_vec()], true, opts)
}

/// `barplot(height, ...)` for a matrix, given as its *columns* (each inner
/// `Vec` is one column, i.e. one group/stack; the entries are the rows, i.e.
/// the series). All columns must have the same length.
///
/// Stacked unless `opts.beside`. Returns `w.m`, flattened column-major (for
/// `beside`: one centre per bar, `nrow` per group; otherwise one per column).
pub fn barplot_matrix<B: Backend>(
    dev: &mut Device<B>,
    columns: &[Vec<f64>],
    opts: &BarplotOptions,
) -> Vec<f64> {
    assert!(!columns.is_empty(), "barplot: 'height' must be non-empty");
    let nr = columns[0].len();
    assert!(
        nr > 0 && columns.iter().all(|c| c.len() == nr),
        "barplot: matrix columns must be non-empty and of equal length"
    );
    barplot_impl(dev, columns, false, opts)
}

fn recycle(v: &[f64], n: usize) -> Vec<f64> {
    (0..n).map(|k| v[k % v.len()]).collect()
}

fn barplot_impl<B: Backend>(
    dev: &mut Device<B>,
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
            .flat_map(|_| std::iter::once(space[1]).chain(std::iter::repeat_n(space[0], nr - 1)))
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

    // --- bars: (position index, base, top, colour index) ---
    let mut bars: Vec<(usize, f64, f64, usize)> = Vec::new();
    let mut extent_lo = f64::INFINITY;
    let mut extent_hi = f64::NEG_INFINITY;
    let mut note = |h: f64| {
        // `range(rAdj, height)` with `rAdj = -0.01 * height`.
        extent_lo = extent_lo.min(-0.01 * h).min(h);
        extent_hi = extent_hi.max(-0.01 * h).max(h);
    };
    if beside {
        for (j, col) in columns.iter().enumerate() {
            for (i, &h) in col.iter().enumerate() {
                let k = j * nr + i;
                bars.push((k, 0.0, h, k));
                note(h);
            }
        }
    } else {
        note(0.0);
        for (j, col) in columns.iter().enumerate() {
            let mut acc = 0.0;
            for (i, &h) in col.iter().enumerate() {
                let base = acc;
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
    let (saved_xaxs, saved_yaxs) = (dev.par.xaxs, dev.par.yaxs);
    if horiz {
        dev.par.xaxs = AxisStyle::Exact;
    } else {
        dev.par.yaxs = AxisStyle::Exact;
    }
    dev.plot_new();
    dev.plot_window(xlim, ylim, false, false);
    dev.par.xaxs = saved_xaxs;
    dev.par.yaxs = saved_yaxs;

    // R draws everything with `par(xpd = TRUE)`.
    let saved_xpd = dev.par.xpd;
    dev.par.xpd = true;

    let lwd = dev.par.lwd;
    for &(k, base, top, ci) in &bars {
        let fill = colors[ci % colors.len()];
        if horiz {
            dev.rect(
                base,
                w_l[k],
                top,
                w_r[k],
                fill,
                opts.border,
                lwd,
                LineType::Pattern(&[]),
            );
        } else {
            dev.rect(
                w_l[k],
                base,
                w_r[k],
                top,
                fill,
                opts.border,
                lwd,
                LineType::Pattern(&[]),
            );
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
        let (cex_axis, font_axis, las) = (dev.par.cex_axis, dev.par.font_axis, dev.par.las);
        let fg = ColorSpec::Color(dev.par.fg);
        // `axis(side, at = at, labels = names.arg, lty = 0)`: names only.
        dev.axis_labels(
            if horiz { 2 } else { 1 },
            Some(at),
            Some(names.clone()),
            fg,
            cex_axis,
            font_axis,
            las,
        );
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
        let (ux0, ux1, uy0, uy1) = dev.usr();
        let plt = dev.plot_region();
        let dx = 0.1 * (ux1 - ux0) / plt.width();
        let dy = 0.1 * (uy1 - uy0) / plt.height();
        let lopts = LegendOptions {
            xjust: Some(1.0),
            yjust: Some(1.0),
            ..LegendOptions::default()
        };
        dev.legend_with(LegendPos::Xy(ux1 - dx, uy1 - dy), &entries, &lopts);
    }

    if dev.par.ann {
        dev.title(opts.main, opts.sub, opts.xlab, opts.ylab);
    }

    if opts.axes {
        let fg = ColorSpec::Color(dev.par.fg);
        let (cex_axis, font_axis, las) = (dev.par.cex_axis, dev.par.font_axis, dev.par.las);
        dev.axis(
            if horiz { 1 } else { 2 },
            None,
            None,
            fg,
            cex_axis,
            font_axis,
            las,
        );
    }

    dev.par.xpd = saved_xpd;
    w_m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gray_colors_match_r() {
        // R: gray.colors(1) = "#4D4D4D", gray.colors(3) =
        // "#4D4D4D" "#AEAEAE" "#E6E6E6".
        let hex = |c: &ColorSpec| match c {
            ColorSpec::Color(c) => c.to_hex().to_uppercase(),
            _ => unreachable!(),
        };
        assert_eq!(hex(&gray_colors(1)[0]), "#4D4D4D");
        let g3: Vec<String> = gray_colors(3).iter().map(hex).collect();
        assert_eq!(g3, ["#4D4D4D", "#AEAEAE", "#E6E6E6"]);
    }
}
