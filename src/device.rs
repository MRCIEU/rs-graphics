// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! The graphics device, [`Device`]: the page (or pages) being drawn, the
//! graphical parameters (`par()`), the page layout, coordinates and
//! clipping, and the code behind every drawing function.
//!
//! Devices are opened for a file with [`Device::pdf`], [`Device::svg`],
//! [`Device::png`] or [`Device::format`] (a [`DeviceBuilder`]) and written
//! by [`Device::close`]; [`Device::memory`] keeps the output in memory
//! ([`Device::finish`]), [`Device::null`] draws nothing, and
//! [`Device::new`] wraps a backend of your own. Draw on a device with the
//! [`Graphics`] methods. The module also holds
//! the options structs of the low-level functions ([`XyOptions`],
//! [`AxisOptions`], [`LegendOptions`], ...) and the types they use
//! ([`Side`], [`LegendPos`], ...).

// For contributors: the public entry to each R function is a `Graphics`
// method; the drawing code is here, in crate-private `*_impl` methods
// taking the data and an options struct, whose unset fields are R's
// defaults, mostly the `par()` values. `points()`, `lines()`,
// `segments()`, `arrows()`, `rect()`, `polygon()`, `text()`, `box()` and
// `plot.window()` are in `primitives.rs`; `axis()`, `abline()`,
// `legend()` and `mtext()` in their own child modules; the page, layout,
// coordinate and clipping state, `plot.new()` and `title()` here. Nearly
// every high-level plot (`plot()`, `hist()`, `barplot()`, ...) is built
// purely out of these primitives, mirroring how R's own `graphics.c`
// centres everything around a handful of `GE*` engine calls.

use crate::afm::{self};
use crate::font::FontFace;
use std::ops::{Deref, DerefMut};

use crate::backend::{Backend, LineEnd, LineJoin, Stroke, TextStyle};
use crate::color::{Color, ColorSpec};
use crate::error::Error;
use crate::geometry::{Point, Rect};
use crate::graphics::Graphics;
use crate::layout::{Arrangement, Layout, LayoutRespect, LayoutSize};
use crate::lty::LineType;
use crate::numbers::Num;
use crate::par::{AxisStyle, Par, Pty, Sides, Xpd};
use crate::pch;
use crate::plotmath::Label;
use crate::pretty::{axis_pars, create_at_vector};
use crate::recycle::pick;
use crate::setters::IntoArg;

mod abline;
mod axis;
mod hatch;
mod legend;
mod mtext;
mod open;
mod plotmath;
mod primitives;
pub use abline::AblineOptions;
pub(crate) use axis::AxisLabelStyle;
pub use axis::AxisOptions;
pub(crate) use axis::check_axis;
pub(crate) use legend::check_legend;
pub use legend::{LegendKeyword, LegendLayout, LegendOptions, LegendPos};
pub use mtext::MtextOptions;
pub(crate) use mtext::check_mtext;
pub use open::{DeviceBuilder, Units};
pub(crate) use primitives::check_text;
pub use primitives::{
    ArrowsOptions, BoxOptions, BoxWhich, PlotWindowOptions, PolygonOptions, RectOptions,
    SegmentsOptions, TextOptions, XyOptions,
};

/// A side of the plot, figure or page.
///
/// Setters also take R's side numbers (`TryFrom<i32>`): `1` bottom, `2`
/// left, `3` top, `4` right (`.side(1)` is `.side(Side::Bottom)`); any
/// other number is an error of the call ("invalid axis number").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Side {
    /// Side 1, below the plot.
    Bottom = 1,
    /// Side 2, left of the plot.
    Left = 2,
    /// Side 3, above the plot.
    Top = 3,
    /// Side 4, right of the plot.
    Right = 4,
}

impl Side {
    /// All four sides, in R's order (1 to 4).
    pub const ALL: [Side; 4] = [Side::Bottom, Side::Left, Side::Top, Side::Right];

    /// R's number for the side (1 to 4).
    pub fn number(self) -> i32 {
        self as i32
    }

    /// Bottom or top: the sides that carry an x axis.
    pub fn is_horizontal(self) -> bool {
        matches!(self, Side::Bottom | Side::Top)
    }
}

impl TryFrom<i32> for Side {
    type Error = crate::error::Error;

    /// R's side numbers.
    ///
    /// # Errors
    ///
    /// If `n` isn't 1 to 4 (R: "invalid axis number").
    fn try_from(n: i32) -> Result<Side, Self::Error> {
        match n {
            1 => Ok(Side::Bottom),
            2 => Ok(Side::Left),
            3 => Ok(Side::Top),
            4 => Ok(Side::Right),
            _ => Err(crate::error::Error::invalid(format!(
                "invalid axis number {n}"
            ))),
        }
    }
}

/// A graphics device: one or more pages drawn onto a [`Backend`], with the
/// graphical parameters (`par()`) and the page layout.
///
/// Open one for a file with [`Device::pdf`], [`Device::svg`],
/// [`Device::png`] or [`Device::format`], in memory with
/// [`Device::memory`], or for nothing with [`Device::null`]
/// ([`Device::new`] wraps a backend you implement); draw on it with the
/// [`Graphics`] methods.
///
/// # Examples
///
/// ```no_run
/// # use rs_graphics::prelude::*;
/// # fn main() -> Result<(), rs_graphics::Error> {
/// let mut dev = Device::pdf("plot.pdf").width(6).height(4).open()?;
/// dev.plot().x([1, 2, 3]).y([2, 4, 3]).draw();
/// dev.close()?;
/// # Ok(())
/// # }
/// ```
pub struct Device<B: Backend> {
    backend: B,
    /// R's `par()` state: changed by
    /// [`par()`](crate::graphics::Graphics::par) calls, read with
    /// [`Device::current_par`]. The page layout (`mfrow`, `layout`,
    /// `fig`, ...) is device state, not part of `Par`.
    pub(crate) par: Par,
    width_in: f64,
    height_in: f64,
    /// Inner region (the device minus the outer margins), as of the last
    /// `plot_new()`.
    inner: Rect,
    /// [`par(mar)`] as of the last `plot_new()`, for `title()`.
    ///
    /// [`par(mar)`]: crate::par::Par::mar
    mar: Sides<f64>,
    /// Figure region (part of the inner region).
    fig: Rect,
    /// Plot region (figure region minus `mar` margins).
    plt: Rect,
    /// R's error for invalid regions as of the last layout (see
    /// `region_error()`), raised by `plot_new()`.
    region_error: Option<&'static str>,
    /// [`par(cex)`] as of the last layout (R's `cexbase` in `xNDCPerChar`,
    /// set by `mapNDC2Dev()` at `plot.new()` and region changes, not by a
    /// later `par(cex = )`): the size of R's `CHARS` unit, e.g. `text()`'s
    /// `offset`.
    ///
    /// [`par(cex)`]: crate::par::Par::cex
    pub(crate) layout_cex: f64,
    /// `(x0, x1, y0, y1)` in "user" coordinates - already log10-
    /// transformed if `xlog`/`ylog` is set, matching [`par("usr")`].
    ///
    /// [`par("usr")`]: crate::device::Device::usr
    usr: (f64, f64, f64, f64),
    xlog: bool,
    ylog: bool,
    /// How figures are arranged (`mfrow`/`mfcol`/`layout()`); the default
    /// is a single figure.
    arrangement: Arrangement,
    /// [`par("mfg")`]'s `currentFigure`, 1-based.
    ///
    /// [`par("mfg")`]: crate::graphics::ParCall::mfg
    current_figure: usize,
    /// [`par(fig)`] or [`par(fin)`] (R's `defaultFigure = FALSE`): one figure
    /// region, overriding the arrangement until `mfrow`/`mfcol`/`layout`
    /// is called.
    ///
    /// [`par(fig)`]: crate::graphics::ParCall::fig
    /// [`par(fin)`]: crate::graphics::ParCall::fin
    fig_spec: Option<FigSpec>,
    /// [`par("new")`], set by `par(new = )` and `par(mfg = )`: the
    /// next `plot_new()` reuses the current figure instead of advancing.
    /// Cleared by drawing (see [`Device::dirty`]), not by `plot_new()`.
    ///
    /// [`par("new")`]: crate::par::ParOptions::new
    hold_figure: bool,
    /// Whether anything has been plotted yet (R's `gpptr(dd)->state`).
    used: bool,
    /// `par.ps` when the device was created: R's `startps`, which sets the
    /// device's character size `cra` (`1.2 * startps` points high).
    start_ps: f64,
    /// The file(s) of a device opened with [`DeviceBuilder::open`], which
    /// [`close`](Device::close) writes; `None` for [`Device::new`].
    output: Option<open::Output>,
    /// An R error found while drawing (a negative colour number, ...),
    /// returned by the call when it ends; see [`Device::run`].
    pending_error: std::cell::RefCell<Option<Error>>,
}

/// A [`layout()`](crate::graphics::Graphics::layout) call being built: it
/// divides the page into figures of different sizes.
///
/// Set its arguments with the setters below, then apply it with
/// [`draw`](LayoutCall::draw), which returns the number of figures: on a
/// device, or recorded in a [`Figure`](crate::recording::Figure).
/// Without `.draw()`, nothing happens (the compiler warns).
///
/// See [`Graphics::layout`] for an
/// example.
#[must_use = "nothing is changed until `.draw()`"]
pub struct LayoutCall<'a, G: Graphics> {
    target: &'a mut G,
    mat: Option<Vec<Vec<f64>>>,
    widths: Vec<LayoutSize>,
    heights: Vec<LayoutSize>,
    respect: LayoutRespect,
    /// The first value a setter couldn't convert.
    error: Option<Error>,
}

impl<'a, G: Graphics> LayoutCall<'a, G> {
    pub(crate) fn start(target: &'a mut G) -> Self {
        LayoutCall {
            target,
            mat: None,
            widths: Vec::new(),
            heights: Vec::new(),
            respect: LayoutRespect::No,
            error: None,
        }
    }

    /// The grid of figure numbers, as rows, top to bottom (as the matrix
    /// reads on screen). Every figure number from 1 to the largest must
    /// appear; `0` marks an empty cell, and a number in several cells
    /// makes that figure span them. Fractions are truncated, as R does.
    /// Required.
    pub fn mat<R: AsRef<[T]>, T: crate::numbers::Num>(mut self, mat: &[R]) -> Self {
        self.mat = Some(
            mat.iter()
                .map(|row| row.as_ref().iter().map(|v| v.to_f64()).collect())
                .collect(),
        );
        self
    }

    /// Column widths: numbers are relative sizes (`[2, 1]`), or give
    /// [`LayoutSize`]s to mix in absolute ones (`LayoutSize::Cm`, R's
    /// `lcm()`). A shorter list is padded with relative `1`s; extra
    /// entries are ignored. Default: equal relative widths.
    pub fn widths(mut self, widths: impl crate::setters::Values<LayoutSize>) -> Self {
        match widths.into_values() {
            Ok(v) => self.widths = v,
            Err(e) => {
                self.error.get_or_insert(e);
            }
        }
        self
    }

    /// Row heights; as [`widths`](LayoutCall::widths). Default: equal
    /// relative heights.
    pub fn heights(mut self, heights: impl crate::setters::Values<LayoutSize>) -> Self {
        match heights.into_values() {
            Ok(v) => self.heights = v,
            Err(e) => {
                self.error.get_or_insert(e);
            }
        }
        self
    }

    /// Whether one unit of width equals one unit of height: `true` in every
    /// relative-size cell, or a grid of cells as rows (see
    /// [`LayoutRespect`]). Default: `false`.
    pub fn respect(mut self, respect: impl Into<LayoutRespect>) -> Self {
        self.respect = respect.into();
        self
    }

    /// Applies the layout and returns its number of figures.
    ///
    /// This is R's (invisible) return value of `layout()`.
    ///
    /// # Panics
    ///
    /// With R's error, as [`try_draw`](LayoutCall::try_draw) would return it.
    pub fn draw(self) -> usize {
        self.try_draw().unwrap_or_else(|e| panic!("{e}"))
    }

