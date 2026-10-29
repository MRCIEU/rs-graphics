// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! The plotting functions, as the methods of the [`Graphics`] trait,
//! implemented by [`Device`] (which draws) and
//! [`Figure`](crate::recording::Figure) (which records, to draw later).
//! Each method is one of R's graphics functions, named as in R.
//!
//! Each method starts a call for one R function; the call's chainable
//! setters take all of R's arguments, named as in R (data too: `.x(&x)`),
//! and `.draw()` runs it. Where R would stop with an error (an argument R
//! requires left out, `argument "x" is missing, with no default`; a value
//! a setter can't convert, such as `.lty("wavy")`; data or regions that
//! don't work), `.try_draw()` returns it and `.draw()` panics with it,
//! also when a `Figure` records the call. As in R, numbers may be integers or floats: data
//! vectors are any [`Numbers`], by reference or by value (`&x`, `x`,
//! `[1, 2, 3]`, `1..=10` for R's `1:10`, or one number, `1`, as R's
//! length-one vector; `f64` data isn't copied, whether
//! borrowed or a `Vec` moved in), and numeric setters
//! take integers too (`.cex(2)`, `.xlim([0, 10])`):
//!
//! ```no_run
//! # use rs_graphics::prelude::*;
//! let mut dev = Device::svg("plot.svg").width(5.0).height(5.0).open()?;
//! dev.plot().x(&[1.0, 2.0, 3.0]).y(&[2.0, 4.0, 3.0]).pch(19).main("Title").draw();
//! dev.abline().h(3.0).col("grey").lty("dashed").draw();
//! dev.close()?;
//! # Ok::<(), std::io::Error>(())
//! ```
//!
//! The setters are those of the call's options struct ([`AblineOptions`]
//! for `abline()`, ...), and `.with(&opts)` starts from a reusable one:
//!
//! ```
//! # use rs_graphics::prelude::*;
//! # let mut dev = Device::null().width(5.0).height(5.0).open().unwrap();
//! # dev.plot_new();
//! let grey = SegmentsOptions::default().col("grey").lty("dashed");
//! dev.segments().x0(&[0.1]).y0(&[0.2]).x1(&[0.9]).y1(&[0.2]).with(&grey).draw();
//! dev.segments().x0(&[0.1]).y0(&[0.8]).x1(&[0.9]).y1(&[0.8]).with(&grey).lwd(2.0).draw();
//! ```
//!
//! Nothing is drawn until `.draw()`; the calls are `#[must_use]`, so a
//! forgotten `.draw()` is a compiler warning.
//!
//! ```compile_fail
//! #![deny(unused_must_use)]
//! # use rs_graphics::prelude::*;
//! let mut dev = Device::memory("svg").width(5.0).height(5.0).open().unwrap();
//! dev.plot_new();
//! dev.abline().h(0.5); // no `.draw()`: nothing drawn, and a warning
//! ```

use std::borrow::Cow;

