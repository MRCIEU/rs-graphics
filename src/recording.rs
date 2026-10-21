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
//! such as `plot()` or `hist()` and what is drawn on top of it.
//!
//! ```no_run
//! use rs_graphics::prelude::*;
//!
//! // Build a figure without a device...
//! let mut fig = Figure::new();
//! fig.barplot([3.0, 5.0, 2.0]).names(["a", "b", "c"]).draw();
//! fig.abline().h(4.0).lty("dashed").draw();
//!
//! // ...and draw it later, as often as needed.
//! let mut dev = Device::pdf("plot.pdf").width(8.0).height(4.0).open()?;
//! dev.par().mfrow(1, 2).draw();
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
//! - **What isn't recorded:** device queries (`usr()`, `plot_region()`,
//!   `text_metrics()`, ...) and page layout (`mfrow`, `layout`, `fig`,
//!   ...) are [`Device`] methods only, and a figure's `par()` call has
//!   no `mfrow`/`mfcol` setters (a figure is one panel).
//! - **Data is copied** into the figure. For very large plots, draw on
//!   the device directly.
//!
//! # Saving a figure
//!
//! A figure has no page size, format or resolution, so there is no
//! `Figure::save`: open a device for the file, add the figure, and close
//! the device, as in R (`png(...); replayPlot(p); dev.off()`). The same
//! figure can go to several files:
//!
//! ```no_run
//! use rs_graphics::prelude::*;
//!
//! # fn main() -> std::io::Result<()> {
//! let mut fig = Figure::new();
//! fig.hist(&[2.1, 3.4, 3.9, 4.2, 5.0, 5.3, 6.8]).draw();
//!
//! for format in ["pdf", "png"] {
//!     let mut dev = Device::format(format, format!("hist.{format}"))
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
    AblineOptions, ArrowsOptions, AxisOptions, BoxOptions, Device, LegendEntry, LegendLayout,
    LegendOptions, LegendPos, MtextOptions, PlotWindowOptions, PolygonOptions, RectOptions,
    SegmentsOptions, Side, TextOptions, TitleOptions, XyOptions,
};
use crate::graphics::{Graphics, ParPage};
use crate::highlevel::barplot::bar_layout;
use crate::highlevel::boxplot::boxplot_all_stats;
use crate::highlevel::hist::hist_bins;
use crate::highlevel::{
    BarHeights, BarplotOptions, BoxStats, BoxplotOptions, HistOptions, HistResult, PieOptions,
    PlotOptions,
};
use crate::par::ParOptions;
use crate::plotmath::Label;

/// The contents of a figure region (a plot and its margins), built
/// without a device: a list of recorded [`Graphics`] calls, drawn with
/// [`Device::add_figure`]. See the [module docs](self).
#[derive(Clone, Debug, Default)]
pub struct Figure {
    calls: Vec<Call>,
}

impl Figure {
    /// An empty figure.
    pub fn new() -> Self {
        Figure::default()
    }

    /// The number of recorded calls.
    pub fn len(&self) -> usize {
        self.calls.len()
    }

    /// Whether nothing has been recorded.
    pub fn is_empty(&self) -> bool {
        self.calls.is_empty()
    }
}

/// One recorded call, with copies of its data and options.
///
/// A new R function needs a variant here; the compiler then asks for its
/// replay arm in `add_figure` (an exhaustive `match`) and its `emit_*` on
/// both `Graphics` impls. Also add it to a scene in `tests/figure_replay.rs`,
/// which checks a recorded figure draws what the device does.
#[derive(Clone, Debug)]
enum Call {
    PlotNew,
    Par(ParOptions),
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
    Text(Vec<f64>, Vec<f64>, Vec<Label>, TextOptions),
    Box(BoxOptions),
    Axis(Side, AxisOptions),
    Abline(AblineOptions),
    Legend(LegendPos, Vec<LegendEntry>, LegendOptions),
    Mtext(Vec<Label>, Side, MtextOptions),
    Title(TitleOptions),
}