    /// Applies the layout as [`draw`](LayoutCall::draw) does, returning R's
    /// error instead of panicking.
    ///
    /// # Errors
    ///
    /// With R's message, if `mat` wasn't set (`argument "mat" is missing,
    /// with no default`); if the grid is empty or not rectangular, has more
    /// than 50 rows, 50 columns or 500 cells, has no figure, or skips a
    /// figure number; if `respect`'s grid doesn't have `mat`'s dimensions;
    /// and, unlike R (which accepts them), if `mat` has a negative or
    /// non-finite value. The device is unchanged after an error, and a
    /// `Figure` fails when recording, as the device does.
    pub fn try_draw(self) -> Result<usize, Error> {
        if let Some(e) = self.error {
            return Err(e.in_call("layout"));
        }
        let fail = |m: String| Error::invalid(m).in_call("layout");
        let mat = crate::graphics::required(self.mat, "mat").map_err(|e| e.in_call("layout"))?;
        let mut rows: Vec<Vec<usize>> = Vec::with_capacity(mat.len());
        for row in &mat {
            let mut r = Vec::with_capacity(row.len());
            for &v in row {
                if !(v.is_finite() && v >= 0.0) {
                    return Err(fail(
                        "layout matrix values must be non-negative numbers".into(),
                    ));
                }
                r.push(v.trunc() as usize);
            }
            rows.push(r);
        }
        let n = build_layout(&rows, &self.widths, &self.heights, &self.respect)
            .map_err(|e| e.in_call("layout"))?
            .num_figures();
        self.target
            .emit_layout(rows, self.widths, self.heights, self.respect)
            .map_err(|e| e.in_call("layout"))?;
        Ok(n)
    }
}

/// A [`Layout`] from `layout()`'s checked arguments, or R's error.
pub(crate) fn build_layout(
    rows: &[Vec<usize>],
    widths: &[LayoutSize],
    heights: &[LayoutSize],
    respect: &LayoutRespect,
) -> Result<Layout, Error> {
    let fail = |m: String| Error::invalid(m);
    let mut layout = Layout::new(rows).map_err(|e| fail(e.0))?;
    if !widths.is_empty() {
        layout = layout.widths(widths);
    }
    if !heights.is_empty() {
        layout = layout.heights(heights);
    }
    Ok(match respect {
        LayoutRespect::No => layout,
        LayoutRespect::All => layout.respect(true),
        LayoutRespect::Cells(cells) => layout.respect_matrix(cells).map_err(|e| fail(e.0))?,
    })
}

impl<B: Backend> Device<B> {
    /// `layout()`'s effect on the device; see
    /// [`Graphics::layout`].
    pub(crate) fn layout_impl(
        &mut self,
        rows: &[Vec<usize>],
        widths: &[LayoutSize],
        heights: &[LayoutSize],
        respect: &LayoutRespect,
    ) -> Result<(), Error> {
        let layout = build_layout(rows, widths, heights, respect)?;
        self.set_arrangement(Arrangement::Layout(layout))
    }
}

/// The units [`strwidth`](Device::strwidth) and
/// [`strheight`](Device::strheight) measure in.
///
/// Setters take R's strings (`"user"`, `"inches"`, `"figure"`, or a
/// unique prefix); any other is an error of the call.
///
/// This is R's `strwidth(units = )`.
///
/// # Examples
///
/// ```
/// # use rs_graphics::prelude::*;
/// assert_eq!("in".parse::<StrUnits>().unwrap(), StrUnits::Inches);
/// assert_eq!("fig".parse::<StrUnits>().unwrap(), StrUnits::Figure);
/// assert!("cm".parse::<StrUnits>().is_err());
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum StrUnits {
    /// Data units along the axis (the default): x for a width, y for a
    /// height; log10 values on a log axis, as [`Device::usr`] is. R's
    /// `"user"`.
    #[default]
    User,
    /// Inches. R's `"inches"`.
    Inches,
    /// Fractions of the figure region's width or height. R's `"figure"`.
    Figure,
}

impl StrUnits {
    /// `"user"`, `"inches"` or `"figure"`, or a unique prefix (R's
    /// `match.arg()`); `None` otherwise. The public routes are
    /// `FromStr`/`TryFrom<&str>`.
    pub(crate) fn parse(s: &str) -> Option<StrUnits> {
        let all = [
            ("user", StrUnits::User),
            ("inches", StrUnits::Inches),
            ("figure", StrUnits::Figure),
        ];
        let mut hits = all
            .iter()
            .filter(|(name, _)| !s.is_empty() && name.starts_with(s));
        match (hits.next(), hits.next()) {
            (Some(&(_, u)), None) => Some(u),
            _ => None,
        }
    }
}

crate::error::from_r_string!(
    StrUnits,
    "string units",
    "\"user\", \"inches\" or \"figure\""
);

/// A [`strwidth()`](Device::strwidth) or [`strheight()`](Device::strheight)
/// measurement being built.
///
/// Set its arguments with the setters below, then get the size with
/// [`get`](StrSizeCall::get). See [`Device::strwidth`] for an example.
#[must_use = "nothing is measured until `.get()`"]
pub struct StrSizeCall<'a, B: Backend> {
    dev: &'a mut Device<B>,
    height: bool,
    s: Option<Label>,
    cex: Option<f64>,
    font: Option<FontFace>,
    units: StrUnits,
    /// The first value a setter couldn't convert.
    error: Option<Error>,
}

impl<'a, B: Backend> StrSizeCall<'a, B> {
    fn start(dev: &'a mut Device<B>, height: bool) -> Self {
        StrSizeCall {
            dev,
            height,
            s: None,
            cex: None,
            font: None,
            units: StrUnits::User,
            error: None,
        }
    }

    /// The text (lines separated by `\n`) or plotmath expression
    /// (`Label::math("x^2")`) to measure. Required.
    pub fn s(mut self, s: impl Into<Label>) -> Self {
        self.s = Some(s.into());
        self
    }

    /// Size, relative to [`par(cex)`]. `None`: 1, so [`par(cex)`].
    ///
    /// [`par(cex)`]: crate::par::Par::cex
    pub fn cex(mut self, cex: impl Num) -> Self {
        self.cex = Some(cex.to_f64());
        self
    }

    /// Font face of text ([`FontFace`]; R's number, a name or the enum).
    /// `None`: [`par(font)`]. Expressions use their own fonts.
    ///
    /// [`par(font)`]: crate::par::Par::font
    pub fn font(mut self, font: impl IntoArg<FontFace>) -> Self {
        match font.into_arg() {
            Ok(f) => self.font = Some(f),
            Err(e) => {
                self.error.get_or_insert(e);
            }
        }
        self
    }

    /// The units of the result: data units (the default), inches, or
    /// fractions of the figure region (see [`StrUnits`]).
    pub fn units(mut self, units: impl IntoArg<StrUnits>) -> Self {
        match units.into_arg() {
            Ok(u) => self.units = u,
            Err(e) => {
                self.error.get_or_insert(e);
            }
        }
        self
    }

    /// Returns the width or height in the chosen units.
    ///
    /// # Panics
    ///
    /// If `s` wasn't set or a setter's value was invalid, as
    /// [`try_get`](StrSizeCall::try_get) would return it
    /// (`strwidth(): argument "s" is missing, with no default`).
    pub fn get(self) -> f64 {
        self.try_get().unwrap_or_else(|e| panic!("{e}"))
    }

    /// Returns the width or height as [`get`](StrSizeCall::get) does,
    /// returning R's error instead of panicking.
    ///
    /// # Errors
    ///
    /// If `s` wasn't set (`argument "s" is missing, with no default`), or
    /// `font` or `units` was given a value that isn't one.
    pub fn try_get(self) -> Result<f64, Error> {
        let call = if self.height { "strheight" } else { "strwidth" };
        if let Some(e) = self.error {
            return Err(e.in_call(call));
        }
        let label = crate::graphics::required(self.s, "s").map_err(|e| e.in_call(call))?;
        let cex = self.cex.unwrap_or(1.0);
        let dev = self.dev;
        let face = self.font.unwrap_or(dev.par.font);
        let inches = match &label {
            Label::Text(t) => {
                let (w, h) = dev.text_size_inches(t, face, cex);
                if self.height { h } else { w }
            }
            Label::Math(_) => {
                if self.height {
                    dev.label_height(&label, face, cex)
                } else {
                    dev.label_width(&label, face, cex)
                }
            }
        };
        let (plot, fig, range) = if self.height {
            (dev.plt.height(), dev.fig.height(), dev.usr.3 - dev.usr.2)
        } else {
            (dev.plt.width(), dev.fig.width(), dev.usr.1 - dev.usr.0)
        };
        Ok(match self.units {
            StrUnits::Inches => inches,
            StrUnits::User => inches / plot * range,
            StrUnits::Figure => inches / fig,
        })
    }
}

/// A figure region set with `fig()` or `fin()`.
#[derive(Clone, Copy, Debug, PartialEq)]
enum FigSpec {
    /// [`par(fig)`]: fractions `[x0, x1, y0, y1]` of the inner region.
    ///
    /// [`par(fig)`]: crate::graphics::ParCall::fig
    Nic([f64; 4]),
    /// [`par(fin)`]: `[width, height]` in inches, centred.
    ///
    /// [`par(fin)`]: crate::graphics::ParCall::fin
    Inches([f64; 2]),
}

crate::setters::options! {
    /// The labels of [`Graphics::title`] (`None` draws
    /// nothing) and R's inline `cex.main`/`col.main`/`font.main` and the
    /// `sub` and `lab` ones (`None`: the `par()` value).
    #[derive(Clone, Debug, Default)]
    pub struct TitleOptions => [TitleCall] {
        /// Main title, above the plot. `None`: not drawn.
        pub main: Option<Label> => option Label,
        /// Subtitle, below the x axis label. `None`: not drawn.
        pub sub: Option<Label> => option Label,
        /// x axis label. `None`: not drawn.
        pub xlab: Option<Label> => option Label,
        /// y axis label. `None`: not drawn.
        pub ylab: Option<Label> => option Label,
        /// Size of the main title, relative to [`par(cex)`]. `None`:
        /// [`par(cex.main)`]. R's `cex.main`.
        ///
        /// [`par(cex)`]: crate::par::Par::cex
        /// [`par(cex.main)`]: crate::par::Par::cex_main
        pub cex_main: Option<f64> => number f64,
        /// Colour of the main title. `None`: [`par(col.main)`]. R's `col.main`.
        ///
        /// [`par(col.main)`]: crate::par::Par::col_main
        pub col_main: Option<ColorSpec> => option_value ColorSpec,
        /// Font face of the main title ([`FontFace`]). `None`:
        /// [`par(font.main)`]. R's `font.main`.
        ///
        /// [`par(font.main)`]: crate::par::Par::font_main
        pub font_main: Option<FontFace> => option FontFace,
        /// Size of the subtitle, relative to [`par(cex)`]. `None`:
        /// [`par(cex.sub)`]. R's `cex.sub`.
        ///
        /// [`par(cex)`]: crate::par::Par::cex
        /// [`par(cex.sub)`]: crate::par::Par::cex_sub
        pub cex_sub: Option<f64> => number f64,
        /// Colour of the subtitle. `None`: [`par(col.sub)`]. R's `col.sub`.
        ///
        /// [`par(col.sub)`]: crate::par::Par::col_sub
        pub col_sub: Option<ColorSpec> => option_value ColorSpec,
        /// Font face of the subtitle ([`FontFace`]). `None`: [`par(font.sub)`].
        /// R's `font.sub`.
        ///
        /// [`par(font.sub)`]: crate::par::Par::font_sub
        pub font_sub: Option<FontFace> => option FontFace,
        /// Size of the axis labels (`xlab`, `ylab`), relative to [`par(cex)`].
        /// `None`: [`par(cex.lab)`]. R's `cex.lab`.
        ///
        /// [`par(cex)`]: crate::par::Par::cex
        /// [`par(cex.lab)`]: crate::par::Par::cex_lab
        pub cex_lab: Option<f64> => number f64,
        /// Colour of the axis labels (`xlab`, `ylab`). `None`: [`par(col.lab)`].
        /// R's `col.lab`.
        ///
        /// [`par(col.lab)`]: crate::par::Par::col_lab
        pub col_lab: Option<ColorSpec> => option_value ColorSpec,
        /// Font face of the axis labels (`xlab`, `ylab`; [`FontFace`]).
        /// `None`: [`par(font.lab)`]. R's `font.lab`.
        ///
        /// [`par(font.lab)`]: crate::par::Par::font_lab
        pub font_lab: Option<FontFace> => option FontFace,
        /// Where each title sits along its side of the plot region, and how
        /// it's justified there: 0 left (bottom for `ylab`), 0.5 centred, 1
        /// right (top). `None`: [`par(adj)`].
        ///
        /// [`par(adj)`]: crate::par::Par::adj
        pub adj: Option<f64> => number f64,
        /// Margin lines for the axis labels (`mgp[0]`) and the subtitle
        /// (`mgp[0] + 1`). `None`: [`par(mgp)`].
        ///
        /// [`par(mgp)`]: crate::par::Par::mgp
        pub mgp: Option<[f64; 3]> => array [f64; 3],
        /// Margin line for every title given, instead of their defaults (half
        /// the top margin for `main`, `mgp[0]` for `xlab`/`ylab`, `mgp[0] + 1`
        /// for `sub`); `main` then sits with its baseline on it. `None` (or
        /// non-finite): the defaults.
        pub line: Option<f64> => number f64,
        /// Draws in the outer margins ([`par(oma)`]) instead, positioned along
        /// the inner region; `main` defaults to half the top outer margin.
        /// Default `false`.
        ///
        /// [`par(oma)`]: crate::par::Par::oma
        pub outer: bool => value bool,
        /// Where the titles are clipped: at least the figure region
        /// ([`Xpd::Plot`] acts as [`Xpd::Figure`], as in R), or
        /// [`Xpd::Device`] for the whole device. `None`: [`par(xpd)`]. `outer`
        /// titles clip to the device.
        ///
        /// [`par(xpd)`]: crate::par::Par::xpd
        pub xpd: Option<Xpd> => option Xpd,
    }
}