use crate::backend::Backend;
use crate::device::{
    AblineOptions, ArrowsOptions, AxisOptions, BoxOptions, Device, LayoutCall, LegendLayout,
    LegendOptions, LegendPos, MtextOptions, PlotWindowOptions, PolygonOptions, RectOptions,
    SegmentsOptions, Side, TextOptions, TitleOptions, XyOptions,
};
use crate::error::Error;
use crate::highlevel::{
    BarHeights, BarplotOptions, BoxStats, BoxplotOptions, HistOptions, HistResult, PieOptions,
    PlotOptions,
};
use crate::layout::{LayoutRespect, LayoutSize};
use crate::numbers::{Groups, Num, Numbers, Pair};
use crate::par::ParOptions;
use crate::plotmath::{Label, Labels};
use crate::setters::IntoArg;

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
    /// [`LegendLayout`] on a [`Device`], `()` on a
    /// [`Figure`](crate::recording::Figure) (whose legend layouts come from
    /// [`Device::add_figure`]).
    type LegendResult;

    /// What [`axis`](Graphics::axis)'s `.draw()` returns: the tick
    /// positions (`Vec<f64>`) on a [`Device`], `()` on a
    /// [`Figure`](crate::recording::Figure) (whose tick positions come from
    /// [`Device::add_figure`]).
    type AxisResult;

    /// Starts a new plot: moves to the next figure (panel) and sets up its
    /// figure and plot regions, ready for drawing.
    ///
    /// - **Coordinates:** it leaves linear coordinates of 0..1 on both axes (as
    ///   `plot_window().xlim([0, 1]).ylim([0, 1])`: -0.04..1.04 with the default
    ///   `xaxs = yaxs = "r"`) and their default ticks, so you can draw straight
    ///   away. Call [`plot_window`](Graphics::plot_window) next for other limits
    ///   or log axes.
    /// - **Which figure:** the next one in the order set by `par()`'s
    ///   [`mfrow`](ParCall::mfrow) / [`mfcol`](ParCall::mfcol),
    ///   [`layout()`](Graphics::layout) or `par()`'s [`mfg`](ParCall::mfg) (one figure per
    ///   page by default). After the last figure, it wraps to figure 1 on a new
    ///   page. The first call always starts page one. After `par()`'s
    ///   [`fig`](ParCall::fig) or [`fin`](ParCall::fin), the next call starts a
    ///   new page unless `new(true)` is set too.
    /// - **Overlaying:** after `dev.par().new(true).draw()` it stays in the
    ///   current figure and draws over it. The flag is not cleared here; any
    ///   drawing clears it.
    /// - **Regions:** figure and plot regions are computed now from `par.mar`/
    ///   `oma` (in lines of [`Device::line_height_inches`], so `par.cex`, `mex`
    ///   and the starting `ps` count too), `par.pty` and `par.plt`. Changing
    ///   these afterwards has no effect until the next call, also on margin
    ///   text (`title()`, `mtext()`, `axis(outer = )`), which R would place with
    ///   the new `mar`/`oma`.
    /// - **New page:** what happens depends on the backend. PDF starts a new
    ///   page in the file, or with
    ///   [`onefile(false)`](crate::device::DeviceBuilder::onefile) writes the
    ///   finished page to its own file as SVG and PNG do; SVG and PNG start an
    ///   empty page, after writing the finished one to its own file if the file
    ///   name has a page number (`"plot%03d.png"`; see
    ///   [`DeviceBuilder::open`](crate::device::DeviceBuilder::open)), or else
    ///   discarding it. The page is then filled with `par.bg`, unless that is
    ///   fully transparent, which it is by default on PDF.
    ///
    /// This is R's `plot.new()`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// # let mut dev = Device::null().open().unwrap();
    /// dev.plot_new();
    /// dev.plot_window().xlim([0, 10]).ylim([0, 100]).draw();
    /// dev.points().x([2, 5, 8]).y([10, 50, 90]).draw();
    /// dev.axis().side(1).draw();
    /// dev.axis().side(2).draw();
    /// ```
    ///
    /// # Panics
    ///
    /// With R's message when the regions don't fit: "figure margins too large"
    /// (the margins leave no room for the plot region, e.g. on a small page,
    /// with many panels or with large `oma`), "figure region too large" (`fin()`
    /// or `layout()` cm sizes beyond the inner region), "plot region too large"
    /// ([`par(pin)`] or [`par(plt)`] beyond the figure). Every high-level plot starts
    /// with `plot_new()`, so they fail the same way (on a
    /// [`Figure`](crate::recording::Figure), when it is added to a device). It
    /// first moves back to the previous figure, so after fixing the cause the
    /// next call retries the same figure. Use
    /// [`try_plot_new`](Graphics::try_plot_new) to handle the error.
    ///
    /// [`par(pin)`]: crate::par::Par::pin
    /// [`par(plt)`]: crate::par::Par::plt
    fn plot_new(&mut self) {
        self.try_plot_new().unwrap_or_else(|e| panic!("{e}"))
    }

    /// Starts a new plot as [`plot_new`](Graphics::plot_new) does, returning
    /// R's error instead of panicking.
    ///
    /// # Errors
    ///
    /// If the regions don't fit ([`ErrorKind::DoesNotFit`](crate::error::ErrorKind),
    /// with R's message, e.g. `plot.new(): figure margins too large`); as
    /// in R, the figure counter is stepped back, so a retry after fixing the
    /// cause uses the same figure.
    fn try_plot_new(&mut self) -> Result<(), Error>;

    // ---- high-level plots ----

    /// Starts a new plot and draws `y` against `x` as points (or lines, steps,
    /// ... as set by [`type_`](PlotOptions::type_)), with axes, a box and titles.
    ///
    /// Only [`x`](PlotCall::x) is required; [`y`](PlotCall::y) is optional. With
    /// `x` alone, its values are plotted against their index and the x axis is
    /// titled "Index". The limits default to the range of the data. The other
    /// arguments are in [`PlotOptions`]. Like every high-level plot, it calls
    /// [`plot_new`](Graphics::plot_new) first, so on a multi-panel page it moves
    /// to the next panel.
    ///
    /// This is R's `plot()` for numeric data (`plot.default`).
    ///
    /// # Examples
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// # let mut dev = Device::null().open().unwrap();
    /// let x = [1.0, 2.0, 3.0, 4.0, 5.0];
    /// let y = [2.1, 3.9, 6.2, 7.8, 10.1];
    /// dev.plot().x(&x).y(&y).pch(19).xlab("Dose").ylab("Response").draw();
    /// ```
    ///
    /// # Errors
    ///
    /// `.try_draw()` returns R's error, and `.draw()` panics with it, if `x` is
    /// missing (`argument "x" is missing, with no default`) or `x` and `y` have
    /// different lengths ("'x' and 'y' lengths differ"), with R's messages.
    fn plot(&mut self) -> PlotCall<'_, Self> {
        PlotCall::start(self)
    }

    /// Starts a new plot and draws a histogram of `x`, returning its bins.
    ///
    /// [`x`](HistCall::x) is required. By default the bins come from Sturges'
    /// rule; `breaks` takes a number of bins, another rule or the bin edges
    /// themselves (see [`HistBreaks`](crate::highlevel::HistBreaks)). The other
    /// arguments are in [`HistOptions`]. It starts a new plot (on a multi-panel
    /// page, the next panel) unless `add` is set, which draws the bars on the
    /// current plot.
    ///
    /// `.draw()` returns the breaks, counts, densities and mid-points as a
    /// [`HistResult`]; with `.plot(false)` it returns them without drawing.
    ///
    /// This is R's `hist()`; the result is its `"histogram"` object.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// # let mut dev = Device::null().open().unwrap();
    /// let h = dev.hist().x(&[4.2, 5.1, 5.3, 5.9, 6.4, 7.0, 7.7]).col("lightblue").draw();
    /// assert_eq!(h.counts.iter().sum::<f64>(), 7.0);
    /// ```
    ///
    /// # Errors
    ///
    /// `.try_draw()` returns R's error, and `.draw()` panics with it, if `x` is
    /// missing, or with R's errors for invalid `breaks` (for example edges that
    /// don't span the data).
    fn hist(&mut self) -> HistCall<'_, Self> {
        HistCall::start(self)
    }

    /// Starts a new plot and draws a bar chart of `height`, returning the bar
    /// centres.
    ///
    /// [`height`](BarplotCall::height) is required: a vector (one bar per
    /// value) or a matrix given as its columns (see [`BarHeights`]), whose
    /// columns are drawn as stacked bars, or side by side with `beside`. The
    /// other arguments are in [`BarplotOptions`]. It starts a new plot (on a
    /// multi-panel page, the next panel) unless `add` is set, which draws the
    /// bars on the current plot.
    ///
    /// `.draw()` returns the bar centres along the category axis, for labelling
    /// or overlaying the bars. For a matrix they are in column order: with
    /// `beside`, one per bar (`nrow` per group); otherwise one per column.
    ///
    /// This is R's `barplot()`; the centres are the value it returns.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// # let mut dev = Device::null().open().unwrap();
    /// let mids = dev.barplot().height([3, 5, 2]).names(["A", "B", "C"]).ylim([0, 6]).draw();
    /// dev.text().x(&mids).y([3.3, 5.3, 2.3]).labels(["3", "5", "2"]).draw();
    /// ```
    ///
    /// # Errors
    ///
    /// `.try_draw()` returns R's error, and `.draw()` panics with it, if `height`
    /// is missing or empty, or a matrix's columns are empty or of different
    /// lengths; if `names` has neither one name per bar nor (for `beside`) one per
    /// column; and, with a log value axis, if any `height + offset`, or a given
    /// limit of a log axis, is <= 0.
    fn barplot(&mut self) -> BarplotCall<'_, Self> {
        BarplotCall::start(self)
    }

    /// Starts a new plot and draws a box-and-whisker plot of each group,
    /// returning their statistics.
    ///
    /// [`x`](BoxplotCall::x) is required: the groups, as any [`Groups`] (for
    /// example `&[a, b]` for two `Vec`s of numbers); a single vector is one
    /// group. The other arguments are in [`BoxplotOptions`]. It starts a new
    /// plot (on a multi-panel page, the next panel) unless `add` is set, which
    /// draws the boxes on the current plot. `.draw()` returns each group's
    /// [`BoxStats`]: the box, whiskers and outliers.
    ///
    /// This is R's `boxplot()` for a list of groups (there is no formula
    /// interface).
    ///
    /// # Examples
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// # let mut dev = Device::null().open().unwrap();
    /// let stats = dev
    ///     .boxplot()
    ///     .x(&[vec![1.0, 2.0, 3.0, 9.0], vec![4.0, 5.0, 6.0]])
    ///     .names(["a", "b"])
    ///     .draw();
    /// assert_eq!(stats.len(), 2);
    /// ```
    ///
    /// # Errors
    ///
    /// `.try_draw()` returns R's error, and `.draw()` panics with it, if `x` is
    /// missing or has no groups, or `at` or `width` doesn't have one value per
    /// group (or a width isn't positive).
    fn boxplot(&mut self) -> BoxplotCall<'_, Self> {
        BoxplotCall::start(self)
    }

    /// Starts a new plot and draws a pie chart with one slice per value of `x`.
    ///
    /// [`x`](PieCall::x) is required: the slice sizes, drawn in proportion to
    /// their sum, counter-clockwise from 3 o'clock unless `clockwise` or
    /// `init_angle` say otherwise. The other arguments (labels, colours,
    /// `radius`, ...) are in [`PieOptions`]. On a multi-panel page it moves to
    /// the next panel.
    ///
    /// This is R's `pie()`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// # let mut dev = Device::null().open().unwrap();
    /// dev.pie().x([3, 5, 2]).labels(["A", "B", "C"]).draw();
    /// ```
    ///
    /// # Errors
    ///
    /// `.try_draw()` returns R's error, and `.draw()` panics with it, if `x` is
    /// missing or empty or has a negative or non-finite value ("'x' values must be
    /// positive").
    fn pie(&mut self) -> PieCall<'_, Self> {
        PieCall::start(self)
    }

    // ---- low-level functions ----

    /// Sets the coordinates of the current plot region: the range of data
    /// shown along each axis, and whether an axis is logarithmic.
    ///
    /// [`xlim`](PlotWindowCall::xlim) and [`ylim`](PlotWindowCall::ylim) are
    /// both required; the other arguments are in [`PlotWindowOptions`]. Call it
    /// after [`plot_new`](Graphics::plot_new) and before drawing.
    ///
    /// - `xlim`/`ylim` are in data units, also on a log axis, and may be
    ///   decreasing (a reversed axis).
    /// - `xlog`/`ylog` make that axis log10. [`Device::usr`] is then in log10
    ///   units on that axis.
    /// - `asp` (linear axes only): first widens whichever range is needed so
    ///   that one x unit is `asp` y units long on the page, centred on the given
    ///   limits.
    /// - Each range is then extended: with `par(xaxs/yaxs) = "r"` (the default)
    ///   by 4% at each end; with `"i"` the limits are used as given. A
    ///   zero-width range is widened, to -1..1 around 0, otherwise by 40% of the
    ///   value either side. Non-finite limits are replaced by huge finite ones;
    ///   on a log axis, a limit <= 0 gives a lower end of about 1e-307 (R warns
    ///   there; this doesn't).
    /// - Sets `par.xaxp`/`par.yaxp` (the default tick range and count) from the
    ///   new range and `par.lab`. Later calls to `axis()` without `at` read
    ///   them; changing `par.lab` afterwards doesn't change them.
    ///
    /// This is R's `plot.window()`; its `log = "x"` is `.xlog(true)`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// # let mut dev = Device::null().open().unwrap();
    /// dev.plot_new();
    /// dev.plot_window().xlim([1, 1000]).ylim([0, 1]).xlog(true).draw();
    /// dev.axis().side(1).draw();
    /// ```
    ///
    /// # Errors
    ///
    /// `.try_draw()` returns R's error, and `.draw()` panics with it, if `xlim` or
    /// `ylim` is missing (R's message).
    fn plot_window(&mut self) -> PlotWindowCall<'_, Self> {
        PlotWindowCall::start(self)
    }

    /// Adds points at (`x`, `y`) to the current plot.
    ///
    /// [`x`](PointsCall::x) is required; with `x` alone, its values are plotted
    /// against their index. The other arguments are in [`XyOptions`]: the
    /// symbol (`pch`), size (`cex`), colours and line width, recycled over the
    /// points, and [`type_`](XyOptions::type_) to draw lines instead. Points
    /// with a non-finite coordinate (including values <= 0 on a log axis) are
    /// skipped.
    ///
    /// This is R's `points()`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// # let mut dev = Device::null().open().unwrap();
    /// # dev.plot().x([0, 10]).y([0, 10]).draw();
    /// dev.points().x([2, 3]).y([5, 1]).pch(17).col("red").draw();
    /// ```
    ///
    /// # Errors
    ///
    /// `.try_draw()` returns R's error, and `.draw()` panics with it, if `x` is
    /// missing (R's message) or `x` and `y` have different lengths ("'x' and 'y'
    /// lengths differ").
    fn points(&mut self) -> PointsCall<'_, Self> {
        PointsCall::start(self)
    }

    /// Adds a line through the points (`x`, `y`) to the current plot.
    ///
    /// [`x`](LinesCall::x) is required; with `x` alone, its values are plotted
    /// against their index. The other arguments are in [`XyOptions`], including
    /// [`type_`](XyOptions::type_) for steps, points or both. The line breaks at
    /// non-finite coordinates (missing values, or values <= 0 on a log axis).
    ///
    /// This is R's `lines()`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// # let mut dev = Device::null().open().unwrap();
    /// # dev.plot().x([0, 10]).y([0, 10]).draw();
    /// dev.lines().x([1, 2, 3]).y([2, 4, 3]).col("blue").lwd(2).draw();
    /// ```
    ///
    /// # Errors
    ///
    /// `.try_draw()` returns R's error, and `.draw()` panics with it, if `x` is
    /// missing (R's message) or `x` and `y` have different lengths ("'x' and 'y'
    /// lengths differ").
    fn lines(&mut self) -> LinesCall<'_, Self> {
        LinesCall::start(self)
    }

    /// Adds straight line segments from (`x0`, `y0`) to (`x1`, `y1`).
    ///
    /// [`x0`](SegmentsCall::x0) and [`y0`](SegmentsCall::y0) are required;
    /// [`x1`](SegmentsCall::x1) and [`y1`](SegmentsCall::y1) default to them.
    /// The other arguments are in [`SegmentsOptions`]. The coordinates and
    /// styles are recycled to the longest; a segment with a non-finite end
    /// (including values <= 0 on a log axis) is skipped.
    ///
    /// This is R's `segments()`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// # let mut dev = Device::null().open().unwrap();
    /// # dev.plot().x([0, 10]).y([0, 10]).draw();
    /// dev.segments().x0([1, 2]).y0([1, 1]).x1([1, 2]).y1([3, 4]).draw();
    /// ```
    ///
    /// # Errors
    ///
    /// `.try_draw()` returns R's error, and `.draw()` panics with it, if `x0` or
    /// `y0` is missing (R's message).
    fn segments(&mut self) -> SegmentsCall<'_, Self> {
        SegmentsCall::start(self)
    }

    /// Adds arrows from (`x0`, `y0`) to (`x1`, `y1`): segments with open heads,
    /// two lines of `length` inches at `angle` degrees to the shaft, at the
    /// start, the end or both (`code`).
    ///
    /// The coordinates are as for [`segments`](Graphics::segments)
    /// ([`x0`](ArrowsCall::x0) and [`y0`](ArrowsCall::y0) required); the other
    /// arguments are in [`ArrowsOptions`]. Coordinates and styles are recycled
    /// to the longest. An arrow with a non-finite end (including values <= 0
    /// on a log axis) is skipped, and the heads of a segment shorter than
    /// 1/1000 inch are not drawn.
    ///
    /// This is R's `arrows()`; its error-bar idiom `angle = 90, code = 3` is
    /// `.angle(90).code(3)`.
    ///
    /// # Examples
    ///
    /// Error bars:
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// # let mut dev = Device::null().open().unwrap();
    /// # dev.plot().x([0, 10]).y([0, 10]).draw();
    /// dev.arrows().x0([1, 2]).y0([2, 3]).y1([4, 5]).angle(90).code(3).length(0.05).draw();
    /// ```
    ///
    /// # Errors
    ///
    /// `.try_draw()` returns R's error, and `.draw()` panics with it, if `x0` or
    /// `y0` is missing (R's message).
    fn arrows(&mut self) -> ArrowsCall<'_, Self> {
        ArrowsCall::start(self)
    }

    /// Adds rectangles, given their left, bottom, right and top edges.
    ///
    /// The four edges, [`xleft`](RectCall::xleft),
    /// [`ybottom`](RectCall::ybottom), [`xright`](RectCall::xright) and
    /// [`ytop`](RectCall::ytop), are all required; the other arguments are in
    /// [`RectOptions`]. Coordinates and styles are recycled to the longest.
    /// Without `density`, a rectangle with a non-finite corner (for example at
    /// 0 on a log axis) is skipped. With `density`, the rectangles are shaded
    /// with lines and drawn as polygons, so a non-finite corner splits the
    /// outline.
    ///
    /// This is R's `rect()`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// # let mut dev = Device::null().open().unwrap();
    /// # dev.plot().x([0, 10]).y([0, 10]).draw();
    /// dev.rect().xleft(1).ybottom(1).xright(3).ytop(2).col("grey").draw();
    /// ```
    ///
    /// # Errors
    ///
    /// `.try_draw()` returns R's error, and `.draw()` panics with it, if a
    /// coordinate is missing (R's message).
    fn rect(&mut self) -> RectCall<'_, Self> {
        RectCall::start(self)
    }

    /// Adds polygons with vertices at (`x`, `y`).
    ///
    /// [`x`](PolygonCall::x) is required, with [`y`](PolygonCall::y); the other
    /// arguments are in [`PolygonOptions`]. Non-finite coordinates (missing
    /// values) separate polygons: one is drawn for each run of at least two
    /// finite points, with `col`, `border` and `lty` recycled over the polygons
    /// drawn (a shorter run is skipped and uses up no colour). With `density`,
    /// each polygon is shaded with lines (see [`PolygonOptions::density`]), or
    /// filled where its density is `None` or negative, with `col`, `lty`,
    /// `density` and `angle` recycled over every run of at least one point;
    /// then all borders are drawn.
    ///
    /// This is R's `polygon()`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// # let mut dev = Device::null().open().unwrap();
    /// # dev.plot().x([0, 10]).y([0, 10]).draw();
    /// dev.polygon().x([2, 5, 8]).y([2, 8, 2]).col("lightblue").border("navy").draw();
    /// ```
    ///
    /// # Errors
    ///
    /// `.try_draw()` returns R's error, and `.draw()` panics with it, if `x` is
    /// missing (R's message) or `x` and `y` have different lengths ("'x' and 'y'
    /// lengths differ").
    fn polygon(&mut self) -> PolygonCall<'_, Self> {
        PolygonCall::start(self)
    }

    /// Adds text labels at (`x`, `y`).
    ///
    /// [`x`](TextCall::x) is required; with `x` alone, the labels go at its
    /// values against their index. [`labels`](TextCall::labels) are text or
    /// plotmath expressions (`Label::math("alpha^2")`); without them, the
    /// labels are the indices `"1"`, `"2"`, .... The other arguments are in
    /// [`TextOptions`]. `x` and `y` are recycled to the longer, and the labels
    /// over the points (extra labels are ignored). Horizontally, `adj` sets
    /// which part of a label is at its point (0 its left end, 0.5 its centre, 1
    /// its right end); vertically see [`TextOptions::vadj`]; labels are rotated
    /// by `srt`, else [`par(srt)`]. A label with newlines (`"one\ntwo"`) is
    /// drawn line by line, [`par(lheight)`] line heights apart, as all text
    /// is. A label at a non-finite position (a missing value, or a value <=
    /// 0 on a log axis) is skipped.
    ///
    /// This is R's `text()`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// # let mut dev = Device::null().open().unwrap();
    /// # dev.plot().x([0, 10]).y([0, 10]).draw();
    /// dev.text().x([2, 5]).y([8, 3]).labels(["peak", "dip"]).col("red").draw();
    /// ```
    ///
    /// # Errors
    ///
    /// `.try_draw()` returns R's error, and `.draw()` panics with it, if `x` is
    /// missing (R's message) or a `pos` isn't 1 to 4 ("invalid 'pos' value").
    ///
    /// [`par(lheight)`]: crate::par::Par::lheight
    /// [`par(srt)`]: crate::par::Par::srt
    fn text(&mut self) -> TextCall<'_, Self> {
        TextCall::start(self)
    }

    /// Draws a box around the plot region, the figure, the inner region or the
    /// whole device.
    ///
    /// Which one is set by `which` in [`BoxOptions`] (the plot region by
    /// default); the box is not clipped. For the plot region, `bty` (else
    /// [`par(bty)`]) picks the sides: `"o"` all four, `"l"`, `"7"`, `"c"`, `"u"`
    /// and `"]"` the sides those characters suggest, `"n"` none.
    ///
    /// This is R's `box()` (`box` is a Rust keyword, hence `box_`).
    ///
    /// # Examples
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// # let mut dev = Device::null().open().unwrap();
    /// # dev.plot().x([0, 10]).y([0, 10]).draw();
    /// dev.box_().bty("l").lwd(2).draw();
    /// ```
    ///
    /// [`par(bty)`]: crate::par::Par::bty
    fn box_(&mut self) -> BoxCall<'_, Self> {
        BoxCall::start(self)
    }

    /// Adds an axis to one side of the current plot: a line, tick marks and
    /// labels.
    ///
    /// [`side`](AxisCall::side) is required: a [`Side`], or its number (1
    /// below, 2 left, 3 above, 4 right). The other arguments, such as the tick
    /// positions `at` and their `labels`, are in [`AxisOptions`]; without `at`,
    /// the ticks are the plot's default ones.
    ///
    /// On a [`Device`], `.draw()` returns the tick positions (`at`, or the
    /// default ticks) in data units, sorted, with `NaN` last, including ticks
    /// outside the plot that weren't drawn; it is empty when `at` is empty or
    /// the axis type is `"n"`. On a [`Figure`](crate::recording::Figure) it
    /// returns `()`; the positions come from [`Device::add_figure`].
    ///
    /// This is R's `axis()`; `dev.abline().h(dev.axis().side(2).draw())` is
    /// R's `abline(h = axis(2))`.
    ///
    /// # Examples
    ///
    /// Grid lines at the ticks of the left axis:
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// # let mut dev = Device::null().open().unwrap();
    /// # dev.plot().x([0, 10]).y([0, 10]).draw();
    /// let at = dev.axis().side(2).draw();
    /// dev.abline().h(at).col("grey").lty("dotted").draw();
    /// ```
    ///
    /// # Errors
    ///
    /// `.try_draw()` returns R's error, and `.draw()` panics with it, if
    /// `side` is missing or not 1 to 4, `labels` is given without `at` or
    /// their lengths differ, or a log axis's `axp` code isn't 1, 2 or 3.
    fn axis(&mut self) -> AxisCall<'_, Self> {
        AxisCall::start(self)
    }

    /// Adds straight lines across the plot: lines `y = a + b * x`, horizontal
    /// lines at `h` and vertical lines at `v`.
    ///
    /// The arguments are in [`AblineOptions`]. `a` and `b` may give several
    /// lines (`.a([1, 2]).b([2, 3])`; one side may have a single value), and
    /// `col`, `lty` and `lwd` are recycled over all the lines in the order
    /// `a`/`b`, `h`, `v`. For a fitted model, pass its intercept and slope as
    /// `a` and `b`.
    ///
    /// This is R's `abline()`, except that `a` and `b` can give several lines,
    /// and R's `coef` and `reg` are written as `a` and `b`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// # let mut dev = Device::null().open().unwrap();
    /// # dev.plot().x([0, 10]).y([0, 10]).draw();
    /// dev.abline().h(6).col("grey").lty("dashed").draw();
    /// dev.abline().a(1).b(0.5).col("red").draw();
    /// ```
    ///
    /// # Errors
    ///
    /// `.try_draw()` returns R's error, and `.draw()` panics with it, if only one
    /// of `a` and `b` is given, if both have several values but not as many, or if
    /// a value isn't finite.
    fn abline(&mut self) -> AblineCall<'_, Self> {
        AblineCall::start(self)
    }

    /// Adds a legend: a key (a filled box, a line or a symbol) and a label for
    /// each entry, in a box placed by a keyword or by coordinates.
    ///
    /// [`labels`](LegendCall::labels) (text or plotmath expressions; one or
    /// several, see [`Labels`]) and [`pos`](LegendCall::pos) are required.
    /// `pos` is a keyword such as `"topright"`, a point `[x, y]` for the
    /// top-left corner, or two corners `[[xa, xb], [ya, yb]]` (see
    /// [`LegendPos`]). The other arguments are in [`LegendOptions`]; the
    /// per-key ones (`col`, `pch`, `lty`, `lwd`, `fill`, ...) are recycled over
    /// the keys. On a [`Device`], `.draw()` returns where the box and labels
    /// went, in `usr` units ([`LegendLayout`]). With `.plot(false)` it only
    /// works that out, drawing nothing, so a legend can be measured first.
    ///
    /// This is R's `legend()`: `pos` is its `x` and `y`, `labels` its `legend`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// # let mut dev = Device::null().open().unwrap();
    /// # dev.plot().x([0, 10]).y([0, 10]).draw();
    /// dev.legend().pos("topright").labels(["a", "b"]).pch(19).col([1, 2]).draw();
    /// dev.legend().pos((5, 8)).labels("fit").lty(2).lwd(2).draw();
    /// ```
    ///
    /// # Errors
    ///
    /// `.try_draw()` returns R's error, and `.draw()` panics with it, if `pos`
    /// or `labels` is missing, `pos` is a string that isn't one of the nine
    /// keywords (or a unique prefix of one), there are no labels ("'legend' is of length
    /// 0"), or a `text_width` is negative or has `NaN` among several values.
    fn legend(&mut self) -> LegendCall<'_, Self> {
        LegendCall::start(self)
    }

    /// Writes text in a margin of the plot, or in the outer margins.
    ///
    /// [`text`](MtextCall::text) is required: text, or a plotmath expression
    /// (`Label::math(...)`) aligned on its bounding box by `adj`/`padj`.
    /// [`side`](MtextCall::side) defaults to 3, the top. The other arguments
    /// (the margin line, `at`, `outer`, ...) are in [`MtextOptions`]; the text
    /// and those arguments are recycled over the longest. Text is rotated
    /// according to `las` (else [`par(las)`]), and a `None` text (a missing
    /// value) is skipped.
    ///
    /// This is R's `mtext()`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// # let mut dev = Device::null().open().unwrap();
    /// # dev.plot().x([0, 10]).y([0, 10]).draw();
    /// dev.mtext().text("Right margin").side(4).line(1).draw();
    /// ```
    ///
    /// # Errors
    ///
    /// `.try_draw()` returns R's error, and `.draw()` panics with it, if
    /// `text` is missing or empty, or `side` isn't 1 to 4.
    ///
    /// [`par(las)`]: crate::par::Par::las
    fn mtext(&mut self) -> MtextCall<'_, Self> {
        MtextCall::start(self)
    }

    /// Adds titles: a main title above the plot, a subtitle below it, and the
    /// axis titles.
    ///
    /// Each is text or a plotmath expression, set in [`TitleOptions`] along
    /// with their sizes, colours and fonts (`cex_main`, `col_main`,
    /// `font_main` and the `sub` and `lab` ones; else `par()`'s). They are
    /// drawn in the order main, sub, xlab, ylab. The main title is centred on
    /// the line halfway out in the top margin; the subtitle and axis titles go
    /// on their margin lines (set by [`par(mgp)`]).
    ///
    /// This is R's `title()`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// # let mut dev = Device::null().open().unwrap();
    /// # dev.plot_new();
    /// # dev.box_().draw();
    /// dev.title().main("Growth").xlab("Day").ylab("Height (cm)").draw();
    /// ```
    ///
    /// [`par(mgp)`]: crate::par::Par::mgp
    fn title(&mut self) -> TitleCall<'_, Self> {
        TitleCall::start(self)
    }

    /// Changes graphical parameters for what is drawn next: margins, colours,
    /// fonts, line styles, axis styles and, on a device, the page layout.
    ///
    /// The parameters are in [`ParOptions`] (each a [`Par`](crate::par::Par)
    /// field, plus `new`, `usr`, `xlog` and `ylog`, which are device state).
    /// This is the one way to change them, on a [`Device`] and in a recorded
    /// [`Figure`](crate::recording::Figure); read the current values with
    /// [`Device::current_par`]. Setting one unit of a region clears the others
    /// (`mar` clears `mai`, `pin` and `plt`, `oma` clears `omi` and `omd`,
    /// ...), setting `oma`/`omi`/`omd` makes the next plot start a new page,
    /// and `mar`, `oma`, `pty` and `plt` take effect at the next
    /// [`plot_new`](Graphics::plot_new).
    ///
    /// The call also takes the page layout: [`mfrow`](ParCall::mfrow),
    /// [`mfcol`](ParCall::mfcol), [`mfg`](ParCall::mfg), [`fig`](ParCall::fig)
    /// and [`fin`](ParCall::fin). In a `Figure` they are recorded too, so the
    /// figure holds a whole multi-panel plot (see
    /// [`add_figure`](Device::add_figure)).
    ///
    /// This is R's `par()`.
    ///
    /// # Examples
    ///
    /// Two panels side by side, with room above them for an outer title:
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// # let mut dev = Device::null().open().unwrap();
    /// dev.par().mfrow([1, 2]).oma([0, 0, 2, 0]).las(1).draw();
    /// ```
    ///
    /// # Errors
    ///
    /// `.try_draw()` returns R's error, and `.draw()` panics with it (also when a
    /// `Figure` records the call), if `usr` has a `NaN` or `x0 == x1` or `y0 == y1`
    /// ("invalid value specified for graphical parameter \"usr\""), or `lmitre`
    /// is below 1 (the same message, naming `lmitre`). Some page layout values
    /// are checked on a device only (see [`mfrow`](ParCall::mfrow) and the
    /// other layout setters).
    fn par(&mut self) -> ParCall<'_, Self> {
        ParCall::start(self)
    }

    /// Starts dividing the page into figures of different sizes: a grid in
    /// which each figure takes one or more cells.
    ///
    /// Set the grid with [`mat`](crate::device::LayoutCall::mat) (required) and,
    /// optionally, [`widths`](crate::device::LayoutCall::widths),
    /// [`heights`](crate::device::LayoutCall::heights) and
    /// [`respect`](crate::device::LayoutCall::respect), then apply it with
    /// [`draw`](crate::device::LayoutCall::draw). Like `par()`'s
    /// [`mfrow`](ParCall::mfrow), it sets [`par(cex)`] and
    /// `mex` from the grid's size, and the next `plot_new()` starts a new
    /// page at the first figure. It replaces `mfrow`/`mfcol` and `fig`/`fin`.
    /// A [`Figure`](crate::recording::Figure) records it like the rest, so a
    /// figure can hold a whole multi-panel plot.
    ///
    /// This is R's `layout()`.
    ///
    /// # Examples
    ///
    /// A wide panel on top and two below, the top one twice as tall:
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// # let mut dev = Device::null().open().unwrap();
    /// let n = dev.layout().mat(&[[1, 1], [2, 3]]).heights([2, 1]).draw();
    /// assert_eq!(n, 3);
    /// dev.plot().x([1, 2, 3]).draw(); // top
    /// dev.hist().x([1, 2, 2, 3]).draw(); // bottom left
    /// dev.pie().x([1, 2]).draw(); // bottom right
    /// ```
    ///
    /// [`par(cex)`]: crate::par::Par::cex
    fn layout(&mut self) -> LayoutCall<'_, Self> {
        LayoutCall::start(self)
    }

    // ---- what `.draw()` calls; not part of the public API ----

    #[doc(hidden)]
    fn emit_plot(&mut self, x: &[f64], y: &[f64], opts: PlotOptions) -> Result<(), Error>;
    #[doc(hidden)]
    fn emit_hist(&mut self, x: &[f64], opts: HistOptions) -> Result<HistResult, Error>;
    #[doc(hidden)]
    fn emit_barplot(
        &mut self,
        heights: BarHeights,
        opts: BarplotOptions,
    ) -> Result<Vec<f64>, Error>;
    #[doc(hidden)]
    fn emit_boxplot(
        &mut self,
        groups: &[Vec<f64>],
        opts: BoxplotOptions,
    ) -> Result<Vec<BoxStats>, Error>;
    #[doc(hidden)]
    fn emit_pie(&mut self, x: &[f64], opts: PieOptions) -> Result<(), Error>;
    #[doc(hidden)]
    fn emit_plot_window(
        &mut self,
        xlim: (f64, f64),
        ylim: (f64, f64),
        opts: PlotWindowOptions,
    ) -> Result<(), Error>;
    #[doc(hidden)]
    fn emit_points(&mut self, x: &[f64], y: &[f64], opts: XyOptions) -> Result<(), Error>;
    #[doc(hidden)]
    fn emit_lines(&mut self, x: &[f64], y: &[f64], opts: XyOptions) -> Result<(), Error>;
    #[doc(hidden)]
    fn emit_segments(
        &mut self,
        x0: &[f64],
        y0: &[f64],
        x1: &[f64],
        y1: &[f64],
        opts: SegmentsOptions,
    ) -> Result<(), Error>;
    #[doc(hidden)]
    fn emit_arrows(
        &mut self,
        x0: &[f64],
        y0: &[f64],
        x1: &[f64],
        y1: &[f64],
        opts: ArrowsOptions,
    ) -> Result<(), Error>;
    #[doc(hidden)]
    fn emit_rect(
        &mut self,
        x0: &[f64],
        y0: &[f64],
        x1: &[f64],
        y1: &[f64],
        opts: RectOptions,
    ) -> Result<(), Error>;
    #[doc(hidden)]
    fn emit_polygon(&mut self, x: &[f64], y: &[f64], opts: PolygonOptions) -> Result<(), Error>;
    #[doc(hidden)]
    fn emit_text(
        &mut self,
        x: &[f64],
        y: &[f64],
        labels: Vec<Option<Label>>,
        opts: TextOptions,
    ) -> Result<(), Error>;
    #[doc(hidden)]
    fn emit_box(&mut self, opts: BoxOptions) -> Result<(), Error>;
    #[doc(hidden)]
    fn emit_axis(&mut self, side: Side, opts: AxisOptions) -> Result<Self::AxisResult, Error>;
    #[doc(hidden)]
    fn emit_abline(&mut self, opts: AblineOptions) -> Result<(), Error>;
    #[doc(hidden)]
    fn emit_legend(
        &mut self,
        pos: LegendPos,
        labels: Vec<Option<Label>>,
        opts: LegendOptions,
    ) -> Result<Self::LegendResult, Error>;
    #[doc(hidden)]
    fn emit_mtext(
        &mut self,
        text: Vec<Option<Label>>,
        side: Side,
        opts: MtextOptions,
    ) -> Result<(), Error>;
    #[doc(hidden)]
    fn emit_title(&mut self, opts: TitleOptions) -> Result<(), Error>;
    #[doc(hidden)]
    fn emit_par(&mut self, opts: ParOptions, page: ParPage) -> Result<(), Error>;
    #[doc(hidden)]
    fn emit_layout(
        &mut self,
        rows: Vec<Vec<usize>>,
        widths: Vec<LayoutSize>,
        heights: Vec<LayoutSize>,
        respect: LayoutRespect,
    ) -> Result<(), Error>;
}