fn four(x0: &[f64], y0: &[f64], x1: &[f64], y1: &[f64]) -> [Vec<f64>; 4] {
    [x0.to_vec(), y0.to_vec(), x1.to_vec(), y1.to_vec()]
}

impl crate::graphics::sealed::Sealed for Figure {}

/// Records each call. `legend()`'s and `axis()`'s `.draw()` return `()`:
/// their results come from [`Device::add_figure`].
impl Graphics for Figure {
    type LegendResult = ();
    type AxisResult = ();

    fn plot_new(&mut self) {
        self.calls.push(Call::PlotNew);
    }
    fn emit_plot(&mut self, x: &[f64], y: &[f64], opts: PlotOptions) {
        self.calls.push(Call::Plot(x.to_vec(), y.to_vec(), opts));
    }
    fn emit_hist(&mut self, x: &[f64], opts: HistOptions) -> HistResult {
        let result = hist_bins(x, &opts);
        self.calls.push(Call::Hist(x.to_vec(), opts));
        result
    }
    fn emit_barplot(&mut self, heights: BarHeights, opts: BarplotOptions) -> Vec<f64> {
        let centres = bar_layout(&heights, &opts).w_m;
        self.calls.push(Call::Barplot(heights, Box::new(opts)));
        centres
    }
    fn emit_boxplot(&mut self, groups: &[Vec<f64>], opts: BoxplotOptions) -> Vec<BoxStats> {
        let stats = boxplot_all_stats(groups, &opts);
        self.calls
            .push(Call::Boxplot(groups.to_vec(), Box::new(opts)));
        stats
    }
    fn emit_pie(&mut self, x: &[f64], opts: PieOptions) {
        self.calls.push(Call::Pie(x.to_vec(), opts));
    }
    fn emit_plot_window(&mut self, xlim: (f64, f64), ylim: (f64, f64), opts: PlotWindowOptions) {
        self.calls.push(Call::PlotWindow(xlim, ylim, opts));
    }
    fn emit_points(&mut self, x: &[f64], y: &[f64], opts: XyOptions) {
        self.calls.push(Call::Points(x.to_vec(), y.to_vec(), opts));
    }
    fn emit_lines(&mut self, x: &[f64], y: &[f64], opts: XyOptions) {
        self.calls.push(Call::Lines(x.to_vec(), y.to_vec(), opts));
    }
    fn emit_segments(
        &mut self,
        x0: &[f64],
        y0: &[f64],
        x1: &[f64],
        y1: &[f64],
        opts: SegmentsOptions,
    ) {
        self.calls.push(Call::Segments(four(x0, y0, x1, y1), opts));
    }
    fn emit_arrows(&mut self, x0: &[f64], y0: &[f64], x1: &[f64], y1: &[f64], opts: ArrowsOptions) {
        self.calls.push(Call::Arrows(four(x0, y0, x1, y1), opts));
    }
    fn emit_rect(&mut self, x0: &[f64], y0: &[f64], x1: &[f64], y1: &[f64], opts: RectOptions) {
        self.calls.push(Call::Rect(four(x0, y0, x1, y1), opts));
    }
    fn emit_polygon(&mut self, x: &[f64], y: &[f64], opts: PolygonOptions) {
        self.calls.push(Call::Polygon(x.to_vec(), y.to_vec(), opts));
    }
    fn emit_text(&mut self, x: &[f64], y: &[f64], labels: Vec<Label>, opts: TextOptions) {
        self.calls
            .push(Call::Text(x.to_vec(), y.to_vec(), labels, opts));
    }
    fn emit_box(&mut self, opts: BoxOptions) {
        self.calls.push(Call::Box(opts));
    }
    fn emit_axis(&mut self, side: Side, opts: AxisOptions) {
        self.calls.push(Call::Axis(side, opts));
    }
    fn emit_abline(&mut self, opts: AblineOptions) {
        self.calls.push(Call::Abline(opts));
    }
    fn emit_legend(&mut self, pos: LegendPos, entries: &[LegendEntry], opts: LegendOptions) {
        self.calls.push(Call::Legend(pos, entries.to_vec(), opts));
    }
    fn emit_mtext(&mut self, text: Vec<Label>, side: Side, opts: MtextOptions) {
        self.calls.push(Call::Mtext(text, side, opts));
    }
    fn emit_title(&mut self, opts: TitleOptions) {
        self.calls.push(Call::Title(opts));
    }
    fn emit_par(&mut self, opts: ParOptions, page: ParPage) {
        // Only a device's `par()` call has page layout setters.
        debug_assert!(page.is_empty());
        self.calls.push(Call::Par(opts));
    }
}

