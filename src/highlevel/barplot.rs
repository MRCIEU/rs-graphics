//! `barplot(height, ...)`, ported from `barplot.default` in
//! `r-source/src/library/graphics/R/barplot.R`, restricted to a single
//! (non-stacked, non-`beside`-matrix) series of vertical bars - i.e. the
//! `height` is a plain vector, matching R's `vectorInput` case.

use crate::backend::Backend;
use crate::color::{Color, ColorSpec};
use crate::device::Device;
use crate::lty::LineType;
use crate::par::AxisStyle;

/// Options for [`barplot`], covering the subset of `barplot.default`'s
/// arguments this crate implements.
#[derive(Clone, Debug)]
pub struct BarplotOptions<'a> {
    /// Category labels drawn under each bar (R's `names.arg`), only shown
    /// if `Some` (R shows them whenever `names(height)` is set).
    pub names: Option<Vec<String>>,
    pub col: ColorSpec,
    pub border: ColorSpec,
    /// Gap between bars, as a fraction of bar width. R's default is `0.2`.
    pub space: f64,
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
            col: ColorSpec::Color(Color::parse("grey").unwrap()),
            border: ColorSpec::Index(1),
            space: 0.2,
            main: None,
            sub: None,
            xlab: None,
            ylab: None,
            axes: true,
        }
    }
}

/// `barplot(height, ...)`. Returns the bar-center x-coordinates (R's
/// `w.m`, i.e. the value `barplot()` itself returns invisibly), useful
/// for e.g. overlaying error bars or additional annotations.
pub fn barplot<B: Backend>(
    dev: &mut Device<B>,
    heights: &[f64],
    opts: &BarplotOptions,
) -> Vec<f64> {
    let n = heights.len();
    assert!(n > 0, "barplot: 'height' must be non-empty");
    let width = 1.0;
    let space = opts.space;

    let mut w_l = vec![0.0; n];
    let mut w_r = vec![0.0; n];
    let mut w_m = vec![0.0; n];
    let mut cum = 0.0;
    for i in 0..n {
        cum += space + width;
        w_r[i] = cum;
        w_m[i] = cum - width / 2.0;
        w_l[i] = w_m[i] - width / 2.0;
    }
    let xlim = (w_l[0], w_r[n - 1]);

    let mut ylo = 0.0_f64;
    let mut yhi = 0.0_f64;
    for &h in heights {
        let radj = -0.01 * h;
        ylo = ylo.min(radj).min(h);
        yhi = yhi.max(radj).max(h);
    }
    let ylim = (ylo, yhi);

    // barplot() temporarily sets `yaxs = "i"` (exact fit, no 4% padding)
    // for the value axis of vertical bars, leaving the categorical (x)
    // axis at its regular `xaxs` setting.
    let saved_yaxs = dev.par.yaxs;
    dev.par.yaxs = AxisStyle::Exact;
    dev.plot_new();
    dev.plot_window(xlim, ylim, false, false);
    dev.par.yaxs = saved_yaxs;

    let lwd = dev.par.lwd;
    for i in 0..n {
        dev.rect(
            w_l[i],
            0.0,
            w_r[i],
            heights[i],
            opts.col,
            opts.border,
            lwd,
            LineType::Pattern(&[]),
        );
    }

    if let Some(names) = &opts.names {
        let lh = dev.par.line_height_inches();
        let label_offset = dev.par.mgp[1] * lh;
        let plt = dev.plot_region();
        let y = plt.y0 - label_offset;
        let (cex_axis, font_axis, col) = (
            dev.par.cex_axis,
            dev.par.font_axis,
            ColorSpec::Color(dev.par.fg),
        );
        // Matches R's `axis(1, at = w.m, labels = names.arg, ...)`, which
        // always draws in the margin regardless of `par(xpd)`.
        let saved_xpd = dev.par.xpd;
        dev.par.xpd = true;
        for (i, label) in names.iter().enumerate() {
            let x_in = dev.user_to_inches(w_m[i], ylim.0).x;
            dev.text_at(
                crate::geometry::Point::new(x_in, y),
                label,
                0.5,
                1.0,
                0.0,
                col,
                cex_axis,
                font_axis,
            );
        }
        dev.par.xpd = saved_xpd;
    }

    if opts.axes {
        let fg = ColorSpec::Color(dev.par.fg);
        let (cex_axis, font_axis, las) = (dev.par.cex_axis, dev.par.font_axis, dev.par.las);
        dev.axis(2, None, None, fg, cex_axis, font_axis, las);
    }

    if dev.par.ann {
        dev.title(opts.main, opts.sub, opts.xlab, opts.ylab);
    }

    w_m
}