impl<B: Backend> Device<B> {
    /// Creates a device drawing onto a backend of your own.
    ///
    /// For a [`Backend`] you implement (a GUI canvas, another file format):
    /// PDF, SVG and PNG devices are opened with [`Device::pdf`],
    /// [`Device::svg`], [`Device::png`], [`Device::format`] or
    /// [`Device::memory`]. The device starts with R's default graphical
    /// parameters and the backend's background ([`Backend::default_bg`]);
    /// change them with [`par()`](crate::graphics::Graphics::par). It has no
    /// file: [`finish`](Device::finish) returns the backend's bytes, or
    /// [`into_backend`](Device::into_backend) the backend itself.
    ///
    /// R's equivalent is a graphics device written in C.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// # fn custom_backend() -> AnyBackend { Device::memory("svg").open().unwrap().into_backend() }
    /// let mut dev = Device::new(custom_backend());
    /// dev.par().cex(1.2).draw();
    /// dev.plot().x([1, 2, 3]).draw();
    /// let bytes = dev.finish();
    /// ```
    pub fn new(backend: B) -> Self {
        Device::with_par(backend, Par::default())
    }

    /// [`Device::new`] starting from `par`, for the device builder (the
    /// starting point size and background).
    pub(crate) fn with_par(backend: B, mut par: Par) -> Self {
        if par.bg == Par::default().bg {
            par.bg = backend.default_bg();
        }
        let (width_in, height_in) = backend.size_inches();
        let par_ps = par.ps;
        let mut dev = Device {
            backend,
            par,
            width_in,
            height_in,
            inner: Rect::new(0.0, 0.0, width_in, height_in),
            mar: Sides::new(0.0, 0.0, 0.0, 0.0),
            fig: Rect::new(0.0, 0.0, width_in, height_in),
            plt: Rect::new(0.0, 0.0, width_in, height_in),
            region_error: None,
            layout_cex: 1.0,
            usr: (0.0, 1.0, 0.0, 1.0),
            xlog: false,
            ylog: false,
            arrangement: Arrangement::Grid {
                nrow: 1,
                ncol: 1,
                by_col: false,
            },
            current_figure: 1,
            fig_spec: None,
            hold_figure: false,
            used: false,
            start_ps: par_ps,
            output: None,
            pending_error: std::cell::RefCell::new(None),
        };
        dev.recompute_layout();
        dev
    }

    /// Returns the backend: your own one from [`Device::new`], or an
    /// [`AnyBackend`](crate::backend::AnyBackend) to inspect
    /// ([`AnyBackend::format`](crate::backend::AnyBackend::format)) or
    /// finish with [`Backend::into_bytes`].
    ///
    /// On a device opened for a file this doesn't write the file: the current
    /// page's file stays empty (with one file per page, earlier pages' files
    /// are already written and the backend holds only the last page). Use
    /// [`close`](Device::close) for that.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// let mut dev = Device::memory("svg").open().unwrap();
    /// dev.plot().x([1, 2, 3]).draw();
    /// let backend = dev.into_backend();
    /// assert_eq!(backend.format(), Some(Format::Svg));
    /// let svg = backend.into_bytes();
    /// assert!(String::from_utf8(svg).unwrap().contains("<svg"));
    /// ```
    pub fn into_backend(self) -> B {
        self.backend
    }

    /// Finishes the output and returns its bytes: PDF, PNG, or SVG as UTF-8.
    ///
    /// On a device opened for a file this doesn't write the file: the current
    /// page's file stays empty (with one file per page, earlier pages' files
    /// are already written and these are the last page's bytes). Use
    /// [`close`](Device::close) for that.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// # fn main() -> Result<(), rs_graphics::Error> {
    /// let mut dev = Device::memory("pdf").width(5).height(4).open()?;
    /// dev.plot().x([1, 2, 3]).draw();
    /// let pdf: Vec<u8> = dev.finish();
    /// assert!(pdf.starts_with(b"%PDF"));
    /// # Ok(())
    /// # }
    /// ```
    pub fn finish(self) -> Vec<u8> {
        self.backend.into_bytes()
    }

    /// Closes the device: finishes the output and writes it to the file
    /// created when the device was opened ([`DeviceBuilder::open`]).
    ///
    /// With one file per page (a PNG or SVG file name with a page number such
    /// as `%03d`, or a PDF with [`onefile(false)`](DeviceBuilder::onefile)),
    /// this writes the last page's file. A device dropped without `close()`
    /// leaves that file empty.
    ///
    /// This is R's `dev.off()`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use rs_graphics::prelude::*;
    /// # fn main() -> Result<(), rs_graphics::Error> {
    /// let mut dev = Device::png("plot.png").res(150).open()?;
    /// dev.plot().x([1, 2, 3]).draw();
    /// dev.close()?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// The errors are named for R's `dev.off()`: an [`ErrorKind::Io`] error
    /// if writing the file fails, or writing an earlier page's file failed
    /// (the first such error). A device from [`Device::memory`] or
    /// [`Device::new`] has no file: closing it is an
    /// [`ErrorKind::InvalidArgument`] error; use [`finish`](Device::finish)
    /// for its bytes.
    ///
    /// [`ErrorKind::Io`]: crate::ErrorKind::Io
    /// [`ErrorKind::InvalidArgument`]: crate::ErrorKind::InvalidArgument
    pub fn close(mut self) -> Result<(), Error> {
        let Some(output) = self.output.take() else {
            return Err(Error::invalid(
                "no file to close: the device's output is in memory \
                 (Device::memory or Device::new); use finish()",
            )
            .in_call("dev.off"));
        };
        output
            .close(&self.finish())
            .map_err(|e| Error::io("dev.off", e))
    }

    /// Returns the page size in inches, `[width, height]`.
    ///
    /// This is R's [`par("din")`].
    ///
    /// # Examples
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// let dev = Device::null().width(7).height(5).open().unwrap();
    /// assert_eq!(dev.din(), [7.0, 5.0]);
    /// ```
    ///
    /// [`par("din")`]: crate::device::Device::din
    pub fn din(&self) -> [f64; 2] {
        [self.width_in, self.height_in]
    }

    /// Returns the current plot region, in inches from the page's bottom-left
    /// corner.
    ///
    /// This is R's [`par("plt")`], in inches rather than fractions of the figure.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// # let mut dev = Device::null().open().unwrap();
    /// # dev.plot().x([1, 10]).y([1, 10]).draw();
    /// let r = dev.plot_region();
    /// assert!(r.x0 > 0.0 && r.x1 < dev.din()[0]);
    /// ```
    ///
    /// [`par("plt")`]: crate::par::Par::plt
    pub fn plot_region(&self) -> Rect {
        self.plt
    }

    /// Returns the coordinate limits of the plot region, `[x0, x1, y0, y1]`
    /// (what [`par().usr(..)`](crate::par::ParOptions::usr) takes).
    ///
    /// On a log axis the limits are log10 values (for data from 1 to 100 the x
    /// limits start near 0, not 1), while everything that takes coordinates
    /// (`points()`, `axis(at = )`, [`Device::user_to_inches`], ...) takes data
    /// units. Set by `plot_window()` (which high-level plots call) or
    /// `par(usr = )`.
    ///
    /// This is R's [`par("usr")`].
    ///
    /// # Examples
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// # let mut dev = Device::null().open().unwrap();
    /// dev.plot_new();
    /// dev.plot_window().xlim([0, 10]).ylim([0, 1]).draw();
    /// let [x0, x1, ..] = dev.usr();
    /// assert!((x0 + 0.4).abs() < 1e-9 && (x1 - 10.4).abs() < 1e-9); // 4% added at each end
    /// ```
    ///
    /// [`par("usr")`]: crate::device::Device::usr
    pub fn usr(&self) -> [f64; 4] {
        let (x0, x1, y0, y1) = self.usr;
        [x0, x1, y0, y1]
    }

    /// Returns whether the x axis is logarithmic.
    ///
    /// Set by the last `plot_window()` or `par(xlog = )`; `plot_new()` resets
    /// it.
    ///
    /// This is R's [`par("xlog")`].
    ///
    /// # Examples
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// # let mut dev = Device::null().open().unwrap();
    /// dev.plot().x([1, 10, 100]).xlog(true).draw();
    /// assert!(dev.xlog());
    /// ```
    ///
    /// [`par("xlog")`]: crate::device::Device::xlog
    pub fn xlog(&self) -> bool {
        self.xlog
    }

    /// Returns whether the y axis is logarithmic; see [`Device::xlog`].
    ///
    /// This is R's [`par("ylog")`].
    ///
    /// # Examples
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// # let mut dev = Device::null().open().unwrap();
    /// dev.plot().x([1, 10, 100]).ylog(true).draw();
    /// assert!(dev.ylog());
    /// ```
    ///
    /// [`par("ylog")`]: crate::device::Device::ylog
    pub fn ylog(&self) -> bool {
        self.ylog
    }

    fn recompute_layout(&mut self) {
        self.layout_cex = self.par.cex;
        // Inner region: the device minus the outer margins; figures are
        // fractions (NIC) of it.
        let omi = self.omi();
        let inner = Rect::new(
            omi.left,
            omi.bottom,
            self.width_in - omi.right,
            self.height_in - omi.top,
        );
        self.inner = inner;
        // Margins in lines, for `title()` (R's `updateFigureMargins()`
        // converts `mai` to `mar`).
        let lh = self.line_height_inches();
        self.mar = match self.par.mai {
            Some(m) => Sides::new(m.bottom / lh, m.left / lh, m.top / lh, m.right / lh),
            None => self.par.mar,
        };
        let [fx0, fx1, fy0, fy1] = match self.fig_spec {
            Some(FigSpec::Nic(fig)) => fig,
            // `updateFigureRegion()`: `fin` centred in the inner region.
            Some(FigSpec::Inches([w, h])) => {
                let (nw, nh) = (w / inner.width(), h / inner.height());
                [
                    0.5 - nw / 2.0,
                    0.5 + nw / 2.0,
                    0.5 - nh / 2.0,
                    0.5 + nh / 2.0,
                ]
            }
            None => self.arrangement.figure_region_nic(
                self.current_figure,
                inner.width(),
                inner.height(),
            ),
        };
        self.fig = Rect::new(
            inner.x0 + fx0 * inner.width(),
            inner.y0 + fy0 * inner.height(),
            inner.x0 + fx1 * inner.width(),
            inner.y0 + fy1 * inner.height(),
        );
        // Corners as computed, before `Rect::new` sorts them, so that
        // `region_error()` sees margins that cross.
        let [x0, x1, y0, y1] = match (self.par.pin, self.par.plt) {
            // `par(pin)`: that size in inches, centred in the figure
            // (`updatePlotRegion()`).
            (Some([w, h]), _) => {
                let (cx, cy) = (
                    0.5 * (self.fig.x0 + self.fig.x1),
                    0.5 * (self.fig.y0 + self.fig.y1),
                );
                [cx - w / 2.0, cx + w / 2.0, cy - h / 2.0, cy + h / 2.0]
            }
            // `par(plt)`: fractions of the figure region.
            (None, Some([px0, px1, py0, py1])) => [
                self.fig.x0 + px0 * self.fig.width(),
                self.fig.x0 + px1 * self.fig.width(),
                self.fig.y0 + py0 * self.fig.height(),
                self.fig.y0 + py1 * self.fig.height(),
            ],
            // `mapPlotRegion()`: the figure less `mar`, made square about
            // its centre for `pty = "s"`.
            (None, None) => {
                let mai = self.mai();
                let (mut x0, mut x1) = (self.fig.x0 + mai.left, self.fig.x1 - mai.right);
                let (mut y0, mut y1) = (self.fig.y0 + mai.bottom, self.fig.y1 - mai.top);
                if self.par.pty == Pty::Square {
                    let (w, h) = (x1 - x0, y1 - y0);
                    if w > h {
                        let c = 0.5 * (x0 + x1);
                        (x0, x1) = (c - 0.5 * h, c + 0.5 * h);
                    } else {
                        let c = 0.5 * (y0 + y1);
                        (y0, y1) = (c - 0.5 * w, c + 0.5 * w);
                    }
                }
                [x0, x1, y0, y1]
            }
        };
        self.plt = Rect::new(x0, y0, x1, y1);
        // The plot region as fractions of the figure (R's NFC `plt`).
        let (fw, fh) = (self.fig.width(), self.fig.height());
        let plt_nfc = [
            (x0 - self.fig.x0) / fw,
            (x1 - self.fig.x0) / fw,
            (y0 - self.fig.y0) / fh,
            (y1 - self.fig.y0) / fh,
        ];
        self.region_error = region_error([fx0, fx1, fy0, fy1], plt_nfc);
    }