/// What drawing a [`Figure`] produced that only a device can work out.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct FigureResult {
    /// What each of the figure's `legend()` calls returned on the device, in
    /// order (see [`Graphics::legend`]).
    pub legends: Vec<Option<LegendLayout>>,
    /// What each of the figure's `axis()` calls returned on the device (its
    /// tick positions), in order (see [`Graphics::axis`]).
    pub axes: Vec<Vec<f64>>,
}

impl<B: Backend> Device<B> {
    /// Draws `fig` on this device: its calls in order, exactly as if they
    /// had been made here. A figure starting with a high-level call (other
    /// than one with `add`) or `plot_new()` moves to the next panel, as
    /// that call would.
    ///
    /// `par` is restored afterwards (like R's `op <- par(...);
    /// on.exit(par(op))`), so the figure's `par()` calls don't affect what
    /// is drawn next. The panel position and `par(new)` are device state
    /// and are not restored.
    ///
    /// # Panics
    /// Where the recorded calls would panic on the device (e.g. `abline()`
    /// with a non-finite `coef`).
    pub fn add_figure(&mut self, fig: &Figure) -> FigureResult {
        // `par` comes back afterwards, also if a recorded call panics.
        let mut dev = self.par_scope(|now, saved| *now = saved.clone());
        let mut out = FigureResult::default();
        // Each call goes through the same `emit_*` as a device's `.draw()`.
        for call in &fig.calls {
            let d = &mut *dev;
            match call {
                Call::PlotNew => d.plot_new(),
                Call::Par(o) => d.emit_par(o.clone(), ParPage::default()),
                Call::Plot(x, y, o) => d.emit_plot(x, y, o.clone()),
                Call::Hist(x, o) => {
                    d.emit_hist(x, o.clone());
                }
                Call::Barplot(h, o) => {
                    d.emit_barplot(h.clone(), (**o).clone());
                }
                Call::Boxplot(g, o) => {
                    d.emit_boxplot(g, (**o).clone());
                }
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
                Call::Axis(side, o) => out.axes.push(d.emit_axis(*side, o.clone())),
                Call::Abline(o) => d.emit_abline(o.clone()),
                Call::Legend(pos, entries, o) => {
                    out.legends.push(d.emit_legend(*pos, entries, o.clone()))
                }
                Call::Mtext(text, side, o) => d.emit_mtext(text.clone(), *side, o.clone()),
                Call::Title(o) => d.emit_title(o.clone()),
            }
        }
        drop(dev);
        out
    }

    /// [`add_figure`](Device::add_figure) into the panel at `row`, `col` of
    /// the current `mfrow`/`mfcol`/`layout` arrangement (R's `par(mfg =
    /// c(row, col))` before it), for a figure that starts with a
    /// high-level call or `plot_new()`. On a device with nothing drawn yet
    /// it first starts the page, since R ignores `mfg` until then (the
    /// same as `plot_new()`, then `par(mfg = c(row, col))`).
    ///
    /// # Panics
    /// If `row` or `col` is outside the arrangement.
    pub fn add_figure_at(&mut self, row: usize, col: usize, fig: &Figure) -> FigureResult {
        if !self.is_used() {
            self.plot_new_impl();
        }
        self.par().mfg(row, col).draw();
        self.add_figure(fig)
    }
}
