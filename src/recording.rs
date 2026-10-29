// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Figures as values: a [`Figure`] records [`Graphics`] calls without a
//! device, and [`Device::add_figure`] draws them later (like R's
//! `recordPlot()`/`replayPlot()`).
//!
//! "Figure" has its meaning in R's base graphics: the **figure region**,
//! one plot together with its margins (one panel of an `mfrow`/`layout`
//! page). A `Figure` usually holds the contents of one: a high-level call
//! such as `plot()` or `hist()` and what is drawn on top of it. It can
//! also hold a whole plot: a page layout (`par()`'s `mfrow`, `layout()`,
//! ...) with its panels, over as many pages as it fills, recorded once and
//! drawn on any device.
//!
//! ```no_run
//! use rs_graphics::prelude::*;
//!
//! // Build a figure without a device...
//! let mut fig = Figure::new();
//! fig.barplot().height([3.0, 5.0, 2.0]).names(["a", "b", "c"]).draw();
//! fig.abline().h(4.0).lty("dashed").draw();
//!
//! // ...and draw it later, as often as needed.
//! let mut dev = Device::pdf("plot.pdf").width(8.0).height(4.0).open()?;
//! dev.par().mfrow([1, 2]).draw();
//! dev.add_figure(&fig); // panel 1
//! dev.add_figure(&fig); // panel 2
//! dev.close()?;
//! # Ok::<(), std::io::Error>(())
//! ```
//!
//! - **Same output.** Drawing a `Figure` calls the same code, in the same
//!   order, as making the calls on the device, so the output is
//!   identical.
//! - **Panels.** A figure that starts with a high-level call (`plot`,
//!   `hist`, ..., other than one with `add`) or `plot_new()` moves to the
//!   next panel when added,
//!   exactly as those calls do on a device;
//!   [`add_figure_at`](Device::add_figure_at) puts it in a chosen panel. A
//!   figure without one draws into the current plot (a layer), and one
//!   with several high-level calls fills several panels.
//! - **Results.** `hist()`, `barplot()` and `boxplot()` don't need a
//!   device to work out what they return, so `.draw()` on a `Figure` returns
//!   the same as on a device. `legend()`'s layout depends on the device;
//!   [`Device::add_figure`] returns it.
//! - **Graphical parameters.** Change them with
//!   [`par()`](Graphics::par) calls in the figure (before the high-level
//!   call for `mar`, `pty`, ...). `add_figure` restores the device's `par`
//!   afterwards, so a figure's settings don't leak into the next one.
//! - **Whole plots.** A figure that sets a page layout (`mfrow`, `mfcol`,
//!   `mfg`, `fig`, `fin` or `layout()`) sets it on the device when added
//!   and leaves the device as it left it, `par` included (R's
//!   `replayPlot()`); it can't go into a panel
//!   ([`add_figure_at`](Device::add_figure_at)), as R's layouts don't
//!   nest. Errors in its layout arguments are found when recording, except
//!   an `mfg` outside the device's grid, found when added.
//! - **What isn't recorded:** device queries (`usr()`, `plot_region()`,
//!   `strwidth()`, ...) are [`Device`] methods only.
//! - **Data is copied** into the figure. For very large plots, draw on
//!   the device directly.
//!
//! # Saving a figure
//!
//! A figure has no page size, format or resolution, so there is no
//! `Figure::save`: open a device for the file, add the figure, and close
//! the device, as in R (`png(...); replayPlot(p); dev.off()`). The same
//! figure, here a whole two-panel plot, can go to several files:
//!
//! ```no_run
//! use rs_graphics::prelude::*;
//!
//! # fn main() -> std::io::Result<()> {
//! let x = [2.1, 3.4, 3.9, 4.2, 5.0, 5.3, 6.8];
//! let mut fig = Figure::new();
//! fig.par().mfrow([1, 2]).draw();
//! fig.hist().x(&x).draw(); // left panel
//! fig.boxplot().x(&x).draw(); // right panel
//!
//! for format in ["pdf", "png"] {
//!     let mut dev = Device::format(format, format!("summary.{format}"))
//!         .width(7.0)
//!         .height(5.0)
//!         .res(300.0) // PNG only
//!         .open()?;
//!     dev.add_figure(&fig);
//!     dev.close()?;
//! }
//! # Ok(())
//! # }
//! ```
//!
//! The device's other arguments, such as `bg` or `pointsize`, are set
//! the same way (see [`DeviceBuilder`](crate::device::DeviceBuilder)).

