// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! R's graphics functions as the [`Graphics`] trait, implemented by
//! [`Device`] (which draws) and [`Figure`](crate::recording::Figure) (which
//! records, to draw later).
//!
//! Each method starts a call for one R function, taking R's data
//! arguments; the call's chainable setters take R's other arguments, and
//! `.draw()` runs it:
//!
//! ```
//! # use rs_graphics::prelude::*;
//! let mut dev = Device::svg(5.0, 5.0);
//! dev.plot(&[1.0, 2.0, 3.0], &[2.0, 4.0, 3.0]).pch(19).main("Title").draw();
//! dev.abline().h(3.0).col("grey").lty(LineType::DASHED).draw();
//! ```
//!
//! The setters are those of the call's options struct ([`AblineOptions`]
//! for `abline()`, ...), and `.with(&opts)` starts from a reusable one:
//!
//! ```
//! # use rs_graphics::prelude::*;
//! # let mut dev = Device::svg(5.0, 5.0);
//! # dev.plot_new();
//! let grey = SegmentsOptions::default().col("grey").lty(LineType::DASHED);
//! dev.segments(&[0.1], &[0.2], &[0.9], &[0.2]).with(&grey).draw();
//! dev.segments(&[0.1], &[0.8], &[0.9], &[0.8]).with(&grey).lwd(2.0).draw();
//! ```
//!
//! Nothing is drawn until `.draw()`; the calls are `#[must_use]`, so a
//! forgotten `.draw()` is a compiler warning.
//!
//! ```compile_fail
//! #![deny(unused_must_use)]
//! # use rs_graphics::prelude::*;
//! let mut dev = Device::svg(5.0, 5.0);
//! dev.plot_new();
//! dev.abline().h(0.5); // no `.draw()`: nothing drawn, and a warning
//! ```

use crate::backend::Backend;
use crate::device::{
    AblineOptions, ArrowsOptions, AxisOptions, BoxOptions, Device, LegendEntry, LegendLayout,
    LegendOptions, LegendPos, MtextOptions, PlotWindowOptions, PolygonOptions, RectOptions,
    SegmentsOptions, Side, TextOptions, TitleOptions, XyOptions,
};
use crate::highlevel::{
    BarHeights, BarplotOptions, BoxStats, BoxplotOptions, HistOptions, HistResult, PieOptions,
    PlotOptions,
};
use crate::par::ParOptions;
use crate::plotmath::Label;

pub(crate) mod sealed {
    pub trait Sealed {}
}

impl<B: Backend> sealed::Sealed for Device<B> {}

/// R's graphics functions. See the [module docs](self) for how calls are
/// written.
///
/// As in R, the high-level functions ([`plot`](Graphics::plot),
/// [`hist`](Graphics::hist), [`barplot`](Graphics::barplot),
/// [`boxplot`](Graphics::boxplot), [`pie`](Graphics::pie)) start a new
/// plot with [`plot_new`](Graphics::plot_new), which moves to the next
/// panel of a multi-panel page (or to a new page). The others add to the
/// current plot.
///
/// Implemented by [`Device`], which draws, and by
/// [`Figure`](crate::recording::Figure), which records the calls to draw
/// later; the trait can't be implemented outside this crate. Write your
/// own plots as functions or extension-trait methods generic over
/// `G: Graphics` (see the crate docs), and they work on both.
pub trait Graphics: sealed::Sealed + Sized {
    /// What [`legend`](Graphics::legend)'s `.draw()` returns:
    /// `Option<LegendLayout>` on a [`Device`], `()` on a
    /// [`Figure`](crate::recording::Figure) (whose legend layouts come from
    /// [`Device::add_figure`]).
    type LegendResult;