impl<B: Backend> Graphics for Device<B> {
    type LegendResult = LegendLayout;
    type AxisResult = Vec<f64>;

    fn try_plot_new(&mut self) -> Result<(), Error> {
        self.run(|d| d.plot_new_impl())
    }
    fn emit_plot(&mut self, x: &[f64], y: &[f64], opts: PlotOptions) -> Result<(), Error> {
        self.run(|d| d.plot_impl(x, y, &opts))
    }
    fn emit_hist(&mut self, x: &[f64], opts: HistOptions) -> Result<HistResult, Error> {
        self.run(|d| d.hist_impl(x, &opts))
    }
    fn emit_barplot(
        &mut self,
        heights: BarHeights,
        opts: BarplotOptions,
    ) -> Result<Vec<f64>, Error> {
        self.run(|d| d.barplot_impl(heights, &opts))
    }
    fn emit_boxplot(
        &mut self,
        groups: &[Vec<f64>],
        opts: BoxplotOptions,
    ) -> Result<Vec<BoxStats>, Error> {
        self.run(|d| d.boxplot_impl(groups, &opts))
    }
    fn emit_pie(&mut self, x: &[f64], opts: PieOptions) -> Result<(), Error> {
        self.run(|d| d.pie_impl(x, &opts))
    }
    fn emit_plot_window(
        &mut self,
        xlim: (f64, f64),
        ylim: (f64, f64),
        opts: PlotWindowOptions,
    ) -> Result<(), Error> {
        self.run(|d| {
            d.plot_window_impl(xlim, ylim, &opts);
            Ok(())
        })
    }
    fn emit_points(&mut self, x: &[f64], y: &[f64], opts: XyOptions) -> Result<(), Error> {
        self.run(|d| d.points_impl(x, y, &opts))
    }
    fn emit_lines(&mut self, x: &[f64], y: &[f64], opts: XyOptions) -> Result<(), Error> {
        self.run(|d| d.lines_impl(x, y, &opts))
    }
    fn emit_segments(
        &mut self,
        x0: &[f64],
        y0: &[f64],
        x1: &[f64],
        y1: &[f64],
        opts: SegmentsOptions,
    ) -> Result<(), Error> {
        self.run(|d| d.segments_impl(x0, y0, x1, y1, &opts))
    }
    fn emit_arrows(
        &mut self,
        x0: &[f64],
        y0: &[f64],
        x1: &[f64],
        y1: &[f64],
        opts: ArrowsOptions,
    ) -> Result<(), Error> {
        self.run(|d| d.arrows_impl(x0, y0, x1, y1, &opts))
    }
    fn emit_rect(
        &mut self,
        x0: &[f64],
        y0: &[f64],
        x1: &[f64],
        y1: &[f64],
        opts: RectOptions,
    ) -> Result<(), Error> {
        self.run(|d| d.rect_impl(x0, y0, x1, y1, &opts))
    }
    fn emit_polygon(&mut self, x: &[f64], y: &[f64], opts: PolygonOptions) -> Result<(), Error> {
        self.run(|d| d.polygon_impl(x, y, &opts))
    }
    fn emit_text(
        &mut self,
        x: &[f64],
        y: &[f64],
        labels: Vec<Option<Label>>,
        opts: TextOptions,
    ) -> Result<(), Error> {
        self.run(|d| d.text_impl(x, y, &labels, &opts))
    }
    fn emit_box(&mut self, opts: BoxOptions) -> Result<(), Error> {
        self.run(|d| d.box_impl(&opts))
    }
    fn emit_axis(&mut self, side: Side, opts: AxisOptions) -> Result<Vec<f64>, Error> {
        self.run(|d| d.axis_impl(side, &opts))
    }
    fn emit_abline(&mut self, opts: AblineOptions) -> Result<(), Error> {
        self.run(|d| d.abline_impl(&opts))
    }
    fn emit_legend(
        &mut self,
        pos: LegendPos,
        labels: Vec<Option<Label>>,
        opts: LegendOptions,
    ) -> Result<LegendLayout, Error> {
        self.run(|d| d.legend_impl(pos, &labels, &opts))
    }
    fn emit_mtext(
        &mut self,
        text: Vec<Option<Label>>,
        side: Side,
        opts: MtextOptions,
    ) -> Result<(), Error> {
        self.run(|d| d.mtext_impl(&text, side, &opts))
    }
    fn emit_title(&mut self, opts: TitleOptions) -> Result<(), Error> {
        self.run(|d| {
            d.title_impl(&opts);
            Ok(())
        })
    }
    fn emit_layout(
        &mut self,
        rows: Vec<Vec<usize>>,
        widths: Vec<LayoutSize>,
        heights: Vec<LayoutSize>,
        respect: LayoutRespect,
    ) -> Result<(), Error> {
        self.run(|d| d.layout_impl(&rows, &widths, &heights, &respect))
    }
    fn emit_par(&mut self, opts: ParOptions, page: ParPage) -> Result<(), Error> {
        self.run(|d| {
            // The page layout first: a grid resets `cex` and `mex`, so values
            // set in the same call win; `fig`/`fin` replace the grid, and
            // `mfg` picks a panel of it. As in R, an error stops the call
            // where it is.
            match page.grid {
                Some(ParGrid::Rows(nrow, ncol)) => d.mfrow(nrow, ncol)?,
                Some(ParGrid::Cols(nrow, ncol)) => d.mfcol(nrow, ncol)?,
                None => {}
            }
            if let Some(fig) = page.fig {
                d.fig(fig)?;
            }
            if let Some(fin) = page.fin {
                d.fin(fin)?;
            }
            if let Some((row, col)) = page.mfg {
                d.mfg(row, col)?;
            }
            // R checks `xaxp`/`yaxp` against the axis's log scale.
            if let (Some(axp), true) = (opts.xaxp, opts.xlog.unwrap_or(d.xlog())) {
                crate::device::check_log_axp("xaxp", axp)?;
            }
            if let (Some(axp), true) = (opts.yaxp, opts.ylog.unwrap_or(d.ylog())) {
                crate::device::check_log_axp("yaxp", axp)?;
            }
            opts.apply(&mut d.par);
            // R's `par(oma/omi/omd = )`: "force eject of multiple figures",
            // so the next `plot_new()` starts a new page.
            if opts.sets_outer_margins() {
                d.eject_page();
            }
            if let Some(new) = opts.new {
                d.set_new(new);
            }
            d.set_log(opts.xlog, opts.ylog);
            if let Some(usr) = opts.usr {
                d.set_usr(usr);
            }
            Ok(())
        })
    }
}