use crate::backend::Backend;
use crate::device::{
    AblineOptions, ArrowsOptions, AxisOptions, BoxOptions, Device, LegendLayout, LegendOptions,
    LegendPos, MtextOptions, PlotWindowOptions, PolygonOptions, RectOptions, SegmentsOptions, Side,
    TextOptions, TitleOptions, XyOptions,
};
use crate::error::Error;
use crate::graphics::{Graphics, ParPage};
use crate::highlevel::barplot::bar_layout;
use crate::highlevel::boxplot::boxplot_all_stats;
use crate::highlevel::hist::hist_bins;
use crate::highlevel::{
    BarHeights, BarplotOptions, BoxStats, BoxplotOptions, HistOptions, HistResult, PieOptions,
    PlotOptions,
};
use crate::layout::{LayoutRespect, LayoutSize};
use crate::par::{Par, ParOptions};
use crate::plotmath::Label;

/// A figure built without a device: a plot and its margins, or a whole
/// multi-panel, multi-page plot, recorded as a list of [`Graphics`] calls
/// and drawn later with [`Device::add_figure`].
///
/// See the [module docs](self) for more.
///
/// # Examples
///
/// ```
/// # use rs_graphics::prelude::*;
/// # let mut dev = Device::null().open().unwrap();
/// let mut fig = Figure::new();
/// fig.plot().x([1, 2, 3]).y([2, 4, 3]).main("Recorded").draw();
/// fig.abline().h(3).lty("dashed").draw();
/// dev.add_figure(&fig);
/// ```
#[derive(Clone, Debug, Default)]
pub struct Figure {
    calls: Vec<Call>,
}

impl Figure {
    /// Creates an empty figure, ready to record calls.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// let mut fig = Figure::new();
    /// fig.plot().x([1, 2, 3]).draw();
    /// assert_eq!(fig.len(), 1);
    /// ```
    pub fn new() -> Self {
        Figure::default()
    }

    /// Returns the number of recorded calls.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// let mut fig = Figure::new();
    /// fig.plot().x([1, 2, 3]).draw();
    /// fig.abline().h(2).draw();
    /// assert_eq!(fig.len(), 2);
    /// ```
    pub fn len(&self) -> usize {
        self.calls.len()
    }

    /// Returns whether nothing has been recorded yet.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// assert!(Figure::new().is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.calls.is_empty()
    }

    /// Whether the figure sets the page layout (`par()`'s `mfrow`, `mfcol`,
    /// `mfg`, `fig`, `fin`, or `layout()`), so is a whole plot rather than
    /// one panel.
    fn sets_page_layout(&self) -> bool {
        self.calls.iter().any(|c| match c {
            Call::Par(_, page) => !page.is_empty(),
            Call::Layout(..) => true,
            _ => false,
        })
    }
}

/// One recorded call, with copies of its data and options.
///
/// A new R function needs a variant here; the compiler then asks for its
/// replay arm in `add_figure` (an exhaustive `match`) and its `emit_*` on
/// both `Graphics` impls. Also add it to a scene in `tests/all/figure_replay.rs`,
/// which checks a recorded figure draws what the device does.
#[derive(Clone, Debug)]
enum Call {
    PlotNew,
    Par(ParOptions, ParPage),
    Layout(
        Vec<Vec<usize>>,
        Vec<LayoutSize>,
        Vec<LayoutSize>,
        LayoutRespect,
    ),
    Plot(Vec<f64>, Vec<f64>, PlotOptions),
    Hist(Vec<f64>, HistOptions),
    // Boxed, as `BoxplotOptions`: much larger than the other options.
    Barplot(BarHeights, Box<BarplotOptions>),
    // Boxed: `BoxplotOptions` is much larger than the other options.
    Boxplot(Vec<Vec<f64>>, Box<BoxplotOptions>),
    Pie(Vec<f64>, PieOptions),
    PlotWindow((f64, f64), (f64, f64), PlotWindowOptions),
    Points(Vec<f64>, Vec<f64>, XyOptions),
    Lines(Vec<f64>, Vec<f64>, XyOptions),
    Segments([Vec<f64>; 4], SegmentsOptions),
    Arrows([Vec<f64>; 4], ArrowsOptions),
    Rect([Vec<f64>; 4], RectOptions),
    Polygon(Vec<f64>, Vec<f64>, PolygonOptions),
    Text(Vec<f64>, Vec<f64>, Vec<Option<Label>>, TextOptions),
    Box(BoxOptions),
    Axis(Side, AxisOptions),
    Abline(AblineOptions),
    Legend(LegendPos, Vec<Option<Label>>, LegendOptions),
    Mtext(Vec<Option<Label>>, Side, MtextOptions),
    Title(TitleOptions),
}