    /// `plot.new()`: moves to the next figure and sets up its figure and
    /// plot regions.
    ///
    /// - **Coordinates:** like R, it leaves linear user coordinates of
    ///   0..1 on both axes (as `plot_window((0, 1), (0, 1))`:
    ///   -0.04..1.04 with the default `xaxs = yaxs = "r"`) and their
    ///   default ticks, so you can draw straight away. Call
    ///   [`plot_window`](Graphics::plot_window) next for other limits or
    ///   log axes.
    /// - **Which figure:** the next one in the order set by
    ///   [`Device::mfrow`] / [`Device::mfcol`] (or `par()`'s
    ///   [`mfrow`](ParCall::mfrow) / [`mfcol`](ParCall::mfcol)),
    ///   [`Device::layout`] or
    ///   [`Device::mfg`] (one figure per page by default). After the last
    ///   figure, it wraps to figure 1 on a new page. The first call always
    ///   starts page one. After [`Device::fig`], the next call starts a
    ///   new page unless `set_new(true)` comes first.
    /// - **`par(new = TRUE)`:** after [`Device::set_new`]`(true)` it stays
    ///   in the current figure and draws over it. The flag is not cleared
    ///   here; any drawing clears it, as in R.
    /// - **Regions:** figure and plot regions are computed now from
    ///   `par.mar`/`oma` (in lines of [`Device::line_height_inches`], so
    ///   `par.cex`, `mex` and the starting `ps` count too), `par.pty` and
    ///   `par.plt`. Changing these afterwards has no effect until the
    ///   next call, also on margin text (`title()`, `mtext()`,
    ///   `axis(outer = )`), which R would place with the new `mar`/`oma`.
    /// - **New page:** what happens depends on the backend. PDF starts
    ///   a new page in the file, SVG discards the previous page, and PNG
    ///   paints over it. The page is then filled with `par.bg`, unless
    ///   that is fully transparent, which it is by default on PDF (as R's
    ///   `pdf()`).
    fn plot_new(&mut self);

    // ---- high-level plots ----