    /// `par(fig = )`; see [`ParCall::fig`](crate::graphics::ParCall::fig).
    pub(crate) fn fig(&mut self, fig: [f64; 4]) -> Result<(), Error> {
        check_fig(fig)?;
        self.arrangement = Arrangement::Grid {
            nrow: 1,
            ncol: 1,
            by_col: false,
        };
        self.current_figure = 1;
        self.fig_spec = Some(FigSpec::Nic(fig));
        self.recompute_layout();
        Ok(())
    }

    /// R's `currentFigure = lastFigure`: the next `plot_new()` starts a
    /// new page (for `par(oma = )` and co.).
    pub(crate) fn eject_page(&mut self) {
        self.current_figure = self.arrangement.last_figure();
    }

    /// `par(fin = )` (R's `fUnits = INCHES`); see
    /// [`ParCall::fin`](crate::graphics::ParCall::fin).
    pub(crate) fn fin(&mut self, fin: [f64; 2]) -> Result<(), Error> {
        check_fin(fin)?;
        self.arrangement = Arrangement::Grid {
            nrow: 1,
            ncol: 1,
            by_col: false,
        };
        self.current_figure = 1;
        self.fig_spec = Some(FigSpec::Inches(fin));
        self.recompute_layout();
        Ok(())
    }

    /// `par(mfrow = )`; see
    /// [`ParCall::mfrow`](crate::graphics::ParCall::mfrow).
    pub(crate) fn mfrow(&mut self, nrow: usize, ncol: usize) -> Result<(), Error> {
        check_grid("mfrow", nrow, ncol)?;
        self.set_arrangement(Arrangement::Grid {
            nrow,
            ncol,
            by_col: false,
        })
    }

    /// `par(mfcol = )`; see
    /// [`ParCall::mfcol`](crate::graphics::ParCall::mfcol).
    pub(crate) fn mfcol(&mut self, nrow: usize, ncol: usize) -> Result<(), Error> {
        check_grid("mfcol", nrow, ncol)?;
        self.set_arrangement(Arrangement::Grid {
            nrow,
            ncol,
            by_col: true,
        })
    }

    fn set_arrangement(&mut self, arrangement: Arrangement) -> Result<(), Error> {
        if !(arrangement.nrow() > 0 && arrangement.ncol() > 0) {
            return Err(Error::invalid(
                "a figure grid needs at least one row and one column",
            ));
        }
        self.par.cex = arrangement.cex_base();
        self.par.mex = 1.0;
        // R sets `currentFigure = lastFigure` so the next plot.new()
        // wraps round to figure 1 on a fresh page.
        self.current_figure = arrangement.last_figure();
        self.arrangement = arrangement;
        self.fig_spec = None;
        self.hold_figure = false;
        self.recompute_layout();
        Ok(())
    }

    /// `par(mfg = )`; see [`ParCall::mfg`](crate::graphics::ParCall::mfg).
    pub(crate) fn mfg(&mut self, row: usize, col: usize) -> Result<(), Error> {
        let (nrow, ncol) = (self.arrangement.nrow(), self.arrangement.ncol());
        if row == 0 || col == 0 {
            return Err(Error::invalid(
                "invalid value specified for graphical parameter \"mfg\"",
            ));
        }
        if !(1..=nrow).contains(&row) {
            return Err(Error::invalid("parameter \"i\" in \"mfg\" is out of range"));
        }
        if !(1..=ncol).contains(&col) {
            return Err(Error::invalid("parameter \"j\" in \"mfg\" is out of range"));
        }
        let by_col = matches!(self.arrangement, Arrangement::Grid { by_col: true, .. });
        self.current_figure = if by_col {
            (col - 1) * nrow + row
        } else {
            (row - 1) * ncol + col
        };
        self.hold_figure = true;
        self.recompute_layout();
        Ok(())
    }

    /// `par(new = )`; see [`ParOptions::new`](crate::par::ParOptions::new).
    pub(crate) fn set_new(&mut self, new: bool) {
        if self.used {
            self.hold_figure = new;
        }
    }

    /// Whether anything has been plotted yet.
    pub(crate) fn is_used(&self) -> bool {
        self.used
    }

    /// Returns the graphical parameters as they are now.
    ///
    /// Change them with [`par()`](crate::graphics::Graphics::par).
    ///
    /// This is R's `par()` read form: R's [`par("cex")`] is
    /// `dev.current_par().cex`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// # let mut dev = Device::null().open().unwrap();
    /// dev.par().mfrow([2, 2]).draw();
    /// assert_eq!(dev.current_par().cex, 0.83);
    /// ```
    ///
    /// [`par("cex")`]: crate::par::Par::cex
    pub fn current_par(&self) -> &Par {
        &self.par
    }

    /// Returns whether the next plot will be drawn over the current one
    /// instead of starting a new figure.
    ///
    /// Set with `dev.par().new(true)`; any drawing clears it.
    ///
    /// This is R's [`par("new")`].
    ///
    /// # Examples
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// # let mut dev = Device::null().open().unwrap();
    /// # dev.plot().x([1, 10]).y([1, 10]).draw();
    /// dev.par().new(true).draw();
    /// assert!(dev.is_new());
    /// ```
    ///
    /// [`par("new")`]: crate::par::ParOptions::new
    pub fn is_new(&self) -> bool {
        self.hold_figure
    }

    /// Returns the number of the current figure (panel), counting from 1.
    ///
    /// This is the position in R's [`par("mfg")`].
    ///
    /// # Examples
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// # let mut dev = Device::null().open().unwrap();
    /// dev.par().mfrow([1, 2]).draw();
    /// dev.plot().x([1, 2]).draw();
    /// dev.plot().x([2, 1]).draw();
    /// assert_eq!(dev.current_figure(), 2);
    /// ```
    ///
    /// [`par("mfg")`]: crate::graphics::ParCall::mfg
    pub fn current_figure(&self) -> usize {
        self.current_figure
    }

    /// Returns the current figure region, in inches from the page's
    /// bottom-left corner.
    ///
    /// This is R's [`par("fig")`], in inches rather than fractions of the inner
    /// region.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// # let mut dev = Device::null().open().unwrap();
    /// dev.par().mfrow([1, 2]).draw();
    /// dev.plot().x([1, 2]).draw();
    /// assert_eq!(dev.figure_region().x1, dev.din()[0] / 2.0);
    /// ```
    ///
    /// [`par("fig")`]: crate::graphics::ParCall::fig
    pub fn figure_region(&self) -> Rect {
        self.fig
    }

    /// Draws R's `plot.new()`; the public entry is
    /// [`Graphics::plot_new`], whose docs
    /// describe it.
    pub(crate) fn plot_new_impl(&mut self) -> Result<(), Error> {
        let mut new_page = false;
        let had_page = self.used;
        if !self.hold_figure || !self.used {
            self.current_figure += 1;
            if self.current_figure > self.arrangement.last_figure() || !self.used {
                self.current_figure = 1;
                new_page = true;
            }
        }
        self.used = true;
        self.recompute_layout();
        if new_page {
            if had_page {
                self.next_page_file();
            }
            self.backend.new_page();
            if self.par.bg.a > 0 {
                self.backend.draw_rect(
                    Rect::new(0.0, 0.0, self.width_in, self.height_in),
                    Some(self.par.bg),
                    None,
                );
            }
        }
        if let Some(msg) = self.region_error {
            // R's `invalidError()`: back to the previous figure, so the
            // next `plot_new()` retries this one.
            self.current_figure -= 1;
            if self.current_figure < 1 {
                self.current_figure = self.arrangement.last_figure();
            }
            return Err(Error::does_not_fit("plot.new", msg));
        }
        // `C_plot_new`: linear axes, `GScale(0, 1)` on both.
        self.set_window((0.0, 1.0), (0.0, 1.0), false, false);
        Ok(())
    }

    /// Runs one call's drawing: its result, or the first R error found
    /// while drawing (recorded by [`Device::record_error`]), which R would
    /// have stopped at.
    pub(crate) fn run<T>(
        &mut self,
        f: impl FnOnce(&mut Self) -> Result<T, Error>,
    ) -> Result<T, Error> {
        // A call made inside another (`plot(panel.first = )`) keeps the
        // outer call's error for the outer call to return.
        let outer = self.pending_error.borrow_mut().take();
        let result = f(self);
        let inner = self.pending_error.replace(outer);
        match inner {
            Some(e) => Err(e),
            None => result,
        }
    }

    /// Records an R error found while drawing, where returning it isn't
    /// practical (deep in colour or text code); the call returns it when it
    /// ends ([`Device::run`]). The first one counts.
    pub(crate) fn record_error(&self, e: Error) {
        let mut slot = self.pending_error.borrow_mut();
        if slot.is_none() {
            *slot = Some(e);
        }
    }

    /// R's `par(xlog = , ylog = )` (`None`: unchanged): only the flags, as
    /// in R; `usr` and the mapping stay.
    pub(crate) fn set_log(&mut self, xlog: Option<bool>, ylog: Option<bool>) {
        if let Some(x) = xlog {
            self.xlog = x;
        }
        if let Some(y) = ylog {
            self.ylog = y;
        }
    }

    /// R's `par(usr = )`: the plot region's user coordinates (log10 units
    /// on a log axis), and the axis tick settings from them
    /// (`GSetupAxis()`). Checked by `ParOptions::check`.
    pub(crate) fn set_usr(&mut self, [x0, x1, y0, y1]: [f64; 4]) {
        self.usr = (x0, x1, y0, y1);
        self.par.xaxp = axis_pars(x0, x1, self.par.lab[0], self.xlog);
        self.par.yaxp = axis_pars(y0, y1, self.par.lab[1], self.ylog);
    }

    fn set_window(&mut self, xlim: (f64, f64), ylim: (f64, f64), xlog: bool, ylog: bool) {
        self.xlog = xlog;
        self.ylog = ylog;
        let (ux0, ux1) = Self::gscale(xlim.0, xlim.1, self.par.xaxs, xlog);
        let (uy0, uy1) = Self::gscale(ylim.0, ylim.1, self.par.yaxs, ylog);
        self.usr = (ux0, ux1, uy0, uy1);
        self.par.xaxp = axis_pars(ux0, ux1, self.par.lab[0], xlog);
        self.par.yaxp = axis_pars(uy0, uy1, self.par.lab[1], ylog);
    }