fn four(x0: &[f64], y0: &[f64], x1: &[f64], y1: &[f64]) -> [Vec<f64>; 4] {
    [x0.to_vec(), y0.to_vec(), x1.to_vec(), y1.to_vec()]
}

impl Call {
    /// The R function a recorded call is, for its errors.
    fn r_name(&self) -> &'static str {
        match self {
            Call::PlotNew => "plot.new",
            Call::Par(..) => "par",
            Call::Layout(..) => "layout",
            Call::Plot(..) => "plot",
            Call::Hist(..) => "hist",
            Call::Barplot(..) => "barplot",
            Call::Boxplot(..) => "boxplot",
            Call::Pie(..) => "pie",
            Call::PlotWindow(..) => "plot.window",
            Call::Points(..) => "points",
            Call::Lines(..) => "lines",
            Call::Segments(..) => "segments",
            Call::Arrows(..) => "arrows",
            Call::Rect(..) => "rect",
            Call::Polygon(..) => "polygon",
            Call::Text(..) => "text",
            Call::Box(..) => "box",
            Call::Axis(..) => "axis",
            Call::Abline(..) => "abline",
            Call::Legend(..) => "legend",
            Call::Mtext(..) => "mtext",
            Call::Title(..) => "title",
        }
    }
}

impl crate::graphics::sealed::Sealed for Figure {}

/// Records each call. `legend()`'s and `axis()`'s `.draw()` return `()`:
/// their results come from [`Device::add_figure`].
impl Graphics for Figure {
    type LegendResult = ();
    type AxisResult = ();

