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
//! Each method starts a call for one R function; the call's chainable
//! setters take all of R's arguments, named as in R (data too: `.x(&x)`),
//! and `.draw()` runs it. An argument R requires (one without a default)
//! that is left out panics at `.draw()` with R's message (`argument "x" is
//! missing, with no default`), also when a `Figure` records the call. As in R, numbers may be integers or floats: data
//! vectors are any [`Numbers`], by reference or by value (`&x`, `x`,
//! `[1, 2, 3]`, `1..=10` for R's `1:10`, or one number, `1`, as R's
//! length-one vector; `f64` data isn't copied, whether
//! borrowed or a `Vec` moved in), and numeric setters
//! take integers too (`.cex(2)`, `.xlim(0, 10)`):
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
//! # let mut dev = Device::new(SvgBackend::new(5.0, 5.0), Par::default());
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
//! let mut dev = Device::new(SvgBackend::new(5.0, 5.0), Par::default());
//! dev.plot_new();
//! dev.abline().h(0.5); // no `.draw()`: nothing drawn, and a warning
//! ```

use std::borrow::Cow;

use crate::backend::Backend;
use crate::device::{
    AblineOptions, ArrowsOptions, AxisOptions, BoxOptions, Device, LegendLayout, LegendOptions,
    LegendPos, MtextOptions, PlotWindowOptions, PolygonOptions, RectOptions, SegmentsOptions, Side,
    TextOptions, TitleOptions, XyOptions,
};
use crate::highlevel::{
    BarHeights, BarplotOptions, BoxStats, BoxplotOptions, HistOptions, HistResult, PieOptions,
    PlotOptions,
};
use crate::numbers::{Groups, Numbers};
use crate::par::ParOptions;
use crate::plotmath::{Label, Labels};

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

    /// `plot.new()`: moves to the next figure and sets up its figure and
    /// plot regions.
    ///
    /// - **Coordinates:** like R, it leaves linear user coordinates of
    ///   0..1 on both axes (as `plot_window().xlim(0, 1).ylim(0, 1)`:
    ///   -0.04..1.04 with the default `xaxs = yaxs = "r"`) and their
    ///   default ticks, so you can draw straight away. Call
    ///   [`plot_window`](Graphics::plot_window) next for other limits or
    ///   log axes.
    /// - **Which figure:** the next one in the order set by `par()`'s
    ///   [`mfrow`](ParCall::mfrow) / [`mfcol`](ParCall::mfcol),
    ///   [`Device::layout`] or `par()`'s [`mfg`](ParCall::mfg) (one figure
    ///   per page by default). After the last figure, it wraps to figure 1
    ///   on a new page. The first call always starts page one. After
    ///   `par()`'s [`fig`](ParCall::fig) or [`fin`](ParCall::fin), the next
    ///   call starts a new page unless `new(true)` is set too.
    /// - **`par(new = TRUE)`:** after `dev.par().new(true).draw()` it stays
    ///   in the current figure and draws over it. The flag is not cleared
    ///   here; any drawing clears it, as in R.
    /// - **Regions:** figure and plot regions are computed now from
    ///   `par.mar`/`oma` (in lines of [`Device::line_height_inches`], so
    ///   `par.cex`, `mex` and the starting `ps` count too), `par.pty` and
    ///   `par.plt`. Changing these afterwards has no effect until the
    ///   next call, also on margin text (`title()`, `mtext()`,
    ///   `axis(outer = )`), which R would place with the new `mar`/`oma`.
    /// - **New page:** what happens depends on the backend. PDF starts
    ///   a new page in the file, or with
    ///   [`onefile(false)`](crate::device::DeviceBuilder::onefile) writes
    ///   the finished page to its own file as SVG and PNG do; SVG and PNG
    ///   start an empty page, after writing the finished one to its own
    ///   file if the file name has a page number (`"plot%03d.png"`; see
    ///   [`DeviceBuilder::open`](crate::device::DeviceBuilder::open)), or
    ///   else discarding it. The page is then filled with `par.bg`, unless
    ///   that is fully transparent, which it is by default on PDF (as R's
    ///   `pdf()`).
    ///
    /// # Panics
    /// With R's message when the regions don't fit: "figure margins too
    /// large" (the margins leave no room for the plot region, e.g. on a
    /// small page, with many panels or with large `oma`), "figure region
    /// too large" (`fin()` or `layout()` cm sizes beyond the inner region),
    /// "plot region too large" (`par(pin)` or `par(plt)` beyond the
    /// figure). Every high-level plot starts with `plot_new()`, so they
    /// panic the same way (on a [`Figure`](crate::recording::Figure), when
    /// it is added to a device). As R's `plot.new()`, it first moves back
    /// to the previous figure, so after fixing the cause (if the panic is
    /// caught) the next call retries the same figure.
    fn plot_new(&mut self);

    // ---- high-level plots ----

    /// `plot(x, y, ...)`: `plot.new()` + `plot.window()` + the requested
    /// point/line geometry + (optionally) `axis()`/`box()`/`title()`. R's
    /// `x` and `y` are [`x`](PlotCall::x) (required) and
    /// [`y`](PlotCall::y); with `x` alone, its values are plotted against
    /// their index, with x axis title "Index" (R's `plot(v)`). Other
    /// arguments in [`PlotOptions`]. A high-level plot: it starts a new
    /// plot with [`plot_new`](Graphics::plot_new), so on a multi-panel page
    /// it moves to the next panel.
    ///
    /// # Panics
    /// At `.draw()`, if `x` is missing (R's `argument "x" is missing, with
    /// no default`) or `x` and `y` have different lengths (R's
    /// `xy.coords()`: "'x' and 'y' lengths differ").
    fn plot(&mut self) -> PlotCall<'_, Self> {
        PlotCall::start(self)
    }

    /// `hist(x, ...)`: R's `x` is [`x`](HistCall::x) (required); other
    /// arguments in [`HistOptions`]. A high-level plot: it
    /// starts a new plot with [`plot_new`](Graphics::plot_new), so on a
    /// multi-panel page it moves to the next panel, unless `add` (R's
    /// `add = TRUE`), which draws the bars on the current plot. `.draw()`
    /// returns the bins and counts (R's `"histogram"` object).
    ///
    /// # Panics
    /// At `.draw()`, if `x` is missing (R's message).
    fn hist(&mut self) -> HistCall<'_, Self> {
        HistCall::start(self)
    }

    /// `barplot(height, ...)`: R's `height` is
    /// [`height`](BarplotCall::height) (required); other arguments in
    /// [`BarplotOptions`]. A high-level
    /// plot: it starts a new plot with [`plot_new`](Graphics::plot_new), so
    /// on a multi-panel page it moves to the next panel, unless `add` (R's
    /// `add = TRUE`), which draws the bars on the current plot. `height` is
    /// a vector (one bar per value) or a matrix given as its columns (see
    /// [`BarHeights`]), drawn stacked unless `beside`.
    ///
    /// `.draw()` returns the bar-centre coordinates along the category
    /// axis (R's `w.m`, the value `barplot()` returns invisibly), useful
    /// for overlaying annotations. For a matrix they are flattened
    /// column-major (for `beside`: one centre per bar, `nrow` per group;
    /// otherwise one per column).
    ///
    /// # Panics
    /// At `.draw()`, as R errors: if `height` is missing or empty, or a matrix's
    /// columns are empty or of different lengths; if `names` has neither
    /// one name per bar nor (for `beside`) one per column; and, with a log
    /// value axis, if any `height + offset`, or a given limit of a log
    /// axis, is <= 0.
    fn barplot(&mut self) -> BarplotCall<'_, Self> {
        BarplotCall::start(self)
    }

    /// `boxplot(x, ...)`: R's `x` is [`x`](BoxplotCall::x) (required), the
    /// groups (R's `list`/formula input reduced to its common case): any
    /// [`Groups`], e.g. `&[a, b]` or `&groups` for `Vec`s of numbers, `f64`
    /// or integer; one vector is one group, as R's `boxplot(x)` (`.x(&v)`). Other arguments in
    /// [`BoxplotOptions`]. A high-level plot:
    /// it starts a new plot with [`plot_new`](Graphics::plot_new), so on a
    /// multi-panel page it moves to the next panel, unless `add` (R's
    /// `add = TRUE`), which draws the boxes on the current plot. `.draw()`
    /// returns each group's statistics.
    ///
    /// # Panics
    /// At `.draw()`, if `x` is missing or has no groups, or `at` or `width`
    /// doesn't have one value per
    /// group (or a width isn't positive), as R errors.
    fn boxplot(&mut self) -> BoxplotCall<'_, Self> {
        BoxplotCall::start(self)
    }

    /// `pie(x, ...)`: R's `x` is [`x`](PieCall::x) (required), the slice
    /// sizes; other arguments in [`PieOptions`]. A high-level plot: it
    /// starts a new plot with [`plot_new`](Graphics::plot_new), so on a
    /// multi-panel page it moves to the next panel.
    ///
    /// # Panics
    /// At `.draw()`, if `x` is missing or empty or has a negative or non-finite value
    /// (R: "'x' values must be positive").
    fn pie(&mut self) -> PieCall<'_, Self> {
        PieCall::start(self)
    }

    // ---- low-level functions ----

    /// `plot.window(xlim, ylim, log, asp)`: sets up the user coordinates
    /// of the current plot region. R's `xlim` and `ylim` are
    /// [`xlim`](PlotWindowCall::xlim) and [`ylim`](PlotWindowCall::ylim)
    /// (both required); other arguments in [`PlotWindowOptions`]. Call it
    /// after [`plot_new`](Graphics::plot_new) and before drawing.
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
    ///
    /// # Panics
    /// At `.draw()`, if `xlim` or `ylim` is missing (R's message).
    fn plot_window(&mut self) -> PlotWindowCall<'_, Self> {
        PlotWindowCall::start(self)
    }

    /// `points(x, y, ...)`: R's `plot.xy(type = "p")` unless `type_` says
    /// otherwise. R's `x` and `y` are [`x`](PointsCall::x) (required) and
    /// [`y`](PointsCall::y) (`x` alone: against its index); other arguments
    /// in [`XyOptions`]. Points with a non-finite
    /// coordinate (including values <= 0 on a log axis) are skipped.
    ///
    /// # Panics
    /// At `.draw()`, if `x` is missing (R's message) or `x` and `y` have
    /// different lengths (R's `xy.coords()`: "'x' and 'y' lengths differ").
    fn points(&mut self) -> PointsCall<'_, Self> {
        PointsCall::start(self)
    }

    /// `lines(x, y, ...)`: R's `plot.xy(type = "l")` unless `type_` says
    /// otherwise. R's `x` and `y` are [`x`](LinesCall::x) (required) and
    /// [`y`](LinesCall::y) (`x` alone: against its index); other arguments
    /// in [`XyOptions`]. The polyline breaks at
    /// non-finite coordinates (R's `NA`, or values <= 0 on a log axis).
    ///
    /// # Panics
    /// At `.draw()`, if `x` is missing (R's message) or `x` and `y` have
    /// different lengths (R's `xy.coords()`: "'x' and 'y' lengths differ").
    fn lines(&mut self) -> LinesCall<'_, Self> {
        LinesCall::start(self)
    }

    /// `segments(x0, y0, x1, y1, ...)` in data units: R's coordinates are
    /// [`x0`](SegmentsCall::x0) and [`y0`](SegmentsCall::y0) (required),
    /// [`x1`](SegmentsCall::x1) and [`y1`](SegmentsCall::y1) (default `x0`
    /// and `y0`); other arguments in [`SegmentsOptions`]. The four
    /// coordinate slices and the styles are recycled to the longest; a
    /// segment with a non-finite end after conversion (including values <=
    /// 0 on a log axis) is skipped, as `C_segments`.
    ///
    /// # Panics
    /// At `.draw()`, if `x0` or `y0` is missing (R's message).
    fn segments(&mut self) -> SegmentsCall<'_, Self> {
        SegmentsCall::start(self)
    }

    /// `arrows(x0, y0, x1, y1, ...)`, a port of `C_arrows` / `GArrow()`,
    /// with the coordinates as for [`segments`](Graphics::segments)
    /// ([`x0`](ArrowsCall::x0) and [`y0`](ArrowsCall::y0) required) and
    /// other arguments in [`ArrowsOptions`]: each segment with open heads - two
    /// lines of `length` inches at `angle` degrees to the shaft - at its
    /// start, end or both (`code`). Coordinates and styles are recycled
    /// to the longest. As in R, an arrow with a non-finite end after
    /// conversion (including values <= 0 on a log axis) is skipped, and
    /// the heads of a segment shorter than 1/1000 inch are not drawn (R
    /// also warns).
    ///
    /// # Panics
    /// At `.draw()`, if `x0` or `y0` is missing (R's message).
    fn arrows(&mut self) -> ArrowsCall<'_, Self> {
        ArrowsCall::start(self)
    }

    /// `rect(xleft, ybottom, xright, ytop, ...)` in data units: the four
    /// coordinates are setters named as R's (all required); other arguments
    /// in [`RectOptions`]. Coordinates and styles are recycled to the
    /// longest. Without `density`, rectangles with a non-finite corner
    /// (e.g. at 0 on a log axis) are skipped, as in R. With `density`, the
    /// rectangles are shaded with lines (R's hatching) and drawn as
    /// polygons, as R does (so a non-finite corner splits the outline).
    ///
    /// # Panics
    /// At `.draw()`, if a coordinate is missing (R's message).
    fn rect(&mut self) -> RectCall<'_, Self> {
        RectCall::start(self)
    }

    /// `polygon(x, y, ...)`: R's `x` and `y` are [`x`](PolygonCall::x)
    /// (required) and [`y`](PolygonCall::y); other arguments in
    /// [`PolygonOptions`]. One polygon
    /// per run of at least two finite points (non-finite coordinates, R's
    /// `NA`, separate them), with `col`, `border` and `lty` recycled over
    /// the polygons drawn, as `C_polygon` (a shorter run is skipped and
    /// uses up no colour). With `density`, each polygon is shaded with
    /// lines (R's hatching; see [`PolygonOptions::density`]), or filled
    /// where its density is `None` or negative, with `col`, `lty`,
    /// `density` and `angle` recycled over every run of at least one point,
    /// as `polygon.R`; then all borders are drawn.
    ///
    /// # Panics
    /// At `.draw()`, if `x` is missing (R's message) or `x` and `y` have
    /// different lengths (R's `xy.coords()`: "'x' and 'y' lengths differ").
    fn polygon(&mut self) -> PolygonCall<'_, Self> {
        PolygonCall::start(self)
    }

    /// `text(x, y, labels, ...)` in data units: R's `x`, `y` and `labels`
    /// are [`x`](TextCall::x) (required), [`y`](TextCall::y) (`x` alone:
    /// against its index) and [`labels`](TextCall::labels); other arguments
    /// in [`TextOptions`]. Each label (text, or a plotmath expression such
    /// as `Label::math("alpha^2")`) goes at its point, with `x`/`y`
    /// recycled to the longer and labels recycled over the points (extra
    /// labels are ignored; R truncates them with a warning). Without
    /// `labels` (or with none), they are R's default, the indices `"1"`,
    /// `"2"`, .... Placement follows
    /// `GEText()`/`GEMathText()`: horizontally `adj` of the width;
    /// vertically see [`TextOptions::vadj`]; rotated by `srt`, else
    /// `par(srt)`. A label at a non-finite position (R's `NA`, or a value
    /// <= 0 on a log axis) is skipped, as in R.
    ///
    /// # Panics
    /// At `.draw()`, if `x` is missing (R's message).
    fn text(&mut self) -> TextCall<'_, Self> {
        TextCall::start(self)
    }

    /// `box(which, ...)`: outlines the plot region, figure, inner region
    /// or the whole device; options in [`BoxOptions`]. Not clipped. For
    /// the plot region, `bty` (or `par(bty)`) picks the sides, as R's
    /// `GBox()`: `"o"` all four, `"l"`, `"7"`, `"c"`, `"u"`, `"]"`, or
    /// `"n"` none.
    fn box_(&mut self) -> BoxCall<'_, Self> {
        BoxCall::start(self)
    }

    /// `axis(side, ...)` with all of R's arguments: R's `side` is
    /// [`side`](AxisCall::side) (required), a [`Side`] or R's number for it
    /// (1 to 4); the others are in [`AxisOptions`].
    ///
    /// On a [`Device`], `.draw()` returns R's value: the tick positions
    /// (`at`, or the default ticks) in data units, sorted, with `NaN`
    /// (R's `NA`) last, including ticks outside the plot that weren't
    /// drawn; empty (R's `NULL`) when `at` is empty or the axis type is
    /// `"n"`. So `let at = dev.axis().side(2).draw();` then
    /// `dev.abline().h(at).col("grey").draw();` puts grid lines at the
    /// ticks, as R's `abline(h = axis(2))`. On a
    /// [`Figure`](crate::recording::Figure) it returns `()`; the positions
    /// come from [`Device::add_figure`].
    ///
    /// # Panics
    /// If `side` is a number other than 1 to 4 (in the setter), or, at
    /// `.draw()`, if `side` is missing (R's message), or `labels` is given
    /// without `at` or their lengths differ (R's errors).
    fn axis(&mut self) -> AxisCall<'_, Self> {
        AxisCall::start(self)
    }

    /// `abline(a, b, h, v, untf, col, lty, lwd, xpd)`; options in
    /// [`AblineOptions`]: lines `y = a + b * x` (`.a(1).b(2)`; unlike R,
    /// several: `.a([1, 2]).b([2, 3])`), horizontal (`.h(..)`) and vertical
    /// (`.v(..)`) lines, with `col`, `lty` and `lwd` recycled over all of
    /// them in that order. For R's `abline(coef = )` or `abline(reg = )`,
    /// pass the intercept and slope as `a` and `b`.
    ///
    /// # Panics
    /// At `.draw()`, if only one of `a` and `b` is given (R: "invalid
    /// a=, b= specification"; R itself ignores a lone `b`), if both have
    /// several values but not as many, or if a value isn't finite (R: "'a'
    /// and 'b' must be finite").
    fn abline(&mut self) -> AblineCall<'_, Self> {
        AblineCall::start(self)
    }

    /// `legend(x, y, legend, ...)`, a port of R's `legend()`: one key per
    /// label ([`labels`](LegendCall::labels), R's `legend`: text or
    /// plotmath expressions; one or several, see [`Labels`]), placed by
    /// [`pos`](LegendCall::pos) (R's `x` and `y`), both required; R's
    /// other arguments in
    /// [`LegendOptions`], whose per-key ones (`col`, `pch`, `lty`, `lwd`,
    /// `fill`, ...) are recycled over the keys, as in R. On a [`Device`],
    /// `.draw()` returns where the box and labels went (R's return value,
    /// in `usr` units). With `.plot(false)` (R's `plot = FALSE`) it only
    /// works that out, drawing nothing, so a legend can be measured first.
    ///
    /// `pos` is written as in R (see [`LegendPos`]): a keyword such as
    /// `"topright"`, a point `(x, y)`, or two corners
    /// `((xa, xb), (ya, yb))`:
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// # let mut dev = Device::new(SvgBackend::new(5.0, 5.0), Par::default());
    /// # dev.plot().x(&[1.0, 10.0]).y(&[1.0, 30.0]).draw();
    /// // legend("topright", legend = c("a", "b"), pch = 19, col = 1:2)
    /// dev.legend().pos("topright").labels(["a", "b"]).pch(19).col([1, 2]).draw();
    /// // legend(5, 20, legend = "fit", lty = 2, lwd = 2)
    /// dev.legend().pos((5.0, 20.0)).labels("fit").lty(2).lwd(2).draw();
    /// ```
    ///
    /// # Panics
    /// If `pos` is a string that isn't one of R's nine keywords (or a
    /// unique prefix of one), and at `.draw()` with R's errors: `pos` or
    /// `labels` missing, no labels ("'legend' is of length 0"), or a
    /// `text_width` that is negative or has `NaN` among several values.
    fn legend(&mut self) -> LegendCall<'_, Self> {
        LegendCall::start(self)
    }

    /// `mtext(text, side, line, outer, at, adj, padj, cex, col, font, las,
    /// xpd)`, with rotation from `las` (inline, else `par(las)`); a `None`
    /// text (R's `NA`) is skipped. R's `text` and `side` are
    /// [`text`](MtextCall::text) (required) and [`side`](MtextCall::side)
    /// (default 3, the top); the others are in [`MtextOptions`]. `text` is
    /// text (placed by `GMtext()`) or a plotmath expression
    /// (`Label::math(...)`, placed by `GMMathText()`: its bounding box
    /// aligned by `adj`/`padj`, in the plain font).
    ///
    /// # Panics
    /// If `side` is a number other than 1 to 4 (in the setter), or, at
    /// `.draw()`, if `text` is missing (R's message).
    fn mtext(&mut self) -> MtextCall<'_, Self> {
        MtextCall::start(self)
    }

    /// `title(main=, sub=, xlab=, ylab=)`; options in [`TitleOptions`],
    /// each text or a plotmath expression, drawn in `C_title`'s order
    /// (main, sub, xlab, ylab) with `cex.main`/`col.main`/`font.main` and
    /// the `sub` and `lab` ones: inline (`.cex_main(2)`), else `par()`'s.
    /// Text `sub`, `xlab` and `ylab` are placed by `GMtext()`
    /// (baseline on the line, nudged by `par(ylbias)`); text `main` is
    /// centred on line `0.5 * mar[3]` (its baseline half the height of "M"
    /// below it, no `yLineBias`). Expressions are placed by `GMathText()`
    /// (`main`: box centred on its line) and `GMMathText()` (the others:
    /// box bottom on the nudged line).
    fn title(&mut self) -> TitleCall<'_, Self> {
        TitleCall::start(self)
    }

    /// `par(...)`: changes graphical parameters; options in
    /// [`ParOptions`] (each a [`Par`](crate::par::Par) field, plus `new`,
    /// `usr`, `xlog` and `ylog`, which are device state). It is the one way
    /// to change them, on a
    /// [`Device`] and in a recorded [`Figure`](crate::recording::Figure);
    /// read the current values with [`Device::current_par`]. As in R,
    /// setting one unit of a region clears the others (`mar` clears `mai`,
    /// `pin` and `plt`, `oma` clears `omi` and `omd`, ...), setting
    /// `oma`/`omi`/`omd` makes the next plot start a new page, and `mar`,
    /// `oma`, `pty` and `plt` take effect at the next
    /// [`plot_new`](Graphics::plot_new).
    ///
    /// On a `Device`, the call also takes R's page layout parameters
    /// [`mfrow`](ParCall::mfrow), [`mfcol`](ParCall::mfcol),
    /// [`mfg`](ParCall::mfg), [`fig`](ParCall::fig) and
    /// [`fin`](ParCall::fin), as in R's `par(mfrow = c(1, 2), oma = c(0,
    /// 0, 2, 0))`:
    ///
    /// ```no_run
    /// # use rs_graphics::prelude::*;
    /// let mut dev = Device::pdf("plot.pdf").width(8.0).height(4.0).open()?;
    /// dev.par().mfrow(1, 2).oma([0.0, 0.0, 2.0, 0.0]).draw();
    /// # Ok::<(), std::io::Error>(())
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
    ///
    /// # Panics
    /// At `.draw()` (also when a `Figure` records the call), if `usr` has
    /// a `NaN` or `x0 == x1` or `y0 == y1` (R's "invalid value specified
    /// for graphical parameter \"usr\""). Some page layout values are
    /// checked on a device only (see [`mfrow`](ParCall::mfrow) and the
    /// other layout setters).
    fn par(&mut self) -> ParCall<'_, Self> {
        ParCall::start(self)
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
    fn emit_text(&mut self, x: &[f64], y: &[f64], labels: Vec<Option<Label>>, opts: TextOptions);
    #[doc(hidden)]
    fn emit_box(&mut self, opts: BoxOptions);
    #[doc(hidden)]
    fn emit_axis(&mut self, side: Side, opts: AxisOptions) -> Self::AxisResult;
    #[doc(hidden)]
    fn emit_abline(&mut self, opts: AblineOptions);
    #[doc(hidden)]
    fn emit_legend(
        &mut self,
        pos: LegendPos,
        labels: Vec<Option<Label>>,
        opts: LegendOptions,
    ) -> Self::LegendResult;
    #[doc(hidden)]
    fn emit_mtext(&mut self, text: Vec<Option<Label>>, side: Side, opts: MtextOptions);
    #[doc(hidden)]
    fn emit_title(&mut self, opts: TitleOptions);
    #[doc(hidden)]
    fn emit_par(&mut self, opts: ParOptions, page: ParPage);
}