    /// `GScale()` (`graphics.c`): the `usr` range (log10 units on a log
    /// axis) for data limits `min..max` (which may be decreasing). Non-finite
    /// limits are replaced by huge ones (on a log axis, limits <= 0 give
    /// about 1e-307, as R does after warning "nonfinite axis limits"), a
    /// zero-width range is widened (to -1..1 around 0, else by 40% of the
    /// value), and `xaxs = "r"` adds 4% at each end.
    fn gscale(min: f64, max: f64, style: AxisStyle, log: bool) -> (f64, f64) {
        const EPS_FAC_1: f64 = 16.0;
        let (mut min, mut max) = if log {
            (min.log10(), max.log10())
        } else {
            (min, max)
        };
        let fix = |v: f64| {
            if v.is_finite() {
                v
            } else if log {
                if v < 0.0 { -320.0 } else { 308.254715559 }
            } else {
                0.45 * if v < 0.0 { -f64::MAX } else { f64::MAX }
            }
        };
        min = fix(min);
        max = fix(max);
        let mut temp = min.abs().max(max.abs());
        if temp == 0.0 {
            min = -1.0;
            max = 1.0;
        } else {
            let mut tf = if temp > 1.0 {
                (temp * f64::EPSILON) * EPS_FAC_1
            } else {
                (temp * EPS_FAC_1) * f64::EPSILON
            };
            if tf == 0.0 {
                tf = f64::MIN_POSITIVE;
            }
            if (max - min).abs() < tf {
                temp *= if min == max { 0.4 } else { 1e-2 };
                min -= temp;
                max += temp;
            }
        }
        if style == AxisStyle::Regular {
            let d = if temp > 100.0 {
                0.04 * max - 0.04 * min
            } else {
                0.04 * (max - min)
            };
            let clamp = |v: f64| {
                if v.is_finite() {
                    v
                } else if v < 0.0 {
                    -f64::MAX
                } else {
                    f64::MAX
                }
            };
            if log {
                min -= d;
                max += d;
            } else {
                min = clamp(min - d);
                max = clamp(max + d);
            }
        }
        if log {
            // Keep 10^min above 0 and 10^max finite.
            if 10f64.powf(min) == 0.0 {
                min = (1.01 * f64::MIN_POSITIVE).log10();
            }
            if max >= 308.25035 {
                max = (0.99 * f64::MAX).log10();
            }
        }
        (min, max)
    }

    /// Returns the tick positions [`axis()`](crate::graphics::Graphics::axis)
    /// would use on `side` without `at`, in data units.
    ///
    /// They come from [`par("xaxp")`] (sides 1 and 3) or [`par("yaxp")`] (sides 2
    /// and 4), which `plot_new()` and `plot_window()` set from the axis range
    /// and [`par("lab")`].
    ///
    /// This is R's `axTicks()`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// # let mut dev = Device::null().open().unwrap();
    /// dev.plot_new();
    /// dev.plot_window().xlim([0, 10]).ylim([0, 1]).draw();
    /// assert_eq!(dev.ax_ticks(1), [0.0, 2.0, 4.0, 6.0, 8.0, 10.0]);
    /// ```
    ///
    /// # Panics
    ///
    /// If `side` isn't 1 to 4 (R: "invalid axis number").
    ///
    /// [`par("lab")`]: crate::par::Par::lab
    /// [`par("xaxp")`]: crate::par::Par::xaxp
    /// [`par("yaxp")`]: crate::par::Par::yaxp
    pub fn ax_ticks(&self, side: impl IntoArg<Side>) -> Vec<f64> {
        let side = side
            .into_arg()
            .unwrap_or_else(|e| panic!("axTicks(): {}", e.message()));
        let axp = if side.is_horizontal() {
            self.par.xaxp
        } else {
            self.par.yaxp
        };
        self.ticks_from_axp(side, axp)
    }

    /// R's error in `CreateAtVector()` for a log axis whose `axp[3]` (the
    /// given one, else `par(xaxp/yaxp)`) isn't one of its codes 1 to 3.
    pub(crate) fn check_axis_axp(&self, side: Side, axp: Option<[f64; 3]>) -> Result<(), Error> {
        let (log, par_axp, xy) = if side.is_horizontal() {
            (self.xlog, self.par.xaxp, "x")
        } else {
            (self.ylog, self.par.yaxp, "y")
        };
        let code = axp.unwrap_or(par_axp)[2];
        // A negative code is a linear count; `create_at_vector` rounds.
        if log && (code.is_nan() || code >= 0.0) && ![1, 2, 3].contains(&((code + 0.5) as i32)) {
            return Err(Error::invalid(format!(
                "log - axis(), 'at' creation: INVALID {xy}axp[3] = {code}"
            )));
        }
        Ok(())
    }

    /// `CreateAtVector()` for `side`'s axis with the given `axp`.
    fn ticks_from_axp(&self, side: Side, axp: [f64; 3]) -> Vec<f64> {
        let (lo, hi, log, nint) = if side.is_horizontal() {
            (self.usr.0, self.usr.1, self.xlog, self.par.lab[0])
        } else {
            (self.usr.2, self.usr.3, self.ylog, self.par.lab[1])
        };
        let data = |v: f64| if log { 10f64.powf(v) } else { v };
        create_at_vector(axp, data(lo), data(hi), nint, log)
    }

    // ---------------------------------------------------------------
    // Coordinate transforms
    // ---------------------------------------------------------------

    /// Converts a point in data units to inches from the page's bottom-left
    /// corner.
    ///
    /// Data units are not log10 values on a log axis.
    ///
    /// This is R's `grconvertX()`/`grconvertY()` from `"user"` to `"inches"`,
    /// measured from the page's bottom-left corner.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// # let mut dev = Device::null().open().unwrap();
    /// # dev.plot().x([1, 10]).y([1, 10]).draw();
    /// let p = dev.user_to_inches(Point::new(5.0, 5.0));
    /// let q = dev.inches_to_user(p);
    /// assert!((q.x - 5.0).abs() < 1e-9 && (q.y - 5.0).abs() < 1e-9);
    /// ```
    pub fn user_to_inches(&self, p: Point) -> Point {
        self.data_to_inches(p.x, p.y)
    }

    /// Converts a point in inches from the page's bottom-left corner to data
    /// units: the inverse of [`Device::user_to_inches`].
    ///
    /// Data units are not log10 values on a log axis.
    ///
    /// This is R's `grconvertX()`/`grconvertY()` from `"inches"` to `"user"`,
    /// measured from the page's bottom-left corner.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// # let mut dev = Device::null().open().unwrap();
    /// # dev.plot().x([1, 10]).y([1, 10]).draw();
    /// let r = dev.plot_region();
    /// let corner = dev.inches_to_user(Point::new(r.x0, r.y0));
    /// assert!((corner.x - dev.usr()[0]).abs() < 1e-9);
    /// ```
    pub fn inches_to_user(&self, p: Point) -> Point {
        let (x, y) = self.inches_to_data(p);
        Point::new(x, y)
    }

    /// [`Device::user_to_inches`] for a data point given as two numbers.
    pub(crate) fn data_to_inches(&self, x: impl Into<f64>, y: impl Into<f64>) -> Point {
        let (x, y) = (x.into(), y.into());
        let ux = if self.xlog { x.log10() } else { x };
        let uy = if self.ylog { y.log10() } else { y };
        self.usr_to_inches(ux, uy)
    }

    /// [`Device::inches_to_user`] as a pair of data values.
    pub(crate) fn inches_to_data(&self, p: Point) -> (f64, f64) {
        let (ux, uy) = self.inches_to_usr(p);
        let x = if self.xlog { 10f64.powf(ux) } else { ux };
        let y = if self.ylog { 10f64.powf(uy) } else { uy };
        (x, y)
    }

    /// Map a point in device inches to `usr` units (log10 on a log axis).
    pub(crate) fn inches_to_usr(&self, p: Point) -> (f64, f64) {
        (self.axis_map(true).usr(p.x), self.axis_map(false).usr(p.y))
    }

    /// Map a point in `usr` units (log10 on a log axis) to device inches.
    fn usr_to_inches(&self, ux: f64, uy: f64) -> Point {
        Point::new(
            self.axis_map(true).inches(ux),
            self.axis_map(false).inches(uy),
        )
    }

    /// The x (`x_axis`) or y axis's map between `usr` units and inches.
    pub(crate) fn axis_map(&self, x_axis: bool) -> AxisMap {
        let (x0, x1, y0, y1) = self.usr;
        let plt = self.plt;
        if x_axis {
            AxisMap {
                u0: x0,
                u1: x1,
                d0: plt.x0,
                d1: plt.x1,
            }
        } else {
            AxisMap {
                u0: y0,
                u1: y1,
                d0: plt.y0,
                d1: plt.y1,
            }
        }
    }

    /// A data value on the x (`x_axis`) or y axis in inches along it.
    pub(crate) fn data_to_inches_along(&self, x_axis: bool, v: f64) -> f64 {
        let log = if x_axis { self.xlog } else { self.ylog };
        self.axis_map(x_axis)
            .inches(if log { v.log10() } else { v })
    }

    /// R's `getxlimits()`/`getylimits()` in `usr` units: the `usr` range
    /// for `xpd = FALSE`, else the figure region's (`TRUE`) or the
    /// device's (`NA`) extent along the axis.
    fn xpd_limits(&self, x_axis: bool, xpd: Xpd) -> (f64, f64) {
        let map = self.axis_map(x_axis);
        let (f0, f1, d1) = if x_axis {
            (self.fig.x0, self.fig.x1, self.width_in)
        } else {
            (self.fig.y0, self.fig.y1, self.height_in)
        };
        match xpd {
            Xpd::Plot => (map.u0, map.u1),
            Xpd::Figure => (map.usr(f0), map.usr(f1)),
            Xpd::Device => (map.usr(0.0), map.usr(d1)),
        }
    }

    /// Returns the device's starting point size.
    ///
    /// It is the [`pointsize`](DeviceBuilder::pointsize) the device was opened
    /// with (12 for [`Device::new`]). Margin lines and
    /// plotting symbols are sized from it; changing [`par(ps)`] later only
    /// changes the text size.
    ///
    /// This is R's `startps`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use rs_graphics::prelude::*;
    /// # fn main() -> Result<(), rs_graphics::Error> {
    /// let dev = Device::svg("plot.svg").pointsize(10).open()?;
    /// assert_eq!(dev.start_ps(), 10.0);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// [`par(ps)`]: crate::par::Par::ps
    pub fn start_ps(&self) -> f64 {
        self.start_ps
    }

    /// Returns the height in inches of one margin line, the unit of `mar`,
    /// `oma`, `mgp` and `tcl`.
    ///
    /// It is `mex * cex * 1.2 * startps / 72` (see [`start_ps`](Device::start_ps)).
    ///
    /// # Examples
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// # let dev = Device::null().open().unwrap();
    /// assert_eq!(dev.line_height_inches(), 1.2 * 12.0 / 72.0);
    /// ```
    pub fn line_height_inches(&self) -> f64 {
        self.par.mex * self.par.cex * 1.2 * self.start_ps / 72.0
    }

    /// Returns the margins in inches (bottom, left, top, right): [`par(mai)`] if
    /// set, else [`par(mar)`] converted.
    ///
    /// This is R's [`par("mai")`].
    ///
    /// # Examples
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// # let mut dev = Device::null().open().unwrap();
    /// dev.par().mar([5, 4, 4, 2]).draw();
    /// assert_eq!(dev.mai().bottom, 5.0 * dev.line_height_inches());
    /// ```
    ///
    /// [`par("mai")`]: crate::device::Device::mai
    /// [`par(mai)`]: crate::par::Par::mai
    /// [`par(mar)`]: crate::par::Par::mar
    pub fn mai(&self) -> Sides<f64> {
        if let Some(mai) = self.par.mai {
            return mai;
        }
        let lh = self.line_height_inches();
        let m = self.par.mar;
        Sides::new(m.bottom * lh, m.left * lh, m.top * lh, m.right * lh)
    }

    /// Returns the outer margins in inches (bottom, left, top, right), from
    /// [`par(omd)`], else [`par(omi)`], else [`par(oma)`].
    ///
    /// This is R's [`par("omi")`].
    ///
    /// # Examples
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// # let mut dev = Device::null().open().unwrap();
    /// dev.par().omi([0, 0, 1, 0]).draw();
    /// assert_eq!(dev.omi().top, 1.0);
    /// ```
    ///
    /// [`par("omi")`]: crate::device::Device::omi
    /// [`par(oma)`]: crate::par::Par::oma
    /// [`par(omd)`]: crate::par::Par::omd
    /// [`par(omi)`]: crate::par::Par::omi
    pub fn omi(&self) -> Sides<f64> {
        if let Some([x0, x1, y0, y1]) = self.par.omd {
            let (w, h) = (self.width_in, self.height_in);
            return Sides::new(y0 * h, x0 * w, (1.0 - y1) * h, (1.0 - x1) * w);
        }
        if let Some(omi) = self.par.omi {
            return omi;
        }
        let lh = self.line_height_inches();
        let m = self.par.oma;
        Sides::new(m.bottom * lh, m.left * lh, m.top * lh, m.right * lh)
    }