    fn try_plot_new(&mut self) -> Result<(), Error> {
        self.calls.push(Call::PlotNew);
        Ok(())
    }
    fn emit_plot(&mut self, x: &[f64], y: &[f64], opts: PlotOptions) -> Result<(), Error> {
        self.calls.push(Call::Plot(x.to_vec(), y.to_vec(), opts));
        Ok(())
    }
    fn emit_hist(&mut self, x: &[f64], opts: HistOptions) -> Result<HistResult, Error> {
        let result = hist_bins(x, &opts)?;
        self.calls.push(Call::Hist(x.to_vec(), opts));
        Ok(result)
    }
    fn emit_barplot(
        &mut self,
        heights: BarHeights,
        opts: BarplotOptions,
    ) -> Result<Vec<f64>, Error> {
        let centres = bar_layout(&heights, &opts)?.w_m;
        self.calls.push(Call::Barplot(heights, Box::new(opts)));
        Ok(centres)
    }
    fn emit_boxplot(
        &mut self,
        groups: &[Vec<f64>],
        opts: BoxplotOptions,
    ) -> Result<Vec<BoxStats>, Error> {
        let stats = boxplot_all_stats(groups, &opts)?;
        self.calls
            .push(Call::Boxplot(groups.to_vec(), Box::new(opts)));
        Ok(stats)
    }
    fn emit_pie(&mut self, x: &[f64], opts: PieOptions) -> Result<(), Error> {
        crate::highlevel::check_pie(x)?;
        self.calls.push(Call::Pie(x.to_vec(), opts));
        Ok(())
    }
    fn emit_plot_window(
        &mut self,
        xlim: (f64, f64),
        ylim: (f64, f64),
        opts: PlotWindowOptions,
    ) -> Result<(), Error> {
        self.calls.push(Call::PlotWindow(xlim, ylim, opts));
        Ok(())
    }
    fn emit_points(&mut self, x: &[f64], y: &[f64], opts: XyOptions) -> Result<(), Error> {
        crate::device::check_lmitre(opts.lmitre)?;
        self.calls.push(Call::Points(x.to_vec(), y.to_vec(), opts));
        Ok(())
    }
    fn emit_lines(&mut self, x: &[f64], y: &[f64], opts: XyOptions) -> Result<(), Error> {
        crate::device::check_lmitre(opts.lmitre)?;
        self.calls.push(Call::Lines(x.to_vec(), y.to_vec(), opts));
        Ok(())
    }
    fn emit_segments(
        &mut self,
        x0: &[f64],
        y0: &[f64],
        x1: &[f64],
        y1: &[f64],
        opts: SegmentsOptions,
    ) -> Result<(), Error> {
        crate::device::check_lmitre(opts.lmitre)?;
        self.calls.push(Call::Segments(four(x0, y0, x1, y1), opts));
        Ok(())
    }
    fn emit_arrows(
        &mut self,
        x0: &[f64],
        y0: &[f64],
        x1: &[f64],
        y1: &[f64],
        opts: ArrowsOptions,
    ) -> Result<(), Error> {
        crate::device::check_lmitre(opts.lmitre)?;
        self.calls.push(Call::Arrows(four(x0, y0, x1, y1), opts));
        Ok(())
    }
    fn emit_rect(
        &mut self,
        x0: &[f64],
        y0: &[f64],
        x1: &[f64],
        y1: &[f64],
        opts: RectOptions,
    ) -> Result<(), Error> {
        crate::device::check_lmitre(opts.lmitre)?;
        self.calls.push(Call::Rect(four(x0, y0, x1, y1), opts));
        Ok(())
    }
    fn emit_polygon(&mut self, x: &[f64], y: &[f64], opts: PolygonOptions) -> Result<(), Error> {
        crate::device::check_lmitre(opts.lmitre)?;
        self.calls.push(Call::Polygon(x.to_vec(), y.to_vec(), opts));
        Ok(())
    }
    fn emit_text(
        &mut self,
        x: &[f64],
        y: &[f64],
        labels: Vec<Option<Label>>,
        opts: TextOptions,
    ) -> Result<(), Error> {
        crate::device::check_text(&opts)?;
        self.calls
            .push(Call::Text(x.to_vec(), y.to_vec(), labels, opts));
        Ok(())
    }
    fn emit_box(&mut self, opts: BoxOptions) -> Result<(), Error> {
        crate::device::check_lmitre(opts.lmitre)?;
        self.calls.push(Call::Box(opts));
        Ok(())
    }
    fn emit_axis(&mut self, side: Side, opts: AxisOptions) -> Result<(), Error> {
        crate::device::check_axis(&opts)?;
        self.calls.push(Call::Axis(side, opts));
        Ok(())
    }
    fn emit_abline(&mut self, opts: AblineOptions) -> Result<(), Error> {
        crate::device::check_lmitre(opts.lmitre)?;
        opts.lines()?;
        self.calls.push(Call::Abline(opts));
        Ok(())
    }
    fn emit_legend(
        &mut self,
        pos: LegendPos,
        labels: Vec<Option<Label>>,
        opts: LegendOptions,
    ) -> Result<(), Error> {
        crate::device::check_legend(&labels, &opts)?;
        self.calls.push(Call::Legend(pos, labels, opts));
        Ok(())
    }
    fn emit_mtext(
        &mut self,
        text: Vec<Option<Label>>,
        side: Side,
        opts: MtextOptions,
    ) -> Result<(), Error> {
        crate::device::check_mtext(&text)?;
        self.calls.push(Call::Mtext(text, side, opts));
        Ok(())
    }
    fn emit_title(&mut self, opts: TitleOptions) -> Result<(), Error> {
        self.calls.push(Call::Title(opts));
        Ok(())
    }
    fn emit_par(&mut self, opts: ParOptions, page: ParPage) -> Result<(), Error> {
        self.calls.push(Call::Par(opts, page));
        Ok(())
    }
    fn emit_layout(
        &mut self,
        rows: Vec<Vec<usize>>,
        widths: Vec<LayoutSize>,
        heights: Vec<LayoutSize>,
        respect: LayoutRespect,
    ) -> Result<(), Error> {
        self.calls
            .push(Call::Layout(rows, widths, heights, respect));
        Ok(())
    }
}