    /// `plot(x, y, ...)`: `plot.new()` + `plot.window()` + the requested
    /// point/line geometry + (optionally) `axis()`/`box()`/`title()`;
    /// options in [`PlotOptions`]. A high-level plot: it starts a new plot
    /// with [`plot_new`](Graphics::plot_new), so on a multi-panel page it
    /// moves to the next panel.
    fn plot<'a>(&'a mut self, x: &'a [f64], y: &'a [f64]) -> PlotCall<'a, Self> {
        PlotCall {
            target: self,
            x,
            y,
            opts: PlotOptions::default(),
        }
    }

    /// `hist(x, ...)`; options in [`HistOptions`]. A high-level plot: it
    /// starts a new plot with [`plot_new`](Graphics::plot_new), so on a
    /// multi-panel page it moves to the next panel, unless `add` (R's
    /// `add = TRUE`), which draws the bars on the current plot. `.draw()`
    /// returns the bins and counts (R's `"histogram"` object).
    fn hist<'a>(&'a mut self, x: &'a [f64]) -> HistCall<'a, Self> {
        HistCall {
            target: self,
            x,
            opts: HistOptions::default(),
        }
    }

    /// `barplot(height, ...)`; options in [`BarplotOptions`]. A high-level
    /// plot: it starts a new plot with [`plot_new`](Graphics::plot_new), so
    /// on a multi-panel page it moves to the next panel. `heights` is a
    /// vector (one bar per value) or a matrix given as its columns (see
    /// [`BarHeights`]), drawn stacked unless `beside`.
    ///
    /// `.draw()` returns the bar-centre coordinates along the category
    /// axis (R's `w.m`, the value `barplot()` returns invisibly), useful
    /// for overlaying annotations. For a matrix they are flattened
    /// column-major (for `beside`: one centre per bar, `nrow` per group;
    /// otherwise one per column).
    fn barplot(&mut self, heights: impl Into<BarHeights>) -> BarplotCall<'_, Self> {
        BarplotCall {
            target: self,
            heights: heights.into(),
            opts: BarplotOptions::default(),
        }
    }

    /// `boxplot(x, ...)` where `x` is a slice of groups (R's
    /// `list`/formula input reduced to its common case; a single group is
    /// `boxplot(&[x])`); options in [`BoxplotOptions`]. A high-level plot:
    /// it starts a new plot with [`plot_new`](Graphics::plot_new), so on a
    /// multi-panel page it moves to the next panel, unless `add` (R's
    /// `add = TRUE`), which draws the boxes on the current plot. `.draw()`
    /// returns each group's statistics.
    ///
    /// # Panics
    /// If `groups` is empty, or `at` or `width` doesn't have one value per
    /// group (or a width isn't positive), as R errors.
    fn boxplot<'a>(&'a mut self, groups: &'a [Vec<f64>]) -> BoxplotCall<'a, Self> {
        BoxplotCall {
            target: self,
            groups,
            opts: BoxplotOptions::default(),
        }
    }

    /// `pie(x, ...)`; options in [`PieOptions`]. A high-level plot: it
    /// starts a new plot with [`plot_new`](Graphics::plot_new), so on a
    /// multi-panel page it moves to the next panel.
    fn pie<'a>(&'a mut self, x: &'a [f64]) -> PieCall<'a, Self> {
        PieCall {
            target: self,
            x,
            opts: PieOptions::default(),
        }
    }

    // ---- low-level functions ----

    /// `plot.window(xlim, ylim, log, asp)`: sets up the user coordinates
    /// of the current plot region; options in [`PlotWindowOptions`]. Call
    /// it after [`plot_new`](Graphics::plot_new) and before drawing.
    ///
    /// - `xlim`/`ylim` are in data units, also on a log axis, and may be
    ///   decreasing (a reversed axis).
    /// - `xlog`/`ylog` make that axis log10 (R's `log = "x"`/`"y"`/
    ///   `"xy"`). [`Device::usr`] is then in log10 units on that axis, as
    ///   R's `par("usr")` is.
    /// - `asp` (linear axes only, as in R): `C_plot_window` first widens
    ///   whichever range is needed so one x unit is `asp` y units long on
    ///   the page, centred on the given limits.
    /// - Each range then goes through R's `GScale()` (`graphics.c`):
    ///   - With `par(xaxs/yaxs) = "r"` (the default), 4% is added at each
    ///     end; with `"i"` the limits are used as given.
    ///   - A zero-width range is widened: to -1..1 around 0, otherwise by
    ///     40% of the value either side.
    ///   - Non-finite limits are replaced by huge finite ones. On a log
    ///     axis, a limit <= 0 gives a lower end of about 1e-307, where R
    ///     warns "nonfinite axis limits"; this function doesn't warn or
    ///     panic.
    /// - Sets `par.xaxp`/`par.yaxp` (the default tick range and count,
    ///   R's `GAxisPars()`) from the new range and `par.lab`. Later calls
    ///   to `axis()` without `at` read them. Changing `par.lab` afterwards
    ///   doesn't change them, as in R.
    fn plot_window(&mut self, xlim: (f64, f64), ylim: (f64, f64)) -> PlotWindowCall<'_, Self> {
        PlotWindowCall {
            target: self,
            xlim,
            ylim,
            opts: PlotWindowOptions::default(),
        }
    }

    /// `points(x, y, ...)`: R's `plot.xy(type = "p")` unless `type_` says
    /// otherwise; options in [`XyOptions`]. Points with a non-finite
    /// coordinate (including values <= 0 on a log axis) are skipped.
    fn points<'a>(&'a mut self, x: &'a [f64], y: &'a [f64]) -> PointsCall<'a, Self> {
        PointsCall {
            target: self,
            x,
            y,
            opts: XyOptions::default(),
        }
    }

    /// `lines(x, y, ...)`: R's `plot.xy(type = "l")` unless `type_` says
    /// otherwise; options in [`XyOptions`]. The polyline breaks at
    /// non-finite coordinates (R's `NA`, or values <= 0 on a log axis).
    fn lines<'a>(&'a mut self, x: &'a [f64], y: &'a [f64]) -> LinesCall<'a, Self> {
        LinesCall {
            target: self,
            x,
            y,
            opts: XyOptions::default(),
        }
    }

    /// `segments(x0, y0, x1, y1, ...)` in data units; options in
    /// [`SegmentsOptions`]. The four coordinate slices and the styles are
    /// recycled to the longest; a segment with a non-finite end after
    /// conversion (including values <= 0 on a log axis) is skipped, as
    /// `C_segments`.
    fn segments<'a>(
        &'a mut self,
        x0: &'a [f64],
        y0: &'a [f64],
        x1: &'a [f64],
        y1: &'a [f64],
    ) -> SegmentsCall<'a, Self> {
        SegmentsCall {
            target: self,
            x0,
            y0,
            x1,
            y1,
            opts: SegmentsOptions::default(),
        }
    }

    /// `arrows(x0, y0, x1, y1, ...)`, a port of `C_arrows` / `GArrow()`;
    /// options in [`ArrowsOptions`]: each segment with open heads - two
    /// lines of `length` inches at `angle` degrees to the shaft - at its
    /// start, end or both (`code`). Coordinates and styles are recycled
    /// to the longest. As in R, an arrow with a non-finite end after
    /// conversion (including values <= 0 on a log axis) is skipped, and
    /// the heads of a segment shorter than 1/1000 inch are not drawn (R
    /// also warns).
    fn arrows<'a>(
        &'a mut self,
        x0: &'a [f64],
        y0: &'a [f64],
        x1: &'a [f64],
        y1: &'a [f64],
    ) -> ArrowsCall<'a, Self> {
        ArrowsCall {
            target: self,
            x0,
            y0,
            x1,
            y1,
            opts: ArrowsOptions::default(),
        }
    }

    /// `rect(xleft, ybottom, xright, ytop, ...)` in data units; options in
    /// [`RectOptions`]. Coordinates and styles are recycled to the
    /// longest. Rectangles with a non-finite corner (e.g. at 0 on a log
    /// axis) are skipped, as in R.
    fn rect<'a>(
        &'a mut self,
        xleft: &'a [f64],
        ybottom: &'a [f64],
        xright: &'a [f64],
        ytop: &'a [f64],
    ) -> RectCall<'a, Self> {
        RectCall {
            target: self,
            x0: xleft,
            y0: ybottom,
            x1: xright,
            y1: ytop,
            opts: RectOptions::default(),
        }
    }

    /// `polygon(x, y, ...)`; options in [`PolygonOptions`]: one polygon
    /// per run of finite points (non-finite coordinates, R's `NA`,
    /// separate them), with `col`, `border` and `lty` recycled over the
    /// polygons, as `C_polygon`.
    fn polygon<'a>(&'a mut self, x: &'a [f64], y: &'a [f64]) -> PolygonCall<'a, Self> {
        PolygonCall {
            target: self,
            x,
            y,
            opts: PolygonOptions::default(),
        }
    }

    /// `text(x, y, labels, ...)` in data units; options in
    /// [`TextOptions`]. Each label (text, or a plotmath expression such as
    /// `Label::math("alpha^2")`) goes at its point, with `x`/`y` recycled
    /// to the longer and labels recycled over the points (extra labels
    /// are ignored; R truncates them with a warning). Empty `labels` are
    /// R's default, the indices `"1"`, `"2"`, .... Placement follows
    /// `GEText()`/`GEMathText()`: horizontally `adj` of the width;
    /// vertically see [`TextOptions::vadj`].
    fn text<'a, L: Clone + Into<Label>>(
        &'a mut self,
        x: &'a [f64],
        y: &'a [f64],
        labels: &[L],
    ) -> TextCall<'a, Self> {
        TextCall {
            target: self,
            x,
            y,
            labels: labels.iter().cloned().map(Into::into).collect(),
            opts: TextOptions::default(),
        }
    }

    /// `box(which, ...)`: outlines the plot region, figure, inner region
    /// or the whole device; options in [`BoxOptions`]. Not clipped.
    fn box_(&mut self) -> BoxCall<'_, Self> {
        BoxCall {
            target: self,
            opts: BoxOptions::default(),
        }
    }

    /// `axis(side, ...)` with all of R's arguments; options in
    /// [`AxisOptions`]. `side` is a [`Side`] or R's number for it (1 to
    /// 4).
    ///
    /// # Panics
    /// If `side` is a number other than 1 to 4 (here), or, at `.draw()`,
    /// if `labels` is given without `at` or their lengths differ (R's
    /// errors).
    fn axis(&mut self, side: impl Into<Side>) -> AxisCall<'_, Self> {
        AxisCall {
            target: self,
            side: side.into(),
            opts: AxisOptions::default(),
        }
    }

    /// `abline(a, b, h, v, untf, col, lty, lwd)`; options in
    /// [`AblineOptions`]. For R's `abline(coef = )` or `abline(reg = )`,
    /// set the intercept and slope as `coef`.
    ///
    /// # Panics
    /// At `.draw()`, if `coef` has a non-finite value (R: "'a' and 'b'
    /// must be finite").
    fn abline(&mut self) -> AblineCall<'_, Self> {
        AblineCall {
            target: self,
            opts: AblineOptions::default(),
        }
    }

    /// `legend(pos, legend, ...)`, a port of R's layout: one row per
    /// [`LegendEntry`], placed by `pos`; options in [`LegendOptions`]. On
    /// a [`Device`], `.draw()` returns where the box and labels went (R's
    /// return value, in `usr` units), or `None` if `entries` is empty (R
    /// errors there).
    ///
    /// `pos` is written as in R (see [`LegendPos`]): a keyword such as
    /// `"topright"`, a point `(x, y)`, or two corners
    /// `((xa, xb), (ya, yb))`:
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// # let mut dev = Device::svg(5.0, 5.0);
    /// # dev.plot(&[1.0, 10.0], &[1.0, 30.0]).draw();
    /// let keys = [LegendEntry::point("observed", 1, 19)];
    /// dev.legend("topright", &keys).draw(); // legend("topright", ...)
    /// dev.legend((5.0, 20.0), &keys).draw(); // legend(5, 20, ...)
    /// ```
    ///
    /// # Panics
    /// If `pos` is a string that isn't one of R's nine keywords (or a
    /// unique prefix of one).
    fn legend<'a>(
        &'a mut self,
        pos: impl Into<LegendPos>,
        entries: &'a [LegendEntry],
    ) -> LegendCall<'a, Self> {
        LegendCall {
            target: self,
            pos: pos.into(),
            entries,
            opts: LegendOptions::default(),
        }
    }

    /// `mtext(text, side, line, outer, at, adj, padj, cex, col, font)`,
    /// with rotation from `par(las)`; options in [`MtextOptions`]. `text`
    /// is text (placed by `GMtext()`) or a plotmath expression
    /// (`Label::math(...)`, placed by `GMMathText()`: its bounding box
    /// aligned by `adj`/`padj`, in the plain font). `side` is a [`Side`]
    /// or R's number for it (1 to 4); R's default is 3 (top).
    ///
    /// # Panics
    /// If `side` is a number other than 1 to 4.
    fn mtext(&mut self, text: impl Into<Label>, side: impl Into<Side>) -> MtextCall<'_, Self> {
        MtextCall {
            target: self,
            text: text.into(),
            side: side.into(),
            opts: MtextOptions::default(),
        }
    }

    /// `title(main=, sub=, xlab=, ylab=)`; options in [`TitleOptions`],
    /// each text or a plotmath expression, drawn in `C_title`'s order
    /// (main, sub, xlab, ylab) with `par(cex.main/col.main/font.main)`
    /// etc. Text is placed by `GMtext()` (baseline on the line, nudged by
    /// `yLineBias`); expressions by `GMathText()` (`main`: box centred on
    /// its line) and `GMMathText()` (the others: box bottom on the nudged
    /// line).
    fn title(&mut self) -> TitleCall<'_, Self> {
        TitleCall {
            target: self,
            opts: TitleOptions::default(),
        }
    }

    /// `par(...)`: changes graphical parameters; options in
    /// [`ParOptions`] (each a [`Par`](crate::par::Par) field, plus `new`
    /// for R's `par(new = TRUE)`). On a [`Device`] it is the same as
    /// assigning `dev.par.<field>` (and [`Device::set_new`] for `new`);
    /// in a recorded [`Figure`](crate::recording::Figure) it is the way to
    /// change them. As in R, `mar`, `oma`, `pty` and `plt` take effect at
    /// the next [`plot_new`](Graphics::plot_new).
    ///
    /// On a `Device`, the call also takes R's page layout parameters
    /// [`mfrow`](ParCall::mfrow) and [`mfcol`](ParCall::mfcol), as in R's
    /// `par(mfrow = c(1, 2), oma = c(0, 0, 2, 0))`:
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// let mut dev = Device::pdf(8.0, 4.0);
    /// dev.par().mfrow(1, 2).oma([0.0, 0.0, 2.0, 0.0]).draw();
    /// ```
    ///
    /// A `Figure` is one panel, so it can't change how the page is split;
    /// its `par()` call has no such setters:
    ///
    /// ```compile_fail
    /// # use rs_graphics::prelude::*;
    /// let mut fig = Figure::new();
    /// fig.par().mfrow(1, 2).draw(); // error: no method `mfrow`
    /// ```
    fn par(&mut self) -> ParCall<'_, Self> {
        ParCall {
            target: self,
            grid: None,
            opts: ParOptions::default(),
        }
    }

    // ---- what `.draw()` calls; not part of the public API ----

    #[doc(hidden)]
    fn emit_plot(&mut self, x: &[f64], y: &[f64], opts: PlotOptions);
    #[doc(hidden)]
    fn emit_hist(&mut self, x: &[f64], opts: HistOptions) -> HistResult;
    #[doc(hidden)]
    fn emit_barplot(&mut self, heights: BarHeights, opts: BarplotOptions) -> Vec<f64>;
    #[doc(hidden)]
    fn emit_boxplot(&mut self, groups: &[Vec<f64>], opts: BoxplotOptions) -> Vec<BoxStats>;
    #[doc(hidden)]
    fn emit_pie(&mut self, x: &[f64], opts: PieOptions);
    #[doc(hidden)]
    fn emit_plot_window(&mut self, xlim: (f64, f64), ylim: (f64, f64), opts: PlotWindowOptions);
    #[doc(hidden)]
    fn emit_points(&mut self, x: &[f64], y: &[f64], opts: XyOptions);
    #[doc(hidden)]
    fn emit_lines(&mut self, x: &[f64], y: &[f64], opts: XyOptions);
    #[doc(hidden)]
    fn emit_segments(
        &mut self,
        x0: &[f64],
        y0: &[f64],
        x1: &[f64],
        y1: &[f64],
        opts: SegmentsOptions,
    );
    #[doc(hidden)]
    fn emit_arrows(&mut self, x0: &[f64], y0: &[f64], x1: &[f64], y1: &[f64], opts: ArrowsOptions);
    #[doc(hidden)]
    fn emit_rect(&mut self, x0: &[f64], y0: &[f64], x1: &[f64], y1: &[f64], opts: RectOptions);
    #[doc(hidden)]
    fn emit_polygon(&mut self, x: &[f64], y: &[f64], opts: PolygonOptions);
    #[doc(hidden)]
    fn emit_text(&mut self, x: &[f64], y: &[f64], labels: Vec<Label>, opts: TextOptions);
    #[doc(hidden)]
    fn emit_box(&mut self, opts: BoxOptions);
    #[doc(hidden)]
    fn emit_axis(&mut self, side: Side, opts: AxisOptions);
    #[doc(hidden)]
    fn emit_abline(&mut self, opts: AblineOptions);
    #[doc(hidden)]
    fn emit_legend(
        &mut self,
        pos: LegendPos,
        entries: &[LegendEntry],
        opts: LegendOptions,
    ) -> Self::LegendResult;
    #[doc(hidden)]
    fn emit_mtext(&mut self, text: Label, side: Side, opts: MtextOptions);
    #[doc(hidden)]
    fn emit_title(&mut self, opts: TitleOptions);
    #[doc(hidden)]
    fn emit_par(&mut self, opts: ParOptions, grid: Option<ParGrid>);
}