    /// Font size in points for `par(ps) * par(cex) * cex`, after any
    /// rounding the backend's real-world counterpart applies (R's `pdf()`
    /// rounds to whole points; `svg()`/`png()` don't).
    fn font_size_pt(&self, cex: f64) -> f64 {
        self.backend
            .effective_font_size(self.par.ps * self.par.cex * cex)
    }

    // ---------------------------------------------------------------
    // Clipping, as R's `xpd`: FALSE clips to the plot region, TRUE to the
    // figure region, NA to the device (`axis()` and `mtext(outer = TRUE)`
    // always use the device, as R).
    // ---------------------------------------------------------------

    /// Where a call with an inline `xpd` clips: that, else [`par(xpd)`].
    ///
    /// [`par(xpd)`]: crate::par::Par::xpd
    pub(crate) fn clip_for(&self, xpd: Option<Xpd>) -> Xpd {
        xpd.unwrap_or(self.par.xpd)
    }

    /// Where margin text (`title()`, `mtext()`) clips: R raises `xpd` (the
    /// inline one, else [`par(xpd)`]) to at least `TRUE`, the figure region
    /// (`NA`, the device, stays), and `outer` text clips to the device.
    ///
    /// [`par(xpd)`]: crate::par::Par::xpd
    fn margin_clip(&self, xpd: Option<Xpd>, outer: bool) -> Xpd {
        match (outer, self.clip_for(xpd)) {
            (true, _) | (_, Xpd::Device) => Xpd::Device,
            _ => Xpd::Figure,
        }
    }

    /// Marks the current plot as drawn on, which resets [`par(new)`] (R's
    /// `GMode()`). Every drawing path goes through here, mostly via
    /// [`Device::push_clip_region`].
    ///
    /// [`par(new)`]: crate::par::ParOptions::new
    fn dirty(&mut self) {
        self.hold_figure = false;
    }

    fn push_clip_region(&mut self, clip: Xpd) {
        self.dirty();
        let rect = match clip {
            Xpd::Plot => self.plt,
            Xpd::Figure => self.fig,
            Xpd::Device => Rect::new(0.0, 0.0, self.width_in, self.height_in),
        };
        self.backend.push_clip(rect);
    }

    fn pop_clip(&mut self) {
        self.backend.pop_clip();
    }

    // ---------------------------------------------------------------
    // Primitives
    // ---------------------------------------------------------------

    fn resolve(&self, spec: ColorSpec) -> Option<Color> {
        if let ColorSpec::Index(i) = spec {
            if i < 0 {
                self.record_error(Error::invalid(format!(
                    "numerical color values must be >= 0, found {i}"
                )));
                return None;
            }
        }
        self.par.resolve_color(spec)
    }

    /// One symbol, as `GSymbol()`/`GESymbol()` draw it. `cex` is the
    /// per-point `cex` (R multiplies it by `cexbase`, i.e. [`par(cex)`]).
    ///
    /// [`par(cex)`]: crate::par::Par::cex
    fn draw_pch(&mut self, pch: i32, p: Point, cex: f64, col: Color, bg: Option<Color>, lwd: f64) {
        let cex = self.par.cex * cex;
        let size = pch::symbol_size_inches(self.start_ps, cex);
        if let Some(geom) = pch::pch_geometry(pch, p.x, p.y, size) {
            // `GSymbol()`: `par(lend/ljoin/lmitre)`, always a solid line.
            // Built as the constant default stroke unless `par()` differs,
            // which keeps the per-point path as fast as before.
            let mut stroke = Stroke::simple(col, lwd);
            let (lend, ljoin, lmitre) = (self.par.lend, self.par.ljoin, self.par.lmitre);
            if (lend, ljoin, lmitre) != (stroke.lend, stroke.ljoin, stroke.lmitre) {
                stroke.lend = lend;
                stroke.ljoin = ljoin;
                stroke.lmitre = lmitre;
            }
            let lines = |d: &mut Self| {
                for &(a, b) in &geom.lines {
                    d.backend
                        .draw_line(Point::new(a.0, a.1), Point::new(b.0, b.1), &stroke);
                }
            };
            if geom.lines_first {
                lines(self);
            }
            for shape in &geom.outline_only {
                self.paint_shape(shape, None, Some(&stroke));
            }
            for shape in &geom.filled_with_col {
                self.paint_shape(shape, Some(col), None);
            }
            for shape in &geom.filled_and_stroked_with_col {
                self.paint_shape(shape, Some(col), Some(&stroke));
            }
            for shape in &geom.filled_with_bg {
                self.paint_shape(shape, bg, Some(&stroke));
            }
            if !geom.lines_first {
                lines(self);
            }
        } else if pch == '.' as i32 {
            // A filled 0.01in square scaled by `cex`, but at least one
            // device unit across.
            if cex <= 0.0 {
                return;
            }
            let xc = (cex * 0.005).max(0.5 * self.backend.device_unit_inches());
            self.backend.draw_rect(
                Rect::new(p.x - xc, p.y - xc, p.x + xc, p.y + xc),
                Some(col),
                None,
            );
        } else if (32..=126).contains(&pch) || pch <= -32 {
            // `GEText(x, y, str, hadj = NA, yc = NA)`: centred on the
            // advance width, and vertically on the glyph's own bounding box.
            // A negative `pch` is minus a Unicode code point, as in R.
            let code = pch.unsigned_abs();
            let ch = char::from_u32(code).unwrap_or('?').to_string();
            let face = self.par.font;
            let size_pt = self.backend.effective_font_size(self.par.ps * cex);
            if size_pt <= 0.0 {
                return;
            }
            let style = TextStyle::new(col, face, size_pt);
            let w = afm::str_width_inches(&ch, face, size_pt);
            let (asc, desc) = afm::glyph_ascent_descent_inches(&ch, face, size_pt);
            let origin = Point::new(p.x - 0.5 * w, p.y - 0.5 * (asc - desc));
            self.backend.draw_text(origin, &ch, &style);
        }
    }

    fn paint_shape(&mut self, shape: &pch::Shape, fill: Option<Color>, stroke: Option<&Stroke>) {
        match shape {
            pch::Shape::Circle { cx, cy, r } => {
                self.backend
                    .draw_circle(Point::new(*cx, *cy), *r, fill, stroke);
            }
            pch::Shape::Rect { x0, y0, x1, y1 } => {
                self.backend
                    .draw_rect(Rect::new(*x0, *y0, *x1, *y1), fill, stroke);
            }
            pch::Shape::Polygon(pts) => {
                let pts: Vec<Point> = pts.iter().map(|&(x, y)| Point::new(x, y)).collect();
                self.backend.draw_polygon(&pts, fill, stroke);
            }
        }
    }

    /// `lines()`: a polyline, broken into separate pieces at non-finite
    /// (`NA`) coordinates, and at values <= 0 on a log axis, matching R.
    fn polyline_xy(&mut self, xs: &[f64], ys: &[f64], stroke: &Stroke, clip: Xpd) {
        let runs = self.finite_runs(xs, ys);
        self.push_clip_region(clip);
        for (_, run) in runs.iter().filter(|(_, run)| run.len() >= 2) {
            self.backend.draw_polyline(run, stroke);
        }
        self.pop_clip();
    }

    /// The lines of `plot(type = "b")` (and `"c"`): each segment between
    /// consecutive points, shortened at both ends by half a character
    /// height (`0.5 * par(cex) * 1.2 * startps / 72` inches) so the points stand
    /// clear, and left out when shorter than twice that. A port of
    /// `C_plotXY`'s `case 'b'`.
    fn broken_lines_xy(&mut self, xs: &[f64], ys: &[f64], stroke: &Stroke, clip: Xpd) {
        let d = 0.5 * self.par.cex * 1.2 * self.start_ps / 72.0;
        let runs = self.finite_runs(xs, ys);
        self.push_clip_region(clip);
        for (_, run) in &runs {
            for w in run.windows(2) {
                let (a, b) = (w[0], w[1]);
                let len = (b.x - a.x).hypot(b.y - a.y);
                if d < 0.5 * len {
                    let f = d / len;
                    let from = Point::new(a.x + f * (b.x - a.x), a.y + f * (b.y - a.y));
                    let to = Point::new(b.x + f * (a.x - b.x), b.y + f * (a.y - b.y));
                    self.backend.draw_line(from, to, stroke);
                }
            }
        }
        self.pop_clip();
    }

    /// `type = "h"`: a vertical line from each point to y = 0 (on a log
    /// y axis, to the bottom of the plot), colours recycled per point and
    /// lines in a missing or transparent colour skipped. A port of
    /// `C_plotXY`'s `case 'h'`.
    fn histogram_xy(
        &mut self,
        xs: &[f64],
        ys: &[f64],
        col: &[ColorSpec],
        lwd: f64,
        lty: LineType,
        clip: Xpd,
    ) {
        let default = ColorSpec::Color(self.par.col);
        // R keeps `usr` in data units internally, so on a log axis its
        // lower y limit is the bottom of the plot.
        let (ux0, _, uy0, _) = self.usr;
        let base = if self.ylog {
            self.usr_to_inches(ux0, uy0).y
        } else {
            self.usr_to_inches(ux0, 0.0).y
        };
        self.push_clip_region(clip);
        for (i, (&x, &y)) in xs.iter().zip(ys).enumerate() {
            let Some(p) = self.finite_point(x, y) else {
                continue;
            };
            let Some(c) = self.resolve(pick(col, i, default)) else {
                continue;
            };
            let stroke = self.stroke_of(c, lwd, lty);
            self.backend.draw_line(Point::new(p.x, base), p, &stroke);
        }
        self.pop_clip();
    }

    /// `type = "s"` (`vertical_first` false: across, then up or down) and
    /// `"S"` (up or down, then across): a staircase through the points,
    /// one polyline per run of finite points. A port of `C_plotXY`'s
    /// `case 's'` and `case 'S'`.
    fn steps_xy(
        &mut self,
        xs: &[f64],
        ys: &[f64],
        vertical_first: bool,
        stroke: &Stroke,
        clip: Xpd,
    ) {
        let runs = self.finite_runs(xs, ys);
        self.push_clip_region(clip);
        for (_, run) in runs.iter().filter(|(_, run)| run.len() >= 2) {
            let mut steps = vec![run[0]];
            for w in run.windows(2) {
                let (a, b) = (w[0], w[1]);
                steps.push(if vertical_first {
                    Point::new(a.x, b.y)
                } else {
                    Point::new(b.x, a.y)
                });
                steps.push(b);
            }
            self.backend.draw_polyline(&steps, stroke);
        }
        self.pop_clip();
    }

    /// `(x, y)` in device inches, or `None` when that isn't finite (`NA`,
    /// or a value <= 0 on a log axis): R's drawing skips such points.
    pub(crate) fn finite_point(&self, x: f64, y: f64) -> Option<Point> {
        Some(self.data_to_inches(x, y)).filter(|p| p.is_finite())
    }

    /// The runs of consecutive points of `xs`/`ys` that are finite in
    /// device inches (see [`Device::finite_point`]), each with the index
    /// of its first point.
    pub(crate) fn finite_runs(&self, xs: &[f64], ys: &[f64]) -> Vec<(usize, Vec<Point>)> {
        let mut runs = Vec::new();
        let mut run: Vec<Point> = Vec::new();
        let n = xs.len().min(ys.len());
        for i in 0..=n {
            match (i < n).then(|| self.finite_point(xs[i], ys[i])).flatten() {
                Some(p) => run.push(p),
                None if !run.is_empty() => runs.push((i - run.len(), std::mem::take(&mut run))),
                None => {}
            }
        }
        runs
    }

    fn stroke_of(&self, color: Color, lwd: f64, lty: LineType) -> Stroke {
        Stroke {
            color,
            lwd,
            lty,
            lend: self.par.lend,
            ljoin: self.par.ljoin,
            lmitre: self.par.lmitre,
        }
    }

    fn draw_line_if_finite(&mut self, from: Point, to: Point, stroke: &Stroke) {
        if from.is_finite() && to.is_finite() {
            self.backend.draw_line(from, to, stroke);
        }
    }