impl<B: Backend> Graphics for Device<B> {
    type LegendResult = LegendLayout;
    type AxisResult = Vec<f64>;

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
    fn emit_text(&mut self, x: &[f64], y: &[f64], labels: Vec<Option<Label>>, opts: TextOptions) {
        self.text_impl(x, y, &labels, &opts)
    }
    fn emit_box(&mut self, opts: BoxOptions) {
        self.box_impl(&opts)
    }
    fn emit_axis(&mut self, side: Side, opts: AxisOptions) -> Vec<f64> {
        self.axis_impl(side, &opts)
    }
    fn emit_abline(&mut self, opts: AblineOptions) {
        self.abline_impl(&opts)
    }
    fn emit_legend(
        &mut self,
        pos: LegendPos,
        labels: Vec<Option<Label>>,
        opts: LegendOptions,
    ) -> LegendLayout {
        self.legend_impl(pos, &labels, &opts)
    }
    fn emit_mtext(&mut self, text: Vec<Option<Label>>, side: Side, opts: MtextOptions) {
        self.mtext_impl(&text, side, &opts)
    }
    fn emit_title(&mut self, opts: TitleOptions) {
        self.title_impl(&opts)
    }
    fn emit_par(&mut self, opts: ParOptions, page: ParPage) {
        // The page layout first: a grid resets `cex` and `mex`, so values
        // set in the same call win; `fig`/`fin` replace the grid, and `mfg`
        // picks a panel of it.
        match page.grid {
            Some(ParGrid::Rows(nrow, ncol)) => self.mfrow(nrow, ncol),
            Some(ParGrid::Cols(nrow, ncol)) => self.mfcol(nrow, ncol),
            None => {}
        }
        if let Some(fig) = page.fig {
            self.fig(fig);
        }
        if let Some(fin) = page.fin {
            self.fin(fin);
        }
        if let Some((row, col)) = page.mfg {
            self.mfg(row, col);
        }
        opts.apply(&mut self.par);
        // R's `par(oma/omi/omd = )`: "force eject of multiple figures",
        // so the next `plot_new()` starts a new page.
        if opts.sets_outer_margins() {
            self.eject_page();
        }
        if let Some(new) = opts.new {
            self.set_new(new);
        }
        self.set_log(opts.xlog, opts.ylog);
        if let Some(usr) = opts.usr {
            self.set_usr(usr);
        }
    }
}