/// What drawing a [`Figure`] produced that only a device can work out:
/// the values its `legend()` and `axis()` calls return on a device.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct FigureResult {
    /// What each of the figure's `legend()` calls returned on the device, in
    /// order (see [`Graphics::legend`]).
    pub legends: Vec<LegendLayout>,
    /// What each of the figure's `axis()` calls returned on the device (its
    /// tick positions), in order (see [`Graphics::axis`]).
    pub axes: Vec<Vec<f64>>,
}

impl<B: Backend> Device<B> {
    /// Draws `fig` on this device, as if its calls had been made here.
    ///
    /// A figure starting with a high-level call (other than one with `add`) or
    /// `plot_new()` moves to the next panel, as that call would. The graphical
    /// parameters are restored afterwards, so the figure's `par()` calls don't
    /// affect what is drawn next; the panel position, [`par(new)`] and the
    /// coordinates (`plot_window()`, `par(usr/xlog/ylog)`) are device state and
    /// are not restored. Returns what the figure's `legend()` and `axis()`
    /// calls returned (see [`FigureResult`]).
    ///
    /// A figure that sets a page layout (`mfrow`, `mfcol`, `mfg`, `fig`,
    /// `fin` or `layout()`) is a whole plot: it sets that layout on the
    /// device, fills its panels and pages, and leaves the device as it left
    /// it, `par` included, as R's `replayPlot()` does (restoring `par` would
    /// undo the `cex` and `mex` the layout set).
    ///
    /// The restoring of `par()` is R's `op <- par(...); on.exit(par(op))`.
    ///
    /// # Examples
    ///
    /// The same figure in two panels:
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// # let mut dev = Device::null().open().unwrap();
    /// let mut fig = Figure::new();
    /// fig.plot().x([1, 2, 3]).y([2, 4, 3]).draw();
    /// dev.par().mfrow([1, 2]).draw();
    /// dev.add_figure(&fig); // left
    /// dev.add_figure(&fig); // right
    /// ```
    ///
    /// # Panics
    ///
    /// With the first R error a recorded call raises on the device, as
    /// [`try_add_figure`](Device::try_add_figure) would return it (for
    /// example `plot.new(): figure margins too large` on a small page).
    ///
    /// [`par(new)`]: crate::par::ParOptions::new
    pub fn add_figure(&mut self, fig: &Figure) -> FigureResult {
        self.try_add_figure(fig).unwrap_or_else(|e| panic!("{e}"))
    }