impl<B: Backend> Graphics for Device<B> {
    type LegendResult = Option<LegendLayout>;

    fn plot_new(&mut self) {
        self.plot_new_impl()
    }
    fn emit_plot(&mut self, x: &[f64], y: &[f64], opts: PlotOptions) {
        self.plot_impl(x, y, &opts)
    }
    fn emit_hist(&mut self, x: &[f64], opts: HistOptions) -> HistResult {
        self.hist_impl(x, &opts)
    }
    fn emit_barplot(&mut self, heights: BarHeights, opts: BarplotOptions) -> Vec<f64> {
        self.barplot_impl(heights, &opts)
    }
    fn emit_boxplot(&mut self, groups: &[Vec<f64>], opts: BoxplotOptions) -> Vec<BoxStats> {
        self.boxplot_impl(groups, &opts)
    }
    fn emit_pie(&mut self, x: &[f64], opts: PieOptions) {
        self.pie_impl(x, &opts)
    }
    fn emit_plot_window(&mut self, xlim: (f64, f64), ylim: (f64, f64), opts: PlotWindowOptions) {
        self.plot_window_impl(xlim, ylim, &opts)
    }
    fn emit_points(&mut self, x: &[f64], y: &[f64], opts: XyOptions) {
        self.points_impl(x, y, &opts)
    }
    fn emit_lines(&mut self, x: &[f64], y: &[f64], opts: XyOptions) {
        self.lines_impl(x, y, &opts)
    }
    fn emit_segments(
        &mut self,
        x0: &[f64],
        y0: &[f64],
        x1: &[f64],
        y1: &[f64],
        opts: SegmentsOptions,
    ) {
        self.segments_impl(x0, y0, x1, y1, &opts)
    }
    fn emit_arrows(&mut self, x0: &[f64], y0: &[f64], x1: &[f64], y1: &[f64], opts: ArrowsOptions) {
        self.arrows_impl(x0, y0, x1, y1, &opts)
    }
    fn emit_rect(&mut self, x0: &[f64], y0: &[f64], x1: &[f64], y1: &[f64], opts: RectOptions) {
        self.rect_impl(x0, y0, x1, y1, &opts)
    }
    fn emit_polygon(&mut self, x: &[f64], y: &[f64], opts: PolygonOptions) {
        self.polygon_impl(x, y, &opts)
    }
    fn emit_text(&mut self, x: &[f64], y: &[f64], labels: Vec<Label>, opts: TextOptions) {
        self.text_impl(x, y, &labels, &opts)
    }
    fn emit_box(&mut self, opts: BoxOptions) {
        self.box_impl(&opts)
    }
    fn emit_axis(&mut self, side: Side, opts: AxisOptions) {
        self.axis_impl(side, &opts)
    }
    fn emit_abline(&mut self, opts: AblineOptions) {
        self.abline_impl(&opts)
    }
    fn emit_legend(
        &mut self,
        pos: LegendPos,
        entries: &[LegendEntry],
        opts: LegendOptions,
    ) -> Option<LegendLayout> {
        self.legend_impl(pos, entries, &opts)
    }
    fn emit_mtext(&mut self, text: Label, side: Side, opts: MtextOptions) {
        self.mtext_impl(text, side, &opts)
    }
    fn emit_title(&mut self, opts: TitleOptions) {
        self.title_impl(&opts)
    }
    fn emit_par(&mut self, opts: ParOptions, grid: Option<ParGrid>) {
        // The grid first: it resets `cex` and `mex`, so values set in the
        // same call win.
        match grid {
            Some(ParGrid::Rows(nrow, ncol)) => self.mfrow(nrow, ncol),
            Some(ParGrid::Cols(nrow, ncol)) => self.mfcol(nrow, ncol),
            None => {}
        }
        opts.apply(&mut self.par);
        if let Some(new) = opts.new {
            self.set_new(new);
        }
    }
}