/// `x`, `y` and whether `x` was plotted against its index.
type XyCoords<'a> = (Cow<'a, [f64]>, Cow<'a, [f64]>, bool);

/// R's `xy.coords()` for `plot()`, `points()`, `lines()`, `polygon()` and
/// `text()`: `x` is required; without `y`, `x` is plotted against its
/// index `1, 2, ...` (R's one-vector form, `plot(v)`), and the returned
/// flag says so. Unless `recycle` (`text()`: R's `xy.coords(recycle =
/// TRUE)`), `x` and `y` must have one length.
fn xy_coords<'a>(
    x: Option<Cow<'a, [f64]>>,
    y: Option<Cow<'a, [f64]>>,
    recycle: bool,
) -> Result<XyCoords<'a>, Error> {
    let x = required(x, "x")?;
    match y {
        Some(y) => {
            if !recycle && x.len() != y.len() {
                return Err(Error::invalid("'x' and 'y' lengths differ"));
            }
            Ok((x, y, false))
        }
        None => {
            let index: Vec<f64> = (1..=x.len()).map(|i| i as f64).collect();
            Ok((index.into(), x, true))
        }
    }
}

/// An R argument without a default: its value, or R's error.
pub(crate) fn required<T>(value: Option<T>, name: &str) -> Result<T, Error> {
    value.ok_or_else(|| Error::missing(name))
}

