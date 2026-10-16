//! `plot(x, y, ...)`, ported from `plot.default` in
//! `r-source/src/library/graphics/R/plot.R`, restricted to numeric x/y
//! scatter/line plots (the vast majority of `plot.default` is dispatch
//! logic and argument munging for other input types, which is out of
//! scope here).

use crate::backend::Backend;
use crate::color::ColorSpec;
use crate::device::{AxisOptions, BoxOptions, Device, PlotWindowOptions, TitleOptions, XyOptions};
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
    /// R's `type =` letters `"p"`, `"l"`, `"b"`, `"c"`, `"o"` and `"n"`.
    /// `None` for anything else, including R's `"h"`, `"s"` and `"S"`,
    /// which aren't implemented.
    pub fn parse(s: &str) -> Option<PlotType> {
        match s {
            "p" => Some(PlotType::Points),
            "l" => Some(PlotType::Lines),
            "b" => Some(PlotType::Both),
            "c" => Some(PlotType::BrokenLines),
            "o" => Some(PlotType::Overplotted),
            "n" => Some(PlotType::None),
            _ => None,
        }
    }
}

crate::error::from_str_via_parse!(PlotType, "plot type");

/// Options for [`plot`], covering the subset of `plot.default`'s
/// arguments this crate implements.
#[derive(Clone, Debug)]
pub struct PlotOptions {
    pub type_: PlotType,
    /// Symbols, colours, fills, sizes and widths as for
    /// [`Device::points`]: recycled per point, empty for the default
    /// (`par()`; no fill; size 1).
    pub pch: Vec<i32>,
    pub col: Vec<ColorSpec>,
    pub bg: Vec<ColorSpec>,
    pub lwd: Vec<f64>,
    /// `None`: `par(lty)`.
    pub lty: Option<LineType>,
    pub cex: Vec<f64>,
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
            pch: Vec::new(),
            col: Vec::new(),
            bg: Vec::new(),
            lwd: Vec::new(),
            lty: None,
            cex: Vec::new(),
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
    let window = PlotWindowOptions {
        xlog: opts.xlog,
        ylog: opts.ylog,
        ..PlotWindowOptions::default()
    };
    dev.plot_window(xlim, ylim, &window);

    // `plot.xy(xy, type, ...)`.
    let xy = XyOptions {
        type_: Some(opts.type_),
        pch: opts.pch.clone(),
        col: opts.col.clone(),
        bg: opts.bg.clone(),
        cex: opts.cex.clone(),
        lwd: opts.lwd.clone(),
        lty: opts.lty,
        xpd: None,
    };
    dev.points(x, y, &xy);

    if opts.axes {
        let fg = ColorSpec::Color(dev.par.fg);
        let (cex_axis, font_axis, las) = (dev.par.cex_axis, dev.par.font_axis, dev.par.las);
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
            dev.axis(side, &o);
        };
        axis(dev, 1, opts.xaxt, opts.xaxp);
        axis(dev, 2, opts.yaxt, opts.yaxp);
        dev.box_(&BoxOptions::default());
    }

    if dev.par.ann {
        dev.title(&TitleOptions {
            main: opts.main.clone(),
            sub: opts.sub.clone(),
            xlab: opts.xlab.clone(),
            ylab: opts.ylab.clone(),
        });
    }
}