/// Declares a call: its struct (target, data fields, options) and its
/// `.draw()`, which passes them to the target's `emit_*` method.
macro_rules! call {
    (
        $(#[$doc:meta])*
        $name:ident, $opts:ty, $emit:ident -> $ret:ty,
        { $($field:ident : $fty:ty),* $(,)? },
        |$t:ident, $o:ident| ($($arg:expr),* $(,)?)
    ) => {
        $(#[$doc])*
        #[must_use = "nothing is drawn until `.draw()`"]
        pub struct $name<'a, G: Graphics> {
            pub(crate) target: &'a mut G,
            $(pub(crate) $field: $fty,)*
            pub(crate) opts: $opts,
        }

        impl<'a, G: Graphics> $name<'a, G> {
            /// Runs the call with the options set so far.
            pub fn draw(self) -> $ret {
                let $name { target: $t, $($field,)* opts: $o } = self;
                $t.$emit($($arg),*)
            }
        }
    };
}

call!(
    /// [`Graphics::plot`]'s call.
    PlotCall, PlotOptions, emit_plot -> (),
    { x: &'a [f64], y: &'a [f64] },
    |t, o| (x, y, o)
);
call!(
    /// [`Graphics::hist`]'s call.
    HistCall, HistOptions, emit_hist -> HistResult,
    { x: &'a [f64] },
    |t, o| (x, o)
);
call!(
    /// [`Graphics::barplot`]'s call.
    BarplotCall, BarplotOptions, emit_barplot -> Vec<f64>,
    { heights: BarHeights },
    |t, o| (heights, o)
);
call!(
    /// [`Graphics::boxplot`]'s call.
    BoxplotCall, BoxplotOptions, emit_boxplot -> Vec<BoxStats>,
    { groups: &'a [Vec<f64>] },
    |t, o| (groups, o)
);
call!(
    /// [`Graphics::pie`]'s call.
    PieCall, PieOptions, emit_pie -> (),
    { x: &'a [f64] },
    |t, o| (x, o)
);
call!(
    /// [`Graphics::plot_window`]'s call.
    PlotWindowCall, PlotWindowOptions, emit_plot_window -> (),
    { xlim: (f64, f64), ylim: (f64, f64) },
    |t, o| (xlim, ylim, o)
);
call!(
    /// [`Graphics::points`]'s call.
    PointsCall, XyOptions, emit_points -> (),
    { x: &'a [f64], y: &'a [f64] },
    |t, o| (x, y, o)
);
call!(
    /// [`Graphics::lines`]'s call.
    LinesCall, XyOptions, emit_lines -> (),
    { x: &'a [f64], y: &'a [f64] },
    |t, o| (x, y, o)
);
call!(
    /// [`Graphics::segments`]'s call.
    SegmentsCall, SegmentsOptions, emit_segments -> (),
    { x0: &'a [f64], y0: &'a [f64], x1: &'a [f64], y1: &'a [f64] },
    |t, o| (x0, y0, x1, y1, o)
);
call!(
    /// [`Graphics::arrows`]'s call.
    ArrowsCall, ArrowsOptions, emit_arrows -> (),
    { x0: &'a [f64], y0: &'a [f64], x1: &'a [f64], y1: &'a [f64] },
    |t, o| (x0, y0, x1, y1, o)
);
call!(
    /// [`Graphics::rect`]'s call.
    RectCall, RectOptions, emit_rect -> (),
    { x0: &'a [f64], y0: &'a [f64], x1: &'a [f64], y1: &'a [f64] },
    |t, o| (x0, y0, x1, y1, o)
);
call!(
    /// [`Graphics::polygon`]'s call.
    PolygonCall, PolygonOptions, emit_polygon -> (),
    { x: &'a [f64], y: &'a [f64] },
    |t, o| (x, y, o)
);
call!(
    /// [`Graphics::text`]'s call.
    TextCall, TextOptions, emit_text -> (),
    { x: &'a [f64], y: &'a [f64], labels: Vec<Label> },
    |t, o| (x, y, labels, o)
);
call!(
    /// [`Graphics::box_`]'s call.
    BoxCall, BoxOptions, emit_box -> (),
    {},
    |t, o| (o)
);
call!(
    /// [`Graphics::axis`]'s call.
    AxisCall, AxisOptions, emit_axis -> (),
    { side: Side },
    |t, o| (side, o)
);
call!(
    /// [`Graphics::abline`]'s call.
    AblineCall, AblineOptions, emit_abline -> (),
    {},
    |t, o| (o)
);
call!(
    /// [`Graphics::legend`]'s call.
    LegendCall, LegendOptions, emit_legend -> G::LegendResult,
    { pos: LegendPos, entries: &'a [LegendEntry] },
    |t, o| (pos, entries, o)
);
call!(
    /// [`Graphics::mtext`]'s call.
    MtextCall, MtextOptions, emit_mtext -> (),
    { text: Label, side: Side },
    |t, o| (text, side, o)
);
call!(
    /// [`Graphics::title`]'s call.
    TitleCall, TitleOptions, emit_title -> (),
    {},
    |t, o| (o)
);
call!(
    /// [`Graphics::par`]'s call.
    ParCall, ParOptions, emit_par -> (),
    { grid: Option<ParGrid> },
    |t, o| (o, grid)
);

/// A `par()` call's `mfrow`/`mfcol`, which only a [`Device`] takes.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParGrid {
    Rows(usize, usize),
    Cols(usize, usize),
}

/// Page layout, which only a [`Device`] has: a [`Figure`] is one panel.
///
/// [`Figure`]: crate::recording::Figure
impl<B: Backend> ParCall<'_, Device<B>> {
    /// `par(mfrow = c(nrow, ncol))`: splits the page into a grid of
    /// panels, filled row by row; the same as [`Device::mfrow`], which
    /// describes it (it also sets `par(cex)` and `mex`). It is applied
    /// before the call's other settings, so a `cex` or `mex` set in the
    /// same call wins (in R, whichever argument comes last wins). Replaces
    /// an earlier `mfrow`/`mfcol` in the call; kept by `.with()`.
    ///
    /// # Panics
    /// At `.draw()`, if `nrow` or `ncol` is zero.
    pub fn mfrow(mut self, nrow: usize, ncol: usize) -> Self {
        self.grid = Some(ParGrid::Rows(nrow, ncol));
        self
    }

    /// `par(mfcol = c(nrow, ncol))`: as [`mfrow`](ParCall::mfrow), but the
    /// panels are filled column by column ([`Device::mfcol`]).
    ///
    /// # Panics
    /// At `.draw()`, if `nrow` or `ncol` is zero.
    pub fn mfcol(mut self, nrow: usize, ncol: usize) -> Self {
        self.grid = Some(ParGrid::Cols(nrow, ncol));
        self
    }
}