/// Declares a call: its struct (target, R's arguments, options), a
/// constructor, a setter for each argument and its `.draw()`.
///
/// Each argument is stored as an `Option` (`None` until its setter is
/// called); `setter: Type => conversion` generates the setter, and an
/// argument without one has its setters written by hand. `state` fields
/// aren't R arguments. The `draw` block resolves the arguments (R's
/// defaults, or [`required`]) and calls the target's `emit_*` method.
macro_rules! call {
    (@setter [$(#[$doc:meta])*] $field:ident (manual)) => {};
    // A conversion that can fail: the error is kept for `try_draw()`.
    (@setter [$(#[$doc:meta])*] $field:ident (fallible $aty:ty => $conv:expr)) => {
        $(#[$doc])*
        pub fn $field(mut self, value: $aty) -> Self {
            match ($conv)(value) {
                Ok(v) => self.$field = Some(v),
                Err(e) => self.opts.defer(e),
            }
            self
        }
    };
    (@setter [$(#[$doc:meta])*] $field:ident ($aty:ty => $conv:expr)) => {
        $(#[$doc])*
        pub fn $field(mut self, value: $aty) -> Self {
            self.$field = Some(($conv)(value));
            self
        }
    };
    (
        $(#[$doc:meta])*
        $name:ident ($rname:literal), $opts:ty => $ret:ty,
        args { $( $(#[$fdoc:meta])* $field:ident : $fty:ty = $setter:tt ),* $(,)? },
        $(state { $($sfield:ident : $sty:ty),* $(,)? },)?
        |$t:ident, $o:ident| $body:block
    ) => {
        $(#[$doc])*
        #[must_use = "nothing is drawn until `.draw()`"]
        pub struct $name<'a, G: Graphics> {
            pub(crate) target: &'a mut G,
            $(pub(crate) $field: Option<$fty>,)*
            $($(pub(crate) $sfield: $sty,)*)?
            pub(crate) opts: $opts,
        }

        impl<'a, G: Graphics> $name<'a, G> {
            pub(crate) fn start(target: &'a mut G) -> Self {
                $name {
                    target,
                    $($field: None,)*
                    $($($sfield: Default::default(),)*)?
                    opts: <$opts>::default(),
                }
            }

            $( call!(@setter [$(#[$fdoc])*] $field $setter); )*

            /// Runs the call with the arguments and options set so far: draws
            /// it on a [`Device`], or records it in a
            /// [`Figure`](crate::recording::Figure).
            ///
            /// Returns what the R function returns, where it returns something
            /// (see the call's method); otherwise `()`.
            ///
            /// # Panics
            ///
            /// With R's error, as [`try_draw`](Self::try_draw) would return
            /// it (`plot.new(): figure margins too large`).
            pub fn draw(self) -> $ret {
                self.try_draw().unwrap_or_else(|e| panic!("{e}"))
            }

            /// Runs the call as [`draw`](Self::draw) does, returning R's error
            /// instead of panicking.
            ///
            /// # Errors
            ///
            /// Every R error the call can raise: a missing required argument,
            /// an invalid value (also one a setter couldn't convert, such as
            /// `.lty("wavy")`; the first such is returned), data that can't
            /// be drawn as given, or regions that don't fit the page (see
            /// [`Error`](crate::error::Error)). An error found while drawing
            /// (such as a negative colour number) may come after part of the
            /// output was drawn, as in R.
            pub fn try_draw(mut self) -> Result<$ret, crate::error::Error> {
                if let Some(e) = self.opts.deferred_error.take() {
                    return Err(e.in_call($rname));
                }
                #[allow(unused_variables)]
                let $name { target: $t, $($field,)* $($($sfield,)*)? opts: $o } = self;
                // A closure, so that `?` in the body ends the body only.
                #[allow(clippy::redundant_closure_call)]
                let result: Result<$ret, crate::error::Error> = (|| $body)();
                result.map_err(|e| e.in_call($rname))
            }
        }
    };
}

call!(
    /// A [`plot()`](Graphics::plot) call being built: it starts a new plot and
    /// draws `y` against `x`.
    ///
    /// Set its arguments with the setters below, then run it with
    /// [`draw`](PlotCall::draw).
    /// Without `.draw()`, nothing happens (the compiler warns).
    ///
    /// See [`Graphics::plot`] for the arguments and an example.
    PlotCall("plot"), PlotOptions => (),
    args {
        /// The x coordinates, or, without [`y`](PlotCall::y), the values to
        /// plot against their index (the x axis is then titled "Index").
        /// Required.
        x: Cow<'a, [f64]> = (impl Numbers<'a> => Numbers::into_numbers),
        /// The y coordinates, as many as `x`. Default: none, and `x` is
        /// plotted against its index.
        y: Cow<'a, [f64]> = (impl Numbers<'a> => Numbers::into_numbers),
    },
    |t, o| {
        let (x, y, index) = xy_coords(x, y, false)?;
        let mut o = o;
        if index && o.xlab.is_none() {
            o.xlab = Some("Index".into());
        }
        t.emit_plot(&x, &y, o)
    }
);
call!(
    /// A [`hist()`](Graphics::hist) call being built: it starts a new plot and
    /// draws a histogram.
    ///
    /// Set its arguments with the setters below, then run it with
    /// [`draw`](HistCall::draw), which returns the bins and counts
    /// ([`HistResult`]).
    /// Without `.draw()`, nothing happens (the compiler warns).
    ///
    /// See [`Graphics::hist`] for the arguments and an example.
    HistCall("hist"), HistOptions => HistResult,
    args {
        /// The values to bin. Required.
        x: Cow<'a, [f64]> = (impl Numbers<'a> => Numbers::into_numbers),
    },
    |t, o| { t.emit_hist(&required(x, "x")?, o) }
);
call!(
    /// A [`barplot()`](Graphics::barplot) call being built: it starts a new
    /// plot and draws a bar chart.
    ///
    /// Set its arguments with the setters below, then run it with
    /// [`draw`](BarplotCall::draw), which returns the bar centres.
    /// Without `.draw()`, nothing happens (the compiler warns).
    ///
    /// See [`Graphics::barplot`] for the arguments and an example.
    BarplotCall("barplot"), BarplotOptions => Vec<f64>,
    args {
        /// The bar heights: a vector, one bar per value, or a matrix given
        /// as its columns (see [`BarHeights`]). Required.
        height: BarHeights = (impl Into<BarHeights> => Into::into),
    },
    |t, o| { t.emit_barplot(required(height, "height")?, o) }
);
call!(
    /// A [`boxplot()`](Graphics::boxplot) call being built: it starts a new
    /// plot and draws box-and-whisker plots.
    ///
    /// Set its arguments with the setters below, then run it with
    /// [`draw`](BoxplotCall::draw), which returns each box's statistics
    /// ([`BoxStats`]).
    /// Without `.draw()`, nothing happens (the compiler warns).
    ///
    /// See [`Graphics::boxplot`] for the arguments and an example.
    BoxplotCall("boxplot"), BoxplotOptions => Vec<BoxStats>,
    args {
        /// The groups: any [`Groups`], e.g. `&[a, b]` or `&groups`; one
        /// vector (`&x`) is one group. Required.
        x: Cow<'a, [Vec<f64>]> = (impl Groups<'a> => Groups::into_groups),
    },
    |t, o| { t.emit_boxplot(&required(x, "x")?, o) }
);
call!(
    /// A [`pie()`](Graphics::pie) call being built: it starts a new plot and
    /// draws a pie chart.
    ///
    /// Set its arguments with the setters below, then run it with
    /// [`draw`](PieCall::draw).
    /// Without `.draw()`, nothing happens (the compiler warns).
    ///
    /// See [`Graphics::pie`] for the arguments and an example.
    PieCall("pie"), PieOptions => (),
    args {
        /// The slice sizes. Required.
        x: Cow<'a, [f64]> = (impl Numbers<'a> => Numbers::into_numbers),
    },
    |t, o| { t.emit_pie(&required(x, "x")?, o) }
);
call!(
    /// A [`plot_window()`](Graphics::plot_window) call being built: it sets
    /// the plot region's coordinates.
    ///
    /// Set its arguments with the setters below, then run it with
    /// [`draw`](PlotWindowCall::draw).
    /// Without `.draw()`, nothing happens (the compiler warns).
    ///
    /// See [`Graphics::plot_window`] for the arguments and an example.
    PlotWindowCall("plot.window"), PlotWindowOptions => (),
    args { xlim: (f64, f64) = (manual), ylim: (f64, f64) = (manual) },
    |t, o| { t.emit_plot_window(required(xlim, "xlim")?, required(ylim, "ylim")?, o) }
);

impl<G: Graphics> PlotWindowCall<'_, G> {
    /// The x range shown, `[from, to]` in data units (also on a log axis);
    /// decreasing for a reversed axis. Takes an array or tuple of two
    /// numbers (see [`Pair`]). Required.
    pub fn xlim(mut self, xlim: impl Pair) -> Self {
        self.xlim = Some(xlim.into_pair());
        self
    }

    /// The y range shown; as [`xlim`](PlotWindowCall::xlim). Required.
    pub fn ylim(mut self, ylim: impl Pair) -> Self {
        self.ylim = Some(ylim.into_pair());
        self
    }
}

call!(
    /// A [`points()`](Graphics::points) call being built: it adds points.
    ///
    /// Set its arguments with the setters below, then run it with
    /// [`draw`](PointsCall::draw).
    /// Without `.draw()`, nothing happens (the compiler warns).
    ///
    /// See [`Graphics::points`] for the arguments and an example.
    PointsCall("points"), XyOptions => (),
    args {
        /// The x coordinates, or, without [`y`](PointsCall::y), the values
        /// to plot against their index. Required.
        x: Cow<'a, [f64]> = (impl Numbers<'a> => Numbers::into_numbers),
        /// The y coordinates, as many as `x`. Default: none, and `x` is
        /// plotted against its index.
        y: Cow<'a, [f64]> = (impl Numbers<'a> => Numbers::into_numbers),
    },
    |t, o| {
        let (x, y, _) = xy_coords(x, y, false)?;
        t.emit_points(&x, &y, o)
    }
);
call!(
    /// A [`lines()`](Graphics::lines) call being built: it adds a line.
    ///
    /// Set its arguments with the setters below, then run it with
    /// [`draw`](LinesCall::draw).
    /// Without `.draw()`, nothing happens (the compiler warns).
    ///
    /// See [`Graphics::lines`] for the arguments and an example.
    LinesCall("lines"), XyOptions => (),
    args {
        /// The x coordinates, or, without [`y`](LinesCall::y), the values
        /// to draw against their index. Required.
        x: Cow<'a, [f64]> = (impl Numbers<'a> => Numbers::into_numbers),
        /// The y coordinates, as many as `x`. Default: none, and `x` is
        /// plotted against its index.
        y: Cow<'a, [f64]> = (impl Numbers<'a> => Numbers::into_numbers),
    },
    |t, o| {
        let (x, y, _) = xy_coords(x, y, false)?;
        t.emit_lines(&x, &y, o)
    }
);
call!(
    /// A [`segments()`](Graphics::segments) call being built: it adds line
    /// segments.
    ///
    /// Set its arguments with the setters below, then run it with
    /// [`draw`](SegmentsCall::draw).
    /// Without `.draw()`, nothing happens (the compiler warns).
    ///
    /// See [`Graphics::segments`] for the arguments and an example.
    SegmentsCall("segments"), SegmentsOptions => (),
    args {
        /// The x coordinates of the starts. Required.
        x0: Cow<'a, [f64]> = (impl Numbers<'a> => Numbers::into_numbers),
        /// The y coordinates of the starts. Required.
        y0: Cow<'a, [f64]> = (impl Numbers<'a> => Numbers::into_numbers),
        /// The x coordinates of the ends. Default: `x0`.
        x1: Cow<'a, [f64]> = (impl Numbers<'a> => Numbers::into_numbers),
        /// The y coordinates of the ends. Default: `y0`.
        y1: Cow<'a, [f64]> = (impl Numbers<'a> => Numbers::into_numbers),
    },
    |t, o| {
        let [x0, y0, x1, y1] = segment_ends(x0, y0, x1, y1)?;
        t.emit_segments(&x0, &y0, &x1, &y1, o)
    }
);
call!(
    /// A [`arrows()`](Graphics::arrows) call being built: it adds arrows.
    ///
    /// Set its arguments with the setters below, then run it with
    /// [`draw`](ArrowsCall::draw).
    /// Without `.draw()`, nothing happens (the compiler warns).
    ///
    /// See [`Graphics::arrows`] for the arguments and an example.
    ArrowsCall("arrows"), ArrowsOptions => (),
    args {
        /// The x coordinates of the starts. Required.
        x0: Cow<'a, [f64]> = (impl Numbers<'a> => Numbers::into_numbers),
        /// The y coordinates of the starts. Required.
        y0: Cow<'a, [f64]> = (impl Numbers<'a> => Numbers::into_numbers),
        /// The x coordinates of the ends. Default: `x0`.
        x1: Cow<'a, [f64]> = (impl Numbers<'a> => Numbers::into_numbers),
        /// The y coordinates of the ends. Default: `y0`.
        y1: Cow<'a, [f64]> = (impl Numbers<'a> => Numbers::into_numbers),
    },
    |t, o| {
        let [x0, y0, x1, y1] = segment_ends(x0, y0, x1, y1)?;
        t.emit_arrows(&x0, &y0, &x1, &y1, o)
    }
);
call!(
    /// A [`rect()`](Graphics::rect) call being built: it adds rectangles.
    ///
    /// Set its arguments with the setters below, then run it with
    /// [`draw`](RectCall::draw).
    /// Without `.draw()`, nothing happens (the compiler warns).
    ///
    /// See [`Graphics::rect`] for the arguments and an example.
    RectCall("rect"), RectOptions => (),
    args {
        /// The left edges. Required.
        xleft: Cow<'a, [f64]> = (impl Numbers<'a> => Numbers::into_numbers),
        /// The bottom edges. Required.
        ybottom: Cow<'a, [f64]> = (impl Numbers<'a> => Numbers::into_numbers),
        /// The right edges. Required.
        xright: Cow<'a, [f64]> = (impl Numbers<'a> => Numbers::into_numbers),
        /// The top edges. Required.
        ytop: Cow<'a, [f64]> = (impl Numbers<'a> => Numbers::into_numbers),
    },
    |t, o| {
        t.emit_rect(
            &required(xleft, "xleft")?,
            &required(ybottom, "ybottom")?,
            &required(xright, "xright")?,
            &required(ytop, "ytop")?,
            o,
        )
    }
);
call!(
    /// A [`polygon()`](Graphics::polygon) call being built: it adds polygons.
    ///
    /// Set its arguments with the setters below, then run it with
    /// [`draw`](PolygonCall::draw).
    /// Without `.draw()`, nothing happens (the compiler warns).
    ///
    /// See [`Graphics::polygon`] for the arguments and an example.
    PolygonCall("polygon"), PolygonOptions => (),
    args {
        /// The x coordinates of the vertices, or, without
        /// [`y`](PolygonCall::y), the y values against their index.
        /// Required.
        x: Cow<'a, [f64]> = (impl Numbers<'a> => Numbers::into_numbers),
        /// The y coordinates, as many as `x`. Default: none, and `x` is
        /// plotted against its index.
        y: Cow<'a, [f64]> = (impl Numbers<'a> => Numbers::into_numbers),
    },
    |t, o| {
        let (x, y, _) = xy_coords(x, y, false)?;
        t.emit_polygon(&x, &y, o)
    }
);
call!(
    /// A [`text()`](Graphics::text) call being built: it adds text labels.
    ///
    /// Set its arguments with the setters below, then run it with
    /// [`draw`](TextCall::draw).
    /// Without `.draw()`, nothing happens (the compiler warns).
    ///
    /// See [`Graphics::text`] for the arguments and an example.
    TextCall("text"), TextOptions => (),
    args {
        /// The x coordinates of the labels, or, without [`y`](TextCall::y),
        /// the y values against their index. Required.
        x: Cow<'a, [f64]> = (impl Numbers<'a> => Numbers::into_numbers),
        /// The y coordinates of the labels; `x` and `y` are recycled to the
        /// longer. Default: none, and `x` gives the y values against their
        /// index.
        y: Cow<'a, [f64]> = (impl Numbers<'a> => Numbers::into_numbers),
        /// The labels: text or plotmath expressions, one or several (see
        /// [`Labels`]); a `None` element (R's `NA`) is skipped. Default:
        /// `"1"`, `"2"`, ....
        labels: Vec<Option<Label>> = (impl Labels => Labels::into_labels),
    },
    |t, o| {
        let (x, y, _) = xy_coords(x, y, true)?;
        t.emit_text(&x, &y, labels.unwrap_or_default(), o)
    }
);
call!(
    /// A [`box_()`](Graphics::box_) call being built: it draws a box around a
    /// region.
    ///
    /// Set its arguments with the setters below, then run it with
    /// [`draw`](BoxCall::draw).
    /// Without `.draw()`, nothing happens (the compiler warns).
    ///
    /// See [`Graphics::box_`] for the arguments and an example.
    BoxCall("box"), BoxOptions => (),
    args {},
    |t, o| { t.emit_box(o) }
);
call!(
    /// A [`axis()`](Graphics::axis) call being built: it adds an axis.
    ///
    /// Set its arguments with the setters below, then run it with
    /// [`draw`](AxisCall::draw), which returns on a [`Device`], the tick
    /// positions.
    /// Without `.draw()`, nothing happens (the compiler warns).
    ///
    /// See [`Graphics::axis`] for the arguments and an example.
    AxisCall("axis"), AxisOptions => G::AxisResult,
    args {
        /// The side to draw on: a [`Side`], or its number (1 below, 2 left,
        /// 3 above, 4 right). Required. Another number is an error of the
        /// call.
        side: Side = (fallible impl IntoArg<Side> => IntoArg::into_arg),
    },
    |t, o| { t.emit_axis(required(side, "side")?, o) }
);
call!(
    /// A [`abline()`](Graphics::abline) call being built: it adds straight
    /// lines across the plot.
    ///
    /// Set its arguments with the setters below, then run it with
    /// [`draw`](AblineCall::draw).
    /// Without `.draw()`, nothing happens (the compiler warns).
    ///
    /// See [`Graphics::abline`] for the arguments and an example.
    AblineCall("abline"), AblineOptions => (),
    args {},
    |t, o| { t.emit_abline(o) }
);
call!(
    /// A [`legend()`](Graphics::legend) call being built: it adds a legend.
    ///
    /// Set its arguments with the setters below, then run it with
    /// [`draw`](LegendCall::draw), which returns on a [`Device`], where the
    /// legend went ([`LegendLayout`]).
    /// Without `.draw()`, nothing happens (the compiler warns).
    ///
    /// See [`Graphics::legend`] for the arguments and an example.
    LegendCall("legend"), LegendOptions => G::LegendResult,
    args {
        /// Where the legend goes: a keyword such as `"topright"`, a point
        /// `[x, y]` for its top-left corner, or two corners `[[xa, xb], [ya,
        /// yb]]` (tuples work too; see [`LegendPos`]). Required. A string that isn't one of
        /// the nine keywords (or a unique prefix of one) is an error of the
        /// call. R's `x` and `y`.
        pos: LegendPos = (fallible impl IntoArg<LegendPos> => IntoArg::into_arg),
        /// The keys' labels: text or plotmath expressions, one or several
        /// (see [`Labels`]); a `None` element (R's `NA`) keeps its key but
        /// has no text and takes no room. Required. R's `legend`.
        labels: Vec<Option<Label>> = (impl Labels => Labels::into_labels),
    },
    |t, o| { t.emit_legend(required(pos, "pos")?, required(labels, "labels")?, o) }
);
call!(
    /// A [`mtext()`](Graphics::mtext) call being built: it writes text in a
    /// margin.
    ///
    /// Set its arguments with the setters below, then run it with
    /// [`draw`](MtextCall::draw).
    /// Without `.draw()`, nothing happens (the compiler warns).
    ///
    /// See [`Graphics::mtext`] for the arguments and an example.
    MtextCall("mtext"), MtextOptions => (),
    args {
        /// The text: text or plotmath expressions, one or several (see
        /// [`Labels`]), recycled with the other arguments; a `None` element
        /// (R's `NA`) is skipped. Required.
        text: Vec<Option<Label>> = (impl Labels => Labels::into_labels),
        /// The margin to write in: a [`Side`], or its number (1 below, 2
        /// left, 3 above, 4 right). Default: 3, the top. Another number is
        /// an error of the call.
        side: Side = (fallible impl IntoArg<Side> => IntoArg::into_arg),
    },
    |t, o| { t.emit_mtext(required(text, "text")?, side.unwrap_or(Side::Top), o) }
);
call!(
    /// A [`title()`](Graphics::title) call being built: it adds titles.
    ///
    /// Set its arguments with the setters below, then run it with
    /// [`draw`](TitleCall::draw).
    /// Without `.draw()`, nothing happens (the compiler warns).
    ///
    /// See [`Graphics::title`] for the arguments and an example.
    TitleCall("title"), TitleOptions => (),
    args {},
    |t, o| { t.emit_title(o) }
);
call!(
    /// A [`par()`](Graphics::par) call being built: it changes graphical
    /// parameters.
    ///
    /// Set its arguments with the setters below, then run it with
    /// [`draw`](ParCall::draw).
    /// Without `.draw()`, nothing happens (the compiler warns).
    ///
    /// See [`Graphics::par`] for the arguments and an example.
    ParCall("par"), ParOptions => (),
    args {},
    state { page: ParPage },
    |t, o| {
        o.check()?;
        page.check()?;
        t.emit_par(o, page)
    }
);