    // ---------------------------------------------------------------
    // Text
    // ---------------------------------------------------------------

    /// Starts measuring how wide a string or plotmath expression is when
    /// drawn: set it with [`s`](StrSizeCall::s), then get the width with
    /// [`get`](StrSizeCall::get).
    ///
    /// The width is in data units along the x axis by default (see
    /// [`units`](StrSizeCall::units)), at the size `text()` would draw it
    /// (`par(ps) * par(cex) * cex` points, after the backend's font-size
    /// rounding). For text with several lines, the widest line counts.
    ///
    /// This is R's `strwidth()`, for one string at a time.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// # let mut dev = Device::null().open().unwrap();
    /// # dev.plot().x([1, 10]).y([1, 10]).draw();
    /// let w = dev.strwidth().s("Title").cex(1.2).get(); // in x data units
    /// let w_in = dev.strwidth().s("Title").cex(1.2).units("inches").get();
    /// assert!(w > 0.0 && w_in > 0.0);
    /// ```
    pub fn strwidth(&mut self) -> StrSizeCall<'_, B> {
        StrSizeCall::start(self, false)
    }

    /// Starts measuring how tall a string or plotmath expression is when
    /// drawn; as [`strwidth`](Device::strwidth).
    ///
    /// For text, the height is that of "M", plus one line height
    /// (`par(lheight) * 1.2 * ps / 72` inches, times the size factors) per
    /// `\n`; for an expression, its height above and depth below the
    /// baseline. In data units along the y axis by default.
    ///
    /// This is R's `strheight()`, for one string at a time.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// # let mut dev = Device::null().open().unwrap();
    /// # dev.plot().x([1, 10]).y([1, 10]).draw();
    /// let one = dev.strheight().s("a").units("inches").get();
    /// let two = dev.strheight().s("a\nb").units("inches").get();
    /// assert!(two > one);
    /// ```
    pub fn strheight(&mut self) -> StrSizeCall<'_, B> {
        StrSizeCall::start(self, true)
    }

    /// Width and height in inches of text in `face` at `cex` (times
    /// [`par(cex)`]): R's `GEStrWidth`/`GEStrHeight`.
    ///
    /// [`par(cex)`]: crate::par::Par::cex
    pub(crate) fn text_size_inches(&self, text: &str, face: FontFace, cex: f64) -> (f64, f64) {
        let size = self.font_size_pt(cex);
        let width = text
            .split('\n')
            .map(|line| afm::str_width_inches(line, face, size))
            .fold(0.0, f64::max);
        let newlines = text.matches('\n').count() as f64;
        (
            width,
            newlines * self.text_line_height(cex) + afm::m_height_inches(face, size),
        )
    }

    /// The distance in inches between the baselines of multi-line text at
    /// `cex` (times [`par(cex)`]): [`par(lheight)`] times 1.2 times the
    /// unrounded font size, as R's `GEText()` and `GEStrHeight()`.
    ///
    /// [`par(cex)`]: crate::par::Par::cex
    /// [`par(lheight)`]: crate::par::Par::lheight
    fn text_line_height(&self, cex: f64) -> f64 {
        self.par.lheight * 1.2 * self.par.ps * self.par.cex * cex / 72.0
    }

    /// Draws text at `anchor` (device inches), clipped to `clip`, as
    /// `GEText()`: each line of a string with newlines is drawn on its own,
    /// [`Device::text_line_height`] apart, the block placed so that `vadj`
    /// (`None`: 0.5) of it is below the anchor and each line justified by
    /// `hadj` (`NaN`: 0.5); one line is drawn by
    /// [`Device::text_line_at_clipped`].
    #[allow(clippy::too_many_arguments)]
    fn text_at_clipped(
        &mut self,
        clip: Xpd,
        anchor: Point,
        label: &str,
        hadj: f64,
        vadj: Option<f64>,
        rotation_deg: f64,
        col: ColorSpec,
        cex: f64,
        font: FontFace,
    ) {
        if !label.contains('\n') {
            return self.text_line_at_clipped(
                clip,
                anchor,
                label,
                hadj,
                vadj,
                rotation_deg,
                col,
                cex,
                font,
            );
        }
        let n = label.split('\n').count() as f64;
        let xc = if hadj.is_finite() { hadj } else { 0.5 };
        let yc = vadj.filter(|v| v.is_finite()).unwrap_or(0.5);
        let step = self.text_line_height(cex);
        let theta = rotation_deg.to_radians();
        let (c, s) = (theta.cos(), theta.sin());
        for (i, line) in label.split('\n').enumerate() {
            let off = ((1.0 - yc) * (n - 1.0) - i as f64) * step;
            let at = Point::new(anchor.x - off * s, anchor.y + off * c);
            let (vadj, rot) = (Some(yc), rotation_deg);
            self.text_line_at_clipped(clip, at, line, xc, vadj, rot, col, cex, font);
        }
    }

    /// Draws one line of text at `anchor` (device inches), clipped to
    /// `clip`. Placement follows `GEText()`: horizontally `hadj` of the
    /// string's width; vertically, a finite `vadj` puts the baseline
    /// `vadj * height("M")` below the anchor, while `None` (`NA`) centres the ink
    /// of the string's glyphs on it (baseline at `anchor - (ury + lly) / 2`
    /// of the glyph bounding boxes).
    #[allow(clippy::too_many_arguments)]
    fn text_line_at_clipped(
        &mut self,
        clip: Xpd,
        anchor: Point,
        label: &str,
        hadj: f64,
        vadj: Option<f64>,
        rotation_deg: f64,
        col: ColorSpec,
        cex: f64,
        font: FontFace,
    ) {
        let Some(color) = self.resolve(col) else {
            return;
        };
        if label.is_empty() {
            return;
        }
        let face = font;
        let size_pt = self.font_size_pt(cex);
        if size_pt <= 0.0 {
            return;
        }
        let width = afm::str_width_inches(label, face, size_pt);
        let dy = match vadj {
            Some(yc) => -yc * afm::m_height_inches(face, size_pt),
            None => {
                let (asc, desc) = afm::glyph_ascent_descent_inches(label, face, size_pt);
                -0.5 * (asc - desc)
            }
        };
        let dx = -hadj * width;
        let theta = rotation_deg.to_radians();
        let (c, s) = (theta.cos(), theta.sin());
        let origin = Point::new(anchor.x + dx * c - dy * s, anchor.y + dx * s + dy * c);
        let style = TextStyle {
            rotation_deg,
            ..TextStyle::new(color, face, size_pt)
        };
        self.push_clip_region(clip);
        self.backend.draw_text(origin, label, &style);
        self.pop_clip();
    }

    // ---------------------------------------------------------------
    // title()
    // ---------------------------------------------------------------

    /// `GMtext()`'s `yLineBias` nudge (`graphics.c`; [`par(ylbias)`]), in
    /// margin lines, for text on `side`: `1 - bias` on sides 1 and 4, `bias`
    /// on 2 and 3, divided
    /// by [`par(mex)`] as R does (so the nudge stays the same size in inches);
    /// 0 for text perpendicular to the side. Shared by `title()`, `axis()`
    /// and `mtext()`.
    ///
    /// [`par(mex)`]: crate::par::Par::mex
    /// [`par(ylbias)`]: crate::par::Par::ylbias
    pub(crate) fn line_bias(&self, side: Side, parallel: bool) -> f64 {
        if !parallel {
            return 0.0;
        }
        let bias = match side {
            Side::Bottom | Side::Right => 1.0 - self.par.ylbias,
            Side::Left | Side::Top => self.par.ylbias,
        };
        bias / self.par.mex
    }

    /// Draws R's `title()`; the public entry is
    /// [`Graphics::title`], whose docs
    /// describe it.
    pub(crate) fn title_impl(&mut self, opts: &TitleOptions) {
        let lh = self.line_height_inches();
        // `C_title`: every title sits at `adj` along its side and is
        // justified by `adj` (`GMtext` uses `par(adj)` in all cases); along
        // the plot region (NPC), or with `outer` the inner region (NIC).
        let adj = opts.adj.unwrap_or(self.par.adj);
        let mgp = opts.mgp.unwrap_or(self.par.mgp);
        let line = opts.line.filter(|l| l.is_finite());
        let region = if opts.outer { self.inner } else { self.plt };
        let clip = self.margin_clip(opts.xpd, opts.outer);
        let x_at = region.x0 + adj * region.width();
        let y_at = region.y0 + adj * region.height();
        if let Some(main) = &opts.main {
            // `GText(hpos, vpos, MAR3/OMA3, .., adj, adjy)`: at `line` with
            // the baseline on it (`adjy = 0`), else half the top margin
            // (`0.5 * mar[2]` or `oma[2]`) with `adjy = 0.5` (finite, so
            // `GEText` uses `yc * height(M)`, not exact ink centring). An
            // expression is always centred (`GMathText(.., adj, 0.5)`).
            let (y, adjy) = match line {
                Some(l) => (region.y1 + l * lh, 0.0),
                None => {
                    let margin = if opts.outer {
                        self.height_in - self.inner.y1
                    } else {
                        self.mar.top * lh
                    };
                    (region.y1 + 0.5 * margin, 0.5)
                }
            };
            let adjy = if matches!(main, Label::Math(_)) {
                0.5
            } else {
                adjy
            };
            self.draw_label(
                clip,
                Point::new(x_at, y),
                main,
                adj,
                Some(adjy),
                0.0,
                opts.col_main.unwrap_or(ColorSpec::Color(self.par.col_main)),
                opts.cex_main.unwrap_or(self.par.cex_main),
                opts.font_main.unwrap_or(self.par.font_main),
            );
        }
        // The rest via `GMtext()`: at `line`, else `mgp[0]` (+ 1 for
        // `sub`), nudged by `yLineBias` (see `line_bias`).
        if let Some(sub) = &opts.sub {
            let l = line.unwrap_or(mgp[0] + 1.0) + self.line_bias(Side::Bottom, true);
            self.draw_label(
                clip,
                Point::new(x_at, region.y0 - l * lh),
                sub,
                adj,
                Some(0.0),
                0.0,
                opts.col_sub.unwrap_or(ColorSpec::Color(self.par.col_sub)),
                opts.cex_sub.unwrap_or(self.par.cex_sub),
                opts.font_sub.unwrap_or(self.par.font_sub),
            );
        }
        if let Some(xlab) = &opts.xlab {
            let l = line.unwrap_or(mgp[0]) + self.line_bias(Side::Bottom, true);
            self.draw_label(
                clip,
                Point::new(x_at, region.y0 - l * lh),
                xlab,
                adj,
                Some(0.0),
                0.0,
                opts.col_lab.unwrap_or(ColorSpec::Color(self.par.col_lab)),
                opts.cex_lab.unwrap_or(self.par.cex_lab),
                opts.font_lab.unwrap_or(self.par.font_lab),
            );
        }
        if let Some(ylab) = &opts.ylab {
            let l = line.unwrap_or(mgp[0]) + self.line_bias(Side::Left, true);
            self.draw_label(
                clip,
                Point::new(region.x0 - l * lh, y_at),
                ylab,
                adj,
                Some(0.0),
                90.0,
                opts.col_lab.unwrap_or(ColorSpec::Color(self.par.col_lab)),
                opts.cex_lab.unwrap_or(self.par.cex_lab),
                opts.font_lab.unwrap_or(self.par.font_lab),
            );
        }
    }
}