    /// Draws `fig` as [`add_figure`](Device::add_figure) does, returning the
    /// first R error instead of panicking.
    ///
    /// Arguments are checked when the figure records its calls, so the
    /// errors left here are those only a device can see, chiefly regions
    /// that don't fit its page.
    ///
    /// # Errors
    ///
    /// The first R error a recorded call raises on the device. The calls
    /// before it have been drawn, and `par` is restored, as in R.
    pub fn try_add_figure(&mut self, fig: &Figure) -> Result<FigureResult, Error> {
        // `par` comes back afterwards, also after an error or a panic;
        // except after a figure with a page layout, which sets the device up
        // as a whole plot (R's `replayPlot()`): restoring `par` there would
        // undo the `cex`/`mex` its layout set while the layout stays.
        let restore: fn(&mut Par, &Par) = if fig.sets_page_layout() {
            |_, _| {}
        } else {
            |now, saved| *now = saved.clone()
        };
        let mut dev = self.par_scope(restore);
        let mut out = FigureResult::default();
        // Each call goes through the same `emit_*` as a device's `.draw()`.
        for call in &fig.calls {
            let d = &mut *dev;
            let r: Result<(), Error> = match call {
                Call::PlotNew => d.try_plot_new(),
                Call::Par(o, page) => d.emit_par(o.clone(), *page),
                Call::Layout(rows, w, h, r) => {
                    d.emit_layout(rows.clone(), w.clone(), h.clone(), r.clone())
                }
                Call::Plot(x, y, o) => d.emit_plot(x, y, o.clone()),
                Call::Hist(x, o) => d.emit_hist(x, o.clone()).map(|_| ()),
                Call::Barplot(h, o) => d.emit_barplot(h.clone(), (**o).clone()).map(|_| ()),
                Call::Boxplot(g, o) => d.emit_boxplot(g, (**o).clone()).map(|_| ()),
                Call::Pie(x, o) => d.emit_pie(x, o.clone()),
                Call::PlotWindow(xlim, ylim, o) => d.emit_plot_window(*xlim, *ylim, o.clone()),
                Call::Points(x, y, o) => d.emit_points(x, y, o.clone()),
                Call::Lines(x, y, o) => d.emit_lines(x, y, o.clone()),
                Call::Segments([x0, y0, x1, y1], o) => d.emit_segments(x0, y0, x1, y1, o.clone()),
                Call::Arrows([x0, y0, x1, y1], o) => d.emit_arrows(x0, y0, x1, y1, o.clone()),
                Call::Rect([x0, y0, x1, y1], o) => d.emit_rect(x0, y0, x1, y1, o.clone()),
                Call::Polygon(x, y, o) => d.emit_polygon(x, y, o.clone()),
                Call::Text(x, y, labels, o) => d.emit_text(x, y, labels.clone(), o.clone()),
                Call::Box(o) => d.emit_box(o.clone()),
                Call::Axis(side, o) => d.emit_axis(*side, o.clone()).map(|at| out.axes.push(at)),
                Call::Abline(o) => d.emit_abline(o.clone()),
                Call::Legend(pos, labels, o) => d
                    .emit_legend(*pos, labels.clone(), o.clone())
                    .map(|l| out.legends.push(l)),
                Call::Mtext(text, side, o) => d.emit_mtext(text.clone(), *side, o.clone()),
                Call::Title(o) => d.emit_title(o.clone()),
            };
            r.map_err(|e| e.in_call(call.r_name()))?;
        }
        drop(dev);
        Ok(out)
    }

    /// Draws `fig` in the panel at (`row`, `col`), counting from 1, of the
    /// current `mfrow`/`mfcol`/`layout` arrangement; otherwise as
    /// [`add_figure`](Device::add_figure).
    ///
    /// Meant for a figure that starts with a high-level call or `plot_new()`.
    /// On a device with nothing drawn yet, it first starts the page, since the
    /// panel can't be chosen before then (the same as `plot_new()` followed by
    /// `par()`'s [`mfg`](crate::graphics::ParCall::mfg)).
    ///
    /// This is R's `par(mfg = c(row, col))` before the figure's calls.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// # let mut dev = Device::null().open().unwrap();
    /// let mut fig = Figure::new();
    /// fig.plot().x([1, 2, 3]).draw();
    /// dev.par().mfrow([2, 2]).draw();
    /// dev.add_figure_at(2, 2, &fig); // bottom right
    /// ```
    ///
    /// # Panics
    ///
    /// With R's error, as [`try_add_figure_at`](Device::try_add_figure_at)
    /// would return it.
    pub fn add_figure_at(&mut self, row: usize, col: usize, fig: &Figure) -> FigureResult {
        self.try_add_figure_at(row, col, fig)
            .unwrap_or_else(|e| panic!("{e}"))
    }

    /// Draws `fig` in a chosen panel as
    /// [`add_figure_at`](Device::add_figure_at) does, returning the first R
    /// error instead of panicking.
    ///
    /// # Errors
    ///
    /// If `row` or `col` is outside the arrangement, if the figure sets a
    /// page layout (it fills the whole device: use
    /// [`add_figure`](Device::add_figure)), or as
    /// [`try_add_figure`](Device::try_add_figure).
    pub fn try_add_figure_at(
        &mut self,
        row: usize,
        col: usize,
        fig: &Figure,
    ) -> Result<FigureResult, Error> {
        if fig.sets_page_layout() {
            // R's layouts don't nest.
            return Err(Error::invalid(
                "a figure with a page layout (mfrow, mfcol, fig, fin or layout()) \
                 can't be added into a panel; use add_figure",
            ));
        }
        if !self.is_used() {
            self.try_plot_new()?;
        }
        self.par().mfg((row, col)).try_draw()?;
        self.try_add_figure(fig)
    }
}