/// `segments()`/`arrows()`'s ends: `x0` and `y0` required, `x1` and `y1`
/// defaulting to them, as in R.
fn segment_ends<'a>(
    x0: Option<Cow<'a, [f64]>>,
    y0: Option<Cow<'a, [f64]>>,
    x1: Option<Cow<'a, [f64]>>,
    y1: Option<Cow<'a, [f64]>>,
) -> Result<[Cow<'a, [f64]>; 4], Error> {
    let (x0, y0) = (required(x0, "x0")?, required(y0, "y0")?);
    let x1 = x1.unwrap_or_else(|| x0.clone());
    let y1 = y1.unwrap_or_else(|| y0.clone());
    Ok([x0, y0, x1, y1])
}

/// A `par()` call's page layout arguments (`mfrow`/`mfcol`, `mfg`, `fig`,
/// `fin`).
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ParPage {
    pub(crate) grid: Option<ParGrid>,
    pub(crate) mfg: Option<(usize, usize)>,
    pub(crate) fig: Option<[f64; 4]>,
    pub(crate) fin: Option<[f64; 2]>,
}

impl ParPage {
    pub(crate) fn is_empty(&self) -> bool {
        *self == ParPage::default()
    }

    /// R's checks of the values, made when the call is drawn or recorded
    /// (`mfg`'s range depends on the device's grid, so is checked there).
    pub(crate) fn check(&self) -> Result<(), Error> {
        match self.grid {
            Some(ParGrid::Rows(r, c)) => crate::device::check_grid("mfrow", r, c)?,
            Some(ParGrid::Cols(r, c)) => crate::device::check_grid("mfcol", r, c)?,
            None => {}
        }
        if let Some(fig) = self.fig {
            crate::device::check_fig(fig)?;
        }
        if let Some(fin) = self.fin {
            crate::device::check_fin(fin)?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ParGrid {
    Rows(usize, usize),
    Cols(usize, usize),
}

/// Page layout: on a device, or recorded in a
/// [`Figure`](crate::recording::Figure), which can then hold a whole
/// multi-panel plot.
/// A pair of grid counts (`mfrow`, `mfcol`, `mfg`) as R's `as.integer()`
/// reads them: fractions truncated. A negative or non-finite value becomes
/// 0, which R's checks reject ("invalid value specified for graphical
/// parameter").
fn grid_pair(p: impl Pair) -> (usize, usize) {
    let count = |v: f64| {
        if v.is_finite() && v >= 0.0 {
            v as usize
        } else {
            0
        }
    };
    let (a, b) = p.into_pair();
    (count(a), count(b))
}

impl<G: Graphics> ParCall<'_, G> {
    /// Splits the page into a grid of `[nrow, ncol]` panels, filled row by
    /// row (an array or tuple of two counts; fractions are truncated, as in
    /// R).
    ///
    /// It also sets [`par(cex)`] to 0.83 (2 x 2) or 0.66 (more than 2 rows or
    /// columns) and `mex` to 1, and the next `plot_new()` starts a new page at
    /// the first panel. It replaces `fig`/`fin` and [`layout()`](Graphics::layout).
    ///
    /// In one call the page layout is applied first, in the order grid, `fig`,
    /// `fin`, `mfg`, then the other settings, then `new`: so a `cex` or `mex`
    /// set in the same call wins (in R, whichever argument comes last wins).
    /// Replaces an earlier `mfrow`/`mfcol` in the call; kept by `.with()`.
    ///
    /// This is R's `par(mfrow = c(nrow, ncol))`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// # let mut dev = Device::null().open().unwrap();
    /// dev.par().mfrow([1, 2]).draw();
    /// dev.plot().x([1, 2, 3]).draw(); // left panel
    /// dev.hist().x([1, 2, 2, 3]).draw(); // right panel
    /// ```
    ///
    /// # Errors
    ///
    /// `.try_draw()` returns R's error, and `.draw()` panics with it, if `nrow` or
    /// `ncol` is below 1 (or not finite).
    ///
    /// [`par(cex)`]: crate::par::Par::cex
    pub fn mfrow(mut self, mfrow: impl Pair) -> Self {
        let (nrow, ncol) = grid_pair(mfrow);
        self.page.grid = Some(ParGrid::Rows(nrow, ncol));
        self
    }

    /// Splits the page into a grid of `[nrow, ncol]` panels, filled column by
    /// column; otherwise as [`mfrow`](ParCall::mfrow).
    ///
    /// This is R's `par(mfcol = c(nrow, ncol))`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// # let mut dev = Device::null().open().unwrap();
    /// dev.par().mfcol([2, 1]).draw();
    /// dev.plot().x([1, 2, 3]).draw(); // top panel
    /// dev.plot().x([3, 2, 1]).draw(); // bottom panel
    /// ```
    ///
    /// # Errors
    ///
    /// `.try_draw()` returns R's error, and `.draw()` panics with it, if `nrow` or
    /// `ncol` is below 1 (or not finite).
    pub fn mfcol(mut self, mfcol: impl Pair) -> Self {
        let (nrow, ncol) = grid_pair(mfcol);
        self.page.grid = Some(ParGrid::Cols(nrow, ncol));
        self
    }

    /// Picks the panel the next plot draws into: the one at `[row, col]`,
    /// counting from 1.
    ///
    /// It neither advances nor starts a new page. The panel number is derived
    /// from the `mfrow`/`mfcol` ordering, even under [`layout()`](Graphics::layout), and it
    /// has no effect before anything has been plotted: the first `plot_new()`
    /// always starts at panel 1 (see [`add_figure_at`](Device::add_figure_at),
    /// which handles that).
    ///
    /// This is R's `par(mfg = c(row, col))`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// # let mut dev = Device::null().open().unwrap();
    /// dev.par().mfrow([2, 2]).draw();
    /// dev.plot().x([1, 2, 3]).draw(); // top left
    /// dev.par().mfg([2, 2]).draw();
    /// dev.plot().x([3, 2, 1]).draw(); // bottom right
    /// ```
    ///
    /// # Errors
    ///
    /// `.try_draw()` returns R's error, and `.draw()` panics with it, if
    /// `row`/`col` are below 1 or outside the current arrangement.
    pub fn mfg(mut self, mfg: impl Pair) -> Self {
        self.page.mfg = Some(grid_pair(mfg));
        self
    }

    /// Places the next figure anywhere on the page, given as fractions
    /// `[x0, x1, y0, y1]` of the inner region (the device less [`par(oma)`]).
    ///
    /// The next `plot_new()` starts a new page unless `new` is set too
    /// (`.fig(..).new(true)` adds a plot to the current page). [`par(cex)`] is
    /// left as it is, and `mfrow`/`mfcol`/[`layout()`](Graphics::layout) replace it.
    ///
    /// This is R's `par(fig = c(x0, x1, y0, y1))`.
    ///
    /// # Examples
    ///
    /// An inset in the top right corner:
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// # let mut dev = Device::null().open().unwrap();
    /// dev.plot().x([1, 2, 3, 4]).draw();
    /// dev.par().fig([0.5, 0.95, 0.5, 0.95]).new(true).draw();
    /// dev.plot().x([4, 3, 2, 1]).draw();
    /// ```
    ///
    /// # Errors
    ///
    /// `.try_draw()` returns R's error, and `.draw()` panics with it, unless
    /// `0 <= x0 < x1 <= 1` and `0 <= y0 < y1 <= 1`.
    ///
    /// [`par(cex)`]: crate::par::Par::cex
    /// [`par(oma)`]: crate::par::Par::oma
    pub fn fig(mut self, fig: [impl Num; 4]) -> Self {
        self.page.fig = Some(fig.map(Num::to_f64));
        self
    }

    /// Gives the next figure a fixed size, `[width, height]` in inches,
    /// centred in the inner region; otherwise as [`fig`](ParCall::fig).
    ///
    /// This is R's `par(fin = c(width, height))`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// # let mut dev = Device::null().open().unwrap();
    /// dev.par().fin([4, 3]).draw();
    /// dev.plot().x([1, 2, 3]).draw();
    /// ```
    ///
    /// # Errors
    ///
    /// `.try_draw()` returns R's error, and `.draw()` panics with it, unless
    /// both sizes are positive.
    pub fn fin(mut self, fin: impl Pair) -> Self {
        let (w, h) = fin.into_pair();
        self.page.fin = Some([w, h]);
        self
    }
}