/// Default numeric axis label formatting, as R's `axis()` does it.
///
/// The tick vector is formatted *jointly*, like R's `formatReal()` (7
/// significant digits): all labels share the same number of decimals
/// (`0.0 0.5 1.0`, not `0 0.5 1`), and scientific notation is chosen when
/// it is strictly narrower than fixed notation (`0 1e+05 2e+05`).
pub(crate) fn format_axis_labels(vals: &[f64]) -> Vec<String> {
    // (significant digits, decimal exponent) of each finite value.
    let sci: Vec<Option<(i32, i32)>> = vals
        .iter()
        .map(|&v| {
            if !v.is_finite() {
                return None;
            }
            if v == 0.0 {
                return Some((1, 0));
            }
            let s = format!("{:.6e}", v.abs());
            let (mant, exp) = s.split_once('e')?;
            let digits = mant.replace('.', "");
            let nsig = digits.trim_end_matches('0').len().max(1) as i32;
            Some((nsig, exp.parse().ok()?))
        })
        .collect();
    let finite: Vec<(i32, i32)> = sci.iter().flatten().copied().collect();
    if finite.is_empty() {
        return vals.iter().map(|v| format!("{v}")).collect();
    }
    let rgt = finite.iter().map(|&(s, e)| s - e - 1).max().unwrap().max(0);
    let left = finite.iter().map(|&(_, e)| e + 1).max().unwrap().max(1);
    let mxsl = finite.iter().map(|&(s, _)| s).max().unwrap();
    let max_abs_exp = finite.iter().map(|&(_, e)| e.abs()).max().unwrap();
    let neg = vals.iter().any(|&v| v < 0.0) as i32;
    let fixed_w = neg + left + if rgt > 0 { rgt + 1 } else { 0 };
    let exp_w = if max_abs_exp >= 100 { 5 } else { 4 };
    let sci_w = neg + if mxsl > 1 { mxsl + 1 } else { mxsl } + exp_w;
    vals.iter()
        .map(|&v| {
            if !v.is_finite() {
                return format!("{v}");
            }
            if fixed_w <= sci_w {
                format!("{:.*}", rgt as usize, if v == 0.0 { 0.0 } else { v })
            } else {
                let s = format!("{:.*e}", (mxsl - 1) as usize, v);
                let (m, e) = s.split_once('e').unwrap();
                let e: i32 = e.parse().unwrap();
                format!("{m}e{}{:02}", if e < 0 { '-' } else { '+' }, e.abs())
            }
        })
        .collect()
}

/// A temporary change to a device's `par`, as R's `op <- par(...);
/// on.exit(par(op))`: `restore` copies the changed fields back from the
/// saved `par` when the scope ends, also after an early return or a panic.
/// Derefs to the device.
pub(crate) struct ParScope<'a, B: Backend> {
    dev: &'a mut Device<B>,
    saved: Par,
    restore: fn(&mut Par, &Par),
}

impl<B: Backend> Device<B> {
    /// Starts a [`ParScope`]; `restore` copies back what the caller is
    /// about to change.
    pub(crate) fn par_scope(&mut self, restore: fn(&mut Par, &Par)) -> ParScope<'_, B> {
        let saved = self.par.clone();
        ParScope {
            dev: self,
            saved,
            restore,
        }
    }
}

impl<B: Backend> Device<B> {
    /// R's inline `lend`/`ljoin`/`lmitre` for one call (`ProcessInlinePars`,
    /// undone by `GRestorePars`): every line drawn in the scope uses them.
    /// `par.font` is restored too, so callers may set an inline `font` in
    /// the scope (as `points()` does for character symbols).
    ///
    /// # Errors
    /// If `lmitre` is below 1 ([`check_lmitre`]).
    pub(crate) fn line_pars(
        &mut self,
        lend: Option<LineEnd>,
        ljoin: Option<LineJoin>,
        lmitre: Option<f64>,
    ) -> Result<ParScope<'_, B>, Error> {
        check_lmitre(lmitre)?;
        let mut dev = self.par_scope(|now, saved| {
            now.lend = saved.lend;
            now.ljoin = saved.ljoin;
            now.lmitre = saved.lmitre;
            now.font = saved.font;
        });
        if let Some(l) = lend {
            dev.par.lend = l;
        }
        if let Some(j) = ljoin {
            dev.par.ljoin = j;
        }
        if let Some(m) = lmitre {
            dev.par.lmitre = m;
        }
        Ok(dev)
    }
}

impl<B: Backend> Deref for ParScope<'_, B> {
    type Target = Device<B>;
    fn deref(&self) -> &Device<B> {
        self.dev
    }
}

impl<B: Backend> DerefMut for ParScope<'_, B> {
    fn deref_mut(&mut self) -> &mut Device<B> {
        self.dev
    }
}

impl<B: Backend> Drop for ParScope<'_, B> {
    fn drop(&mut self) {
        (self.restore)(&mut self.dev.par, &self.saved);
    }
}

/// `GNewPlot()`'s checks (`graphics.c`), in its order: R's error message
/// when the figure region `fig` (fractions of the inner region) or the plot
/// region `plt` (fractions of the figure) is invalid. Region sizes are
/// absolute, as R's unit conversions take them, so outer margins wider
/// than the device leave a small figure whose margins don't fit ("figure
/// margins too large").
fn region_error(fig: [f64; 4], plt: [f64; 4]) -> Option<&'static str> {
    let eps = f64::from(f32::EPSILON);
    let within = |r: [f64; 4]| r[0] > -eps && r[1] < 1.0 + eps && r[2] > -eps && r[3] < 1.0 + eps;
    let ordered = |r: [f64; 4]| r[0] < r[1] && r[2] < r[3];
    if !ordered(fig) {
        Some("outer margins too large (figure region too small)")
    } else if !within(fig) {
        Some("figure region too large")
    } else if !ordered(plt) {
        Some("figure margins too large")
    } else if !within(plt) {
        Some("plot region too large")
    } else {
        None
    }
}

/// The linear map between one axis's `usr` range (`u0..u1`, log10 units
/// on a log axis) and the plot region's extent along it in device inches
/// (`d0..d1`).
#[derive(Clone, Copy)]
pub(crate) struct AxisMap {
    pub u0: f64,
    pub u1: f64,
    pub d0: f64,
    pub d1: f64,
}

impl AxisMap {
    /// A `usr` value in inches.
    pub(crate) fn inches(self, u: f64) -> f64 {
        self.d0 + (u - self.u0) / (self.u1 - self.u0) * (self.d1 - self.d0)
    }

    /// A position in inches in `usr` units.
    pub(crate) fn usr(self, d: f64) -> f64 {
        self.u0 + (d - self.d0) / (self.d1 - self.d0) * (self.u1 - self.u0)
    }
}

/// Whether text drawn with `las` on `side` is perpendicular to the side,
/// as R's `GMtext()` decides: `las` 2 always, 1 (horizontal text) on the
/// left and right, 3 (vertical text) at the bottom and top. Shared by
/// `axis()` and `mtext()`.
pub(crate) fn las_perpendicular(side: Side, las: i32) -> bool {
    match las {
        1 => !side.is_horizontal(),
        2 => true,
        3 => side.is_horizontal(),
        _ => false,
    }
}

/// The rotation, in degrees, of text drawn with `las` on `side`: 90 for
/// vertical text.
pub(crate) fn las_rotation(side: Side, las: i32) -> f64 {
    if side.is_horizontal() == las_perpendicular(side, las) {
        90.0
    } else {
        0.0
    }
}

/// `ComputePAdjValue()`: the default `padj` for `side` and `las`, 0.5
/// (centred) for text perpendicular to the side, else 0.
pub(crate) fn default_padj(side: Side, las: i32) -> f64 {
    if las_perpendicular(side, las) {
        0.5
    } else {
        0.0
    }
}

/// R's check of `par(fig = )`: `0 <= x0 < x1 <= 1`, `0 <= y0 < y1 <= 1`.
pub(crate) fn check_fig(fig: [f64; 4]) -> Result<(), Error> {
    let [x0, x1, y0, y1] = fig;
    if !(0.0 <= x0 && x0 < x1 && x1 <= 1.0 && 0.0 <= y0 && y0 < y1 && y1 <= 1.0) {
        return Err(Error::invalid(
            "invalid value specified for graphical parameter \"fig\"",
        ));
    }
    Ok(())
}

/// R's check of `par(fin = )`: both sizes positive.
pub(crate) fn check_fin(fin: [f64; 2]) -> Result<(), Error> {
    if !(fin.iter().all(|&v| v > 0.0 && v.is_finite())) {
        return Err(Error::invalid(
            "invalid value specified for graphical parameter \"fin\"",
        ));
    }
    Ok(())
}

/// R's check of `par(mfrow/mfcol = )`: at least one row and column.
pub(crate) fn check_grid(name: &str, nrow: usize, ncol: usize) -> Result<(), Error> {
    if nrow == 0 || ncol == 0 {
        return Err(Error::invalid(format!(
            "invalid value specified for graphical parameter \"{name}\""
        )));
    }
    Ok(())
}

/// R's check of `par(xaxp/yaxp = )` on a log axis (`logAxpCheck`): the
/// code `axp[2]`, as an integer, is at most 4 and not 0 (negative is a
/// linear count).
pub(crate) fn check_log_axp(name: &str, axp: [f64; 3]) -> Result<(), Error> {
    let code = axp[2].trunc();
    if !code.is_finite() || code == 0.0 || code > 4.0 {
        return Err(Error::invalid(format!(
            "invalid value specified for graphical parameter \"{name}\""
        )));
    }
    Ok(())
}

/// R's check of `lmitre` (in `par()` or inline): at least 1.
pub(crate) fn check_lmitre(lmitre: Option<f64>) -> Result<(), Error> {
    match lmitre {
        Some(m) if m.is_nan() || m < 1.0 => Err(Error::invalid(
            "invalid value specified for graphical parameter \"lmitre\"",
        )),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::format_axis_labels;

    fn f(v: &[f64]) -> Vec<String> {
        format_axis_labels(v)
    }

    #[test]
    fn region_errors_follow_r_order() {
        use super::region_error;
        let ok = [0.0, 1.0, 0.0, 1.0];
        let plt = [0.2, 0.8, 0.2, 0.8];
        assert_eq!(region_error(ok, plt), None);
        // `GNewPlot()` checks the figure before the plot region, and order
        // before extent; R's absolute unit sizes make the first one hard to
        // reach from `par()`, so it's checked here.
        assert_eq!(
            region_error([0.6, 0.4, -1.0, 2.0], [0.8, 0.2, 0.0, 2.0]),
            Some("outer margins too large (figure region too small)")
        );
        assert_eq!(
            region_error([-1.0, 2.0, 0.0, 1.0], [0.8, 0.2, 0.0, 2.0]),
            Some("figure region too large")
        );
        assert_eq!(
            region_error(ok, [0.8, 0.2, 0.0, 2.0]),
            Some("figure margins too large")
        );
        assert_eq!(
            region_error(ok, [0.0, 2.0, 0.0, 1.0]),
            Some("plot region too large")
        );
        // R's FLT_EPSILON tolerance at the edges.
        assert_eq!(
            region_error([-1e-9, 1.0 + 1e-9, 0.0, 1.0], [-1e-9, 1.0, 0.0, 1.0]),
            None
        );
    }

    #[test]
    fn axis_labels_share_decimals() {
        assert_eq!(f(&[0.0, 0.5, 1.0]), ["0.0", "0.5", "1.0"]);
        assert_eq!(f(&[0.0, 2.0, 4.0]), ["0", "2", "4"]);
        assert_eq!(f(&[-1.0, -0.5, 0.0]), ["-1.0", "-0.5", "0.0"]);
        assert_eq!(f(&[0.0, 0.25, 0.5]), ["0.00", "0.25", "0.50"]);
    }

    #[test]
    fn axis_labels_scientific_when_narrower() {
        assert_eq!(f(&[0.0, 1e5, 2e5]), ["0e+00", "1e+05", "2e+05"]);
        assert_eq!(f(&[0.0, 50000.0, 100000.0]), ["0e+00", "5e+04", "1e+05"]);
        assert_eq!(f(&[0.0, 2000.0, 4000.0]), ["0", "2000", "4000"]);
    }

    #[test]
    fn inches_to_user_inverts_user_to_inches() {
        use super::{Device, PlotWindowOptions, Side};

        let mut dev = Device::memory("pdf").width(7.0).height(5.0).open().unwrap();
        dev.plot_new_impl().unwrap();
        assert!(!dev.xlog() && !dev.ylog());
        let log_x = PlotWindowOptions {
            xlog: true,
            ..PlotWindowOptions::default()
        };
        dev.plot_window_impl((1.0, 1000.0), (-2.0, 3.0), &log_x);
        assert!(dev.xlog() && !dev.ylog());
        for (x, y) in [(1.0, -2.0), (31.6, 0.25), (1000.0, 3.0)] {
            let (x2, y2) = dev.inches_to_data(dev.data_to_inches(x, y));
            assert!(
                (x2 - x).abs() < 1e-9 * x && (y2 - y).abs() < 1e-9,
                "{x}, {y}"
            );
        }
        assert_eq!(Side::try_from(3), Ok(Side::Top));
        assert_eq!(Side::Right.number(), 4);
    }

    #[test]
    fn side_numbers_outside_1_to_4_are_errors() {
        let e = super::Side::try_from(5).unwrap_err();
        assert_eq!(e.message(), "invalid axis number 5");
    }
}