/// R's `xy.coords()` for `plot()`, `points()`, `lines()`, `polygon()` and
/// `text()`: `x` is required; without `y`, `x` is plotted against its
/// index `1, 2, ...` (R's one-vector form, `plot(v)`), and the returned
/// flag says so. Unless `recycle` (`text()`: R's `xy.coords(recycle =
/// TRUE)`), `x` and `y` must have one length.
fn xy_coords<'a>(
    x: Option<Cow<'a, [f64]>>,
    y: Option<Cow<'a, [f64]>>,
    recycle: bool,
) -> (Cow<'a, [f64]>, Cow<'a, [f64]>, bool) {
    let x = required(x, "x");
    match y {
        Some(y) => {
            assert!(recycle || x.len() == y.len(), "'x' and 'y' lengths differ");
            (x, y, false)
        }
        None => {
            let index: Vec<f64> = (1..=x.len()).map(|i| i as f64).collect();
            (index.into(), x, true)
        }
    }
}

/// An R argument without a default: its value, or R's error.
fn required<T>(value: Option<T>, name: &str) -> T {
    value.unwrap_or_else(|| panic!("argument \"{name}\" is missing, with no default"))
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
    (@setter [$(#[$doc:meta])*] $field:ident ($aty:ty => $conv:expr)) => {
        $(#[$doc])*
        pub fn $field(mut self, value: $aty) -> Self {
            self.$field = Some(($conv)(value));
            self
        }
    };
    (
        $(#[$doc:meta])*
        $name:ident, $opts:ty => $ret:ty,
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

            /// Runs the call with the arguments and options set so far.
            pub fn draw(self) -> $ret {
                #[allow(unused_variables)]
                let $name { target: $t, $($field,)* $($($sfield,)*)? opts: $o } = self;
                $body
            }
        }
    };
}

call!(
    /// [`Graphics::plot`]'s call.
    PlotCall, PlotOptions => (),
    args {
        /// R's `x`: the x coordinates (required), or, without
        /// [`y`](PlotCall::y), the values to plot against their index (R's
        /// `plot(v)`, with x axis title "Index").
        x: Cow<'a, [f64]> = (impl Numbers<'a> => Numbers::into_numbers),
        /// R's `y`: the y coordinates, as many as `x`.
        y: Cow<'a, [f64]> = (impl Numbers<'a> => Numbers::into_numbers),
    },
    |t, o| {
        let (x, y, index) = xy_coords(x, y, false);
        let mut o = o;
        if index && o.xlab.is_none() {
            o.xlab = Some("Index".into());
        }
        t.emit_plot(&x, &y, o)
    }
);
call!(
    /// [`Graphics::hist`]'s call.
    HistCall, HistOptions => HistResult,
    args {
        /// R's `x`: the values (required).
        x: Cow<'a, [f64]> = (impl Numbers<'a> => Numbers::into_numbers),
    },
    |t, o| { t.emit_hist(&required(x, "x"), o) }
);
call!(
    /// [`Graphics::barplot`]'s call.
    BarplotCall, BarplotOptions => Vec<f64>,
    args {
        /// R's `height` (required): a vector, one bar per value, or a
        /// matrix given as its columns (see [`BarHeights`]).
        height: BarHeights = (impl Into<BarHeights> => Into::into),
    },
    |t, o| { t.emit_barplot(required(height, "height"), o) }
);
call!(
    /// [`Graphics::boxplot`]'s call.
    BoxplotCall, BoxplotOptions => Vec<BoxStats>,
    args {
        /// R's `x` (required): the groups, any [`Groups`] (e.g. `&[a, b]`
        /// or `&groups`; one vector, `&x`, is one group).
        x: Cow<'a, [Vec<f64>]> = (impl Groups<'a> => Groups::into_groups),
    },
    |t, o| { t.emit_boxplot(&required(x, "x"), o) }
);
call!(
    /// [`Graphics::pie`]'s call.
    PieCall, PieOptions => (),
    args {
        /// R's `x`: the slice sizes (required).
        x: Cow<'a, [f64]> = (impl Numbers<'a> => Numbers::into_numbers),
    },
    |t, o| { t.emit_pie(&required(x, "x"), o) }
);
call!(
    /// [`Graphics::plot_window`]'s call.
    PlotWindowCall, PlotWindowOptions => (),
    args { xlim: (f64, f64) = (manual), ylim: (f64, f64) = (manual) },
    |t, o| { t.emit_plot_window(required(xlim, "xlim"), required(ylim, "ylim"), o) }
);

