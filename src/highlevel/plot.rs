//! `plot(x, y, ...)`, ported from `plot.default` in
//! `r-source/src/library/graphics/R/plot.R`, restricted to numeric x/y
//! scatter/line plots (the vast majority of `plot.default` is dispatch
//! logic and argument munging for other input types, which is out of
//! scope here).

use crate::backend::Backend;
use crate::color::ColorSpec;
use crate::device::{AxisOptions, Device, LineStyle, TitleOptions};
use crate::lty::LineType;
use crate::par::AxisType;
use crate::plotmath::Label;

use super::data_range;

/// `plot(..., type = )`. Only the core types are implemented: `"p"`
/// (points, the default), `"l"` (lines), `"b"` (both: points, with the
/// lines between them stopping short of each point), `"c"` (those broken
/// lines alone), `"o"` (overplotted: the line drawn straight through,
/// points on top) and `"n"`
/// (set up the axes/window but plot nothing, so points/lines can be added
/// afterwards).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlotType {
    Points,
    Lines,
    Both,
    /// `"c"`: the lines of `"b"` without the points.
    BrokenLines,
    Overplotted,
    None,
}

impl PlotType {
    pub fn parse(s: &str) -> PlotType {
        match s {
            "l" => PlotType::Lines,
            "b" => PlotType::Both,
            "c" => PlotType::BrokenLines,
            "o" => PlotType::Overplotted,
            "n" => PlotType::None,
            _ => PlotType::Points,
        }
    }
}

/// Options for [`plot`], covering the subset of `plot.default`'s
/// arguments this crate implements.
#[derive(Clone, Debug)]
pub struct PlotOptions {
    pub type_: PlotType,
    pub pch: i32,
    pub col: ColorSpec,
    pub bg: ColorSpec,
    pub lwd: f64,
    pub lty: LineType,
    pub cex: f64,
    /// Defaults to the finite data range, like R's `plot.window(xlim =
    /// range(x))` (positive values only on a log axis).
    pub xlim: Option<(f64, f64)>,
    pub ylim: Option<(f64, f64)>,
    pub xlog: bool,
    pub ylog: bool,
    /// Titles, drawn by `title()`: text (`Some("Weight".into())`) or a
    /// plotmath expression (`Some(Label::math("x^2"))`).
    pub main: Option<Label>,
    pub sub: Option<Label>,
    /// R defaults `xlab`/`ylab` to the deparsed variable names passed as
    /// `x`/`y`; we have no equivalent, so these default to `None` (no
    /// label) and callers should pass explicit labels.
    pub xlab: Option<Label>,
    pub ylab: Option<Label>,
    pub axes: bool,
    /// Inline `xaxt`/`yaxt` (`plot(x, y, xaxt = "n")`): `AxisType::None`
    /// leaves out that axis; `None` uses `par(xaxt/yaxt)`.
    pub xaxt: Option<AxisType>,
    pub yaxt: Option<AxisType>,
    /// Inline `xaxp`/`yaxp` (`plot(x, y, xaxp = c(1, 10, 3))`): the default
    /// ticks of that axis; `None` uses `par(xaxp/yaxp)` as `plot_window()`
    /// sets it. Like R, `par()` itself is left unchanged.
    pub xaxp: Option<[f64; 3]>,
    pub yaxp: Option<[f64; 3]>,
}

impl Default for PlotOptions {
    fn default() -> Self {
        PlotOptions {
            type_: PlotType::Points,
            pch: 1,
            col: ColorSpec::Index(1),
            bg: ColorSpec::None,
            lwd: 1.0,
            lty: LineType::Pattern(&[]),
            cex: 1.0,
            xlim: None,
            ylim: None,
            xlog: false,
            ylog: false,
            main: None,
            sub: None,
            xlab: None,
            ylab: None,
            axes: true,
            xaxt: None,
            yaxt: None,
            xaxp: None,
            yaxp: None,
        }
    }
}

/// `plot(x, y, ...)`: `plot.new()` + `plot.window()` + the requested
/// point/line geometry + (optionally) `axis()`/`box()`/`title()`.
pub fn plot<B: Backend>(dev: &mut Device<B>, x: &[f64], y: &[f64], opts: &PlotOptions) {
    // On a log axis, values <= 0 are dropped (R's `xy.coords()` makes
    // them `NA`); `points()`/`lines()` then skip them.
    let range = |v: &[f64], log: bool| {
        if log {
            let positive: Vec<f64> = v.iter().copied().filter(|&a| a > 0.0).collect();
            data_range(&positive)
        } else {
            data_range(v)
        }
    };
    let xlim = opts.xlim.unwrap_or_else(|| range(x, opts.xlog));
    let ylim = opts.ylim.unwrap_or_else(|| range(y, opts.ylog));

    dev.plot_new();
    dev.plot_window(xlim, ylim, opts.xlog, opts.ylog);

    match opts.type_ {
        PlotType::Points => dev.points(x, y, opts.pch, opts.col, opts.bg, opts.cex),
        PlotType::Lines => dev.lines(x, y, LineStyle::new(opts.col, opts.lwd, opts.lty)),
        PlotType::Both => {
            dev.broken_lines(x, y, LineStyle::new(opts.col, opts.lwd, opts.lty));
            dev.points(x, y, opts.pch, opts.col, opts.bg, opts.cex);
        }
        PlotType::BrokenLines => {
            dev.broken_lines(x, y, LineStyle::new(opts.col, opts.lwd, opts.lty));
        }
        PlotType::Overplotted => {
            dev.lines(x, y, LineStyle::new(opts.col, opts.lwd, opts.lty));
            dev.points(x, y, opts.pch, opts.col, opts.bg, opts.cex);
        }
        PlotType::None => {}
    }

    if opts.axes {
        let fg = ColorSpec::Color(dev.par.fg);
        let (cex_axis, font_axis, las, lwd) = (
            dev.par.cex_axis,
            dev.par.font_axis,
            dev.par.las,
            dev.par.lwd,
        );
        let axis = |dev: &mut Device<B>, side, axt, axp| {
            let o = AxisOptions {
                col: Some(fg),
                col_axis: Some(fg),
                cex_axis: Some(cex_axis),
                font: Some(font_axis),
                las: Some(las),
                axt,
                axp,
                ..AxisOptions::default()
            };
            dev.axis_with(side, &o);
        };
        axis(dev, 1, opts.xaxt, opts.xaxp);
        axis(dev, 2, opts.yaxt, opts.yaxp);
        dev.box_plot(fg, lwd, LineType::Pattern(&[]));
    }

    if dev.par.ann {
        dev.title_with(&TitleOptions {
            main: opts.main.clone(),
            sub: opts.sub.clone(),
            xlab: opts.xlab.clone(),
            ylab: opts.ylab.clone(),
        });
    }
}