impl<G: Graphics> PlotWindowCall<'_, G> {
    /// R's `xlim` (required): the x limits in data units, also on a log
    /// axis; decreasing for a reversed axis.
    pub fn xlim(mut self, from: impl Into<f64>, to: impl Into<f64>) -> Self {
        self.xlim = Some((from.into(), to.into()));
        self
    }

    /// R's `ylim` (required): as [`xlim`](PlotWindowCall::xlim).
    pub fn ylim(mut self, from: impl Into<f64>, to: impl Into<f64>) -> Self {
        self.ylim = Some((from.into(), to.into()));
        self
    }
}

call!(
    /// [`Graphics::points`]'s call.
    PointsCall, XyOptions => (),
    args {
        /// R's `x`: the x coordinates (required), or, without
        /// [`y`](PointsCall::y), the values to plot against their index.
        x: Cow<'a, [f64]> = (impl Numbers<'a> => Numbers::into_numbers),
        /// R's `y`: the y coordinates, as many as `x`.
        y: Cow<'a, [f64]> = (impl Numbers<'a> => Numbers::into_numbers),
    },
    |t, o| {
        let (x, y, _) = xy_coords(x, y, false);
        t.emit_points(&x, &y, o)
    }
);
call!(
    /// [`Graphics::lines`]'s call.
    LinesCall, XyOptions => (),
    args {
        /// R's `x`: the x coordinates (required), or, without
        /// [`y`](LinesCall::y), the values to draw against their index.
        x: Cow<'a, [f64]> = (impl Numbers<'a> => Numbers::into_numbers),
        /// R's `y`: the y coordinates, as many as `x`.
        y: Cow<'a, [f64]> = (impl Numbers<'a> => Numbers::into_numbers),
    },
    |t, o| {
        let (x, y, _) = xy_coords(x, y, false);
        t.emit_lines(&x, &y, o)
    }
);
call!(
    /// [`Graphics::segments`]'s call.
    SegmentsCall, SegmentsOptions => (),
    args {
        /// R's `x0`: the x coordinates of the starts (required).
        x0: Cow<'a, [f64]> = (impl Numbers<'a> => Numbers::into_numbers),
        /// R's `y0`: the y coordinates of the starts (required).
        y0: Cow<'a, [f64]> = (impl Numbers<'a> => Numbers::into_numbers),
        /// R's `x1`: the x coordinates of the ends; default `x0`.
        x1: Cow<'a, [f64]> = (impl Numbers<'a> => Numbers::into_numbers),
        /// R's `y1`: the y coordinates of the ends; default `y0`.
        y1: Cow<'a, [f64]> = (impl Numbers<'a> => Numbers::into_numbers),
    },
    |t, o| {
        let [x0, y0, x1, y1] = segment_ends(x0, y0, x1, y1);
        t.emit_segments(&x0, &y0, &x1, &y1, o)
    }
);
call!(
    /// [`Graphics::arrows`]'s call.
    ArrowsCall, ArrowsOptions => (),
    args {
        /// R's `x0`: the x coordinates of the starts (required).
        x0: Cow<'a, [f64]> = (impl Numbers<'a> => Numbers::into_numbers),
        /// R's `y0`: the y coordinates of the starts (required).
        y0: Cow<'a, [f64]> = (impl Numbers<'a> => Numbers::into_numbers),
        /// R's `x1`: the x coordinates of the ends; default `x0`.
        x1: Cow<'a, [f64]> = (impl Numbers<'a> => Numbers::into_numbers),
        /// R's `y1`: the y coordinates of the ends; default `y0`.
        y1: Cow<'a, [f64]> = (impl Numbers<'a> => Numbers::into_numbers),
    },
    |t, o| {
        let [x0, y0, x1, y1] = segment_ends(x0, y0, x1, y1);
        t.emit_arrows(&x0, &y0, &x1, &y1, o)
    }
);
call!(
    /// [`Graphics::rect`]'s call.
    RectCall, RectOptions => (),
    args {
        /// R's `xleft` (required).
        xleft: Cow<'a, [f64]> = (impl Numbers<'a> => Numbers::into_numbers),
        /// R's `ybottom` (required).
        ybottom: Cow<'a, [f64]> = (impl Numbers<'a> => Numbers::into_numbers),
        /// R's `xright` (required).
        xright: Cow<'a, [f64]> = (impl Numbers<'a> => Numbers::into_numbers),
        /// R's `ytop` (required).
        ytop: Cow<'a, [f64]> = (impl Numbers<'a> => Numbers::into_numbers),
    },
    |t, o| {
        t.emit_rect(
            &required(xleft, "xleft"),
            &required(ybottom, "ybottom"),
            &required(xright, "xright"),
            &required(ytop, "ytop"),
            o,
        )
    }
);
call!(
    /// [`Graphics::polygon`]'s call.
    PolygonCall, PolygonOptions => (),
    args {
        /// R's `x`: the x coordinates of the vertices (required), or,
        /// without [`y`](PolygonCall::y), the y values against their index.
        x: Cow<'a, [f64]> = (impl Numbers<'a> => Numbers::into_numbers),
        /// R's `y`: the y coordinates, as many as `x`.
        y: Cow<'a, [f64]> = (impl Numbers<'a> => Numbers::into_numbers),
    },
    |t, o| {
        let (x, y, _) = xy_coords(x, y, false);
        t.emit_polygon(&x, &y, o)
    }
);
call!(
    /// [`Graphics::text`]'s call.
    TextCall, TextOptions => (),
    args {
        /// R's `x`: the x coordinates (required), or, without
        /// [`y`](TextCall::y), the y values against their index.
        x: Cow<'a, [f64]> = (impl Numbers<'a> => Numbers::into_numbers),
        /// R's `y`: the y coordinates; `x` and `y` are recycled to the
        /// longer.
        y: Cow<'a, [f64]> = (impl Numbers<'a> => Numbers::into_numbers),
        /// R's `labels`: text or plotmath expressions, one or several (see
        /// [`Labels`]; a `None` element, R's `NA`, is skipped); default
        /// `"1"`, `"2"`, ....
        labels: Vec<Option<Label>> = (impl Labels => Labels::into_labels),
    },
    |t, o| {
        let (x, y, _) = xy_coords(x, y, true);
        t.emit_text(&x, &y, labels.unwrap_or_default(), o)
    }
);
call!(
    /// [`Graphics::box_`]'s call.
    BoxCall, BoxOptions => (),
    args {},
    |t, o| { t.emit_box(o) }
);
call!(
    /// [`Graphics::axis`]'s call.
    AxisCall, AxisOptions => G::AxisResult,
    args {
        /// R's `side` (required): a [`Side`] or R's number for it (1 to 4).
        ///
        /// # Panics
        /// If the number isn't 1 to 4.
        side: Side = (impl Into<Side> => Into::into),
    },
    |t, o| { t.emit_axis(required(side, "side"), o) }
);
call!(
    /// [`Graphics::abline`]'s call.
    AblineCall, AblineOptions => (),
    args {},
    |t, o| { t.emit_abline(o) }
);
call!(
    /// [`Graphics::legend`]'s call.
    LegendCall, LegendOptions => G::LegendResult,
    args {
        /// Where the legend goes (required; R's `x` and `y`): a keyword such
        /// as `"topright"`, a point `(x, y)`, or two corners `((xa, xb),
        /// (ya, yb))`; see [`LegendPos`].
        ///
        /// # Panics
        /// If a string isn't one of R's nine keywords (or a unique prefix
        /// of one).
        pos: LegendPos = (impl Into<LegendPos> => Into::into),
        /// The keys' labels (required; R's `legend`): text or plotmath
        /// expressions, one or several (see [`Labels`]; a `None` element,
        /// R's `NA`, has its key but no text, and takes no room).
        labels: Vec<Option<Label>> = (impl Labels => Labels::into_labels),
    },
    |t, o| { t.emit_legend(required(pos, "pos"), required(labels, "labels"), o) }
);
call!(
    /// [`Graphics::mtext`]'s call.
    MtextCall, MtextOptions => (),
    args {
        /// R's `text` (required): text or plotmath expressions, one or
        /// several (see [`Labels`]; a `None` element, R's `NA`, is
        /// skipped), recycled with the other arguments.
        text: Vec<Option<Label>> = (impl Labels => Labels::into_labels),
        /// R's `side`: a [`Side`] or R's number for it (1 to 4); default 3
        /// (top).
        ///
        /// # Panics
        /// If the number isn't 1 to 4.
        side: Side = (impl Into<Side> => Into::into),
    },
    |t, o| { t.emit_mtext(required(text, "text"), side.unwrap_or(Side::Top), o) }
);
call!(
    /// [`Graphics::title`]'s call.
    TitleCall, TitleOptions => (),
    args {},
    |t, o| { t.emit_title(o) }
);
call!(
    /// [`Graphics::par`]'s call.
    ParCall, ParOptions => (),
    args {},
    state { page: ParPage },
    |t, o| {
        o.check();
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
) -> [Cow<'a, [f64]>; 4] {
    let (x0, y0) = (required(x0, "x0"), required(y0, "y0"));
    let x1 = x1.unwrap_or_else(|| x0.clone());
    let y1 = y1.unwrap_or_else(|| y0.clone());
    [x0, y0, x1, y1]
}

/// A `par()` call's page layout arguments (`mfrow`/`mfcol`, `mfg`, `fig`,
/// `fin`), which only a [`Device`] takes.
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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ParGrid {
    Rows(usize, usize),
    Cols(usize, usize),
}

/// Page layout, which only a [`Device`] has: a [`Figure`] is one panel.
///
/// [`Figure`]: crate::recording::Figure
impl<B: Backend> ParCall<'_, Device<B>> {
    /// `par(mfrow = c(nrow, ncol))`: splits the page into a grid of
    /// panels, filled row by row. As in R, it also sets `par(cex)` to 0.83
    /// (2 x 2) or 0.66 (more than 2 rows or columns) and `mex` to 1, and
    /// the next `plot_new()` starts a new page at the first panel. It
    /// replaces `fig`/`fin` and [`Device::layout`].
    ///
    /// In one call the page layout is applied first, in the order grid,
    /// `fig`, `fin`, `mfg`, then the other settings, then `new`: so a `cex`
    /// or `mex` set in the same call wins (in R, whichever argument comes
    /// last wins). Replaces an earlier `mfrow`/`mfcol` in the call; kept by
    /// `.with()`.
    ///
    /// # Panics
    /// At `.draw()`, if `nrow` or `ncol` is zero.
    pub fn mfrow(mut self, nrow: usize, ncol: usize) -> Self {
        self.page.grid = Some(ParGrid::Rows(nrow, ncol));
        self
    }

    /// `par(mfcol = c(nrow, ncol))`: as [`mfrow`](ParCall::mfrow), but the
    /// panels are filled column by column.
    ///
    /// # Panics
    /// At `.draw()`, if `nrow` or `ncol` is zero.
    pub fn mfcol(mut self, nrow: usize, ncol: usize) -> Self {
        self.page.grid = Some(ParGrid::Cols(nrow, ncol));
        self
    }

    /// `par(mfg = c(row, col))`: makes the panel at (`row`, `col`)
    /// (1-based) the one the next `plot_new()` draws into, without
    /// advancing or starting a new page. As in R, the panel number is
    /// derived from the `mfrow`/`mfcol` ordering even under `layout()`,
    /// and it has no effect before anything has been plotted: the first
    /// `plot_new()` always starts at panel 1 (see
    /// [`add_figure_at`](Device::add_figure_at), which handles that).
    ///
    /// # Panics
    /// At `.draw()`, if `row`/`col` are outside the current arrangement.
    pub fn mfg(mut self, row: usize, col: usize) -> Self {
        self.page.mfg = Some((row, col));
        self
    }

    /// `par(fig = c(x0, x1, y0, y1))`: a single figure region, as fractions
    /// of the inner region (the device less `par(oma)`), for placing plots
    /// anywhere on the page. As in R, the next `plot_new()` starts a new
    /// page unless `new` is set (`.fig(..).new(true)` adds a plot to the
    /// current page), `par(cex)` is left as it is, and `mfrow`/`mfcol`/
    /// [`Device::layout`] replace it.
    ///
    /// # Panics
    /// At `.draw()`, unless `0 <= x0 < x1 <= 1` and `0 <= y0 < y1 <= 1`
    /// (R's error).
    pub fn fig(mut self, fig: [impl Into<f64>; 4]) -> Self {
        self.page.fig = Some(fig.map(Into::into));
        self
    }

    /// `par(fin = c(width, height))`: a single figure region of that size
    /// in inches, centred in the inner region. Otherwise as
    /// [`fig`](ParCall::fig).
    ///
    /// # Panics
    /// At `.draw()`, unless both sizes are positive (R's error).
    pub fn fin(mut self, fin: [impl Into<f64>; 2]) -> Self {
        self.page.fin = Some(fin.map(Into::into));
        self
    }
}
