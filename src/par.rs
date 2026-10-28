// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! The graphical parameters ([`Par`]), the changes one `par()` call makes
//! ([`ParOptions`]), and their value types ([`Sides`], [`AxisStyle`],
//! [`Pty`], [`Bty`], [`Xpd`], [`AxisType`]).
//!
//! [`Par`] holds the subset of R's graphics parameters this crate
//! implements, with R's exact default values (cross-checked against
//! `Rscript -e 'str(par())'` and `r-source/src/library/graphics/src/par.c`).
//!
//! `cex` scales both text *and* the "line" unit that margins are measured
//! in, as in R, where [`par(cex)`] sets the base `cex` (`cexbase`) and resets
//! the per-call `cex` to 1, and a margin line is `mex * cexbase` times the
//! character height (`mapNDC2Dev()`). Multi-panel layouts (`par()`'s
//! `mfrow`/`mfcol`, and `layout()`) set `cex` (0.83 for 2 x 2, 0.66
//! beyond that) and `mex` just as R's `par(mfrow=)` does.
//!
//! [`par(cex)`]: crate::par::Par::cex

use crate::color::{Color, ColorSpec, DEFAULT_PALETTE};
use crate::font::FontFace;
use crate::lty::LineType;

use super::backend::{LineEnd, LineJoin};

/// One value per side of a region, in R's `mar`/`oma` order: bottom, left,
/// top, right.
///
/// Converts from a `[bottom, left, top, right]` array, so margin setters
/// take arrays.
///
/// # Examples
///
/// ```
/// # use rs_graphics::prelude::*;
/// let mar: Sides<f64> = [5, 4, 4, 2].into();
/// assert_eq!(mar, Sides::new(5.0, 4.0, 4.0, 2.0));
/// # let mut dev = Device::null().open().unwrap();
/// dev.par().mar([5, 4, 1, 1]).draw();
/// assert_eq!(dev.current_par().mar.top, 1.0);
/// ```
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Sides<T> {
    /// Side 1, the bottom.
    pub bottom: T,
    /// Side 2, the left.
    pub left: T,
    /// Side 3, the top.
    pub top: T,
    /// Side 4, the right.
    pub right: T,
}

impl<T: Copy> Sides<T> {
    /// Returns the sides with these values, in R's order.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// let oma = Sides::new(0.0, 0.0, 2.0, 0.0);
    /// assert_eq!(oma.top, 2.0);
    /// ```
    pub const fn new(bottom: T, left: T, top: T, right: T) -> Self {
        Sides {
            bottom,
            left,
            top,
            right,
        }
    }
}

impl<T, U: Into<T>> From<[U; 4]> for Sides<T> {
    /// The sides from `[bottom, left, top, right]`, R's order for
    /// `mar = c(...)`; any numbers for a `Sides<f64>` (`[5, 4, 4, 2]`).
    fn from([bottom, left, top, right]: [U; 4]) -> Self {
        Sides {
            bottom: bottom.into(),
            left: left.into(),
            top: top.into(),
            right: right.into(),
        }
    }
}

/// How an axis range is set from its limits: extended by 4% or exact.
///
/// Only `"r"` (the default, 4% range expansion) and `"i"` (exact, no
/// expansion) are implemented. R's `par()` also accepts `"s"`, `"e"` and
/// `"d"`, but then errors when plotting ("axis style unimplemented"); here
/// they are treated as `"r"`. Setters take the strings.
///
/// This is R's `par(xaxs = )` / `par(yaxs = )`.
///
/// # Examples
///
/// ```
/// # use rs_graphics::prelude::*;
/// # let mut dev = Device::null().open().unwrap();
/// dev.par().xaxs("i").draw();
/// dev.plot_window().xlim(0, 10).ylim(0, 10).draw();
/// let (x0, x1, y0, _) = dev.usr();
/// assert_eq!((x0, x1), (0.0, 10.0)); // exact
/// assert!(y0 < 0.0); // yaxs "r": extended
/// ```
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum AxisStyle {
    /// The range extended by 4% at each end (the default). R's `"r"`.
    Regular,
    /// The range exactly as given. R's `"i"`.
    Exact,
}

impl AxisStyle {
    /// Parses an axis style string, or returns `None` if it isn't one.
    ///
    /// `"i"` is [`AxisStyle::Exact`]; `"r"`, `"s"`, `"e"` and `"d"` are
    /// [`AxisStyle::Regular`] (R accepts the last three in `par()` but errors
    /// when plotting). `None` for anything else (R: "invalid 'xaxs' value").
    ///
    /// These are R's `xaxs`/`yaxs` values.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// assert_eq!(AxisStyle::parse("i"), Some(AxisStyle::Exact));
    /// assert_eq!(AxisStyle::parse("x"), None);
    /// ```
    pub fn parse(s: &str) -> Option<AxisStyle> {
        match s {
            "i" => Some(AxisStyle::Exact),
            "r" | "s" | "e" | "d" => Some(AxisStyle::Regular),
            _ => None,
        }
    }
}

crate::error::from_str_via_parse!(AxisStyle, "axis style");
crate::error::from_r_string!(AxisStyle, "axis style", "\"r\" or \"i\"");

/// The shape of the plot region when it comes from the margins: maximal or
/// square.
///
/// Setters take R's strings, `"m"` and `"s"`.
///
/// This is R's `par(pty = )`.
///
/// # Examples
///
/// ```
/// # use rs_graphics::prelude::*;
/// # let mut dev = Device::null().width(7).height(5).open().unwrap();
/// dev.par().pty("s").draw();
/// dev.plot_new();
/// let r = dev.plot_region();
/// assert!((r.width() - r.height()).abs() < 1e-9);
/// ```
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Pty {
    /// As large as the margins allow (the default). R's `"m"`.
    Maximal,
    /// Square, the longer side shrunk about its centre. R's `"s"`.
    Square,
}

impl Pty {
    /// Parses `"m"` or `"s"`, or returns `None` for anything else (R:
    /// "invalid value specified for graphical parameter \"pty\"").
    ///
    /// # Examples
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// assert_eq!(Pty::parse("s"), Some(Pty::Square));
    /// assert_eq!(Pty::parse("q"), None);
    /// ```
    pub fn parse(s: &str) -> Option<Pty> {
        match s {
            "m" => Some(Pty::Maximal),
            "s" => Some(Pty::Square),
            _ => None,
        }
    }
}

crate::error::from_str_via_parse!(Pty, "pty");
crate::error::from_r_string!(Pty, "pty", "\"m\" or \"s\"");

/// Which sides of the plot region `box()` (and `plot()`'s frame) draws.
///
/// It applies to the plot region only, as in R; `box()` with
/// `which = "figure"` and the other regions always draws all four sides.
/// Setters take R's strings (`"l"`, `"n"`, ...).
///
/// This is R's `par(bty = )`.
///
/// # Examples
///
/// ```
/// # use rs_graphics::prelude::*;
/// # let mut dev = Device::null().open().unwrap();
/// dev.plot().x([1, 2, 3]).y([3, 1, 2]).bty("l").draw();
/// dev.par().bty("n").draw();
/// assert_eq!(dev.current_par().bty, Bty::N);
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Bty {
    /// All four sides (the default). R's `"o"`.
    #[default]
    O,
    /// Left and bottom. R's `"l"`.
    L,
    /// Top and right. R's `"7"`.
    Seven,
    /// Top, left and bottom. R's `"c"` (or `"["`).
    C,
    /// Left, bottom and right. R's `"u"`.
    U,
    /// Top, right and bottom. R's `"]"`.
    Bracket,
    /// None. R's `"n"`.
    N,
}

impl Bty {
    /// Parses a box type string from its first character, or returns `None`
    /// if it isn't one.
    ///
    /// Takes `"o"`, `"l"`, `"7"`, `"c"` or `"["`, `"u"`, `"]"`, `"n"`
    /// (letters in either case). `None` for anything else (R: "invalid
    /// value specified for graphical parameter \"bty\"").
    ///
    /// These are R's `bty` strings, read as R does.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// assert_eq!(Bty::parse("L"), Some(Bty::L));
    /// assert_eq!(Bty::parse("["), Some(Bty::C));
    /// assert_eq!(Bty::parse("x"), None);
    /// ```
    pub fn parse(s: &str) -> Option<Bty> {
        match s.chars().next()? {
            'o' | 'O' => Some(Bty::O),
            'l' | 'L' => Some(Bty::L),
            '7' => Some(Bty::Seven),
            'c' | 'C' | '[' => Some(Bty::C),
            'u' | 'U' => Some(Bty::U),
            ']' => Some(Bty::Bracket),
            'n' | 'N' => Some(Bty::N),
            _ => None,
        }
    }
}

crate::error::from_str_via_parse!(Bty, "box type");
crate::error::from_r_string!(
    Bty,
    "box type",
    "one of \"o\", \"l\", \"7\", \"c\", \"u\", \"]\" or \"n\""
);

/// The region drawing is clipped to: the plot region, the figure region or
/// the whole device.
///
/// `true`/`false` convert, so `.xpd(true)` is R's `xpd = TRUE`.
///
/// This is R's `par(xpd = )`.
///
/// # Examples
///
/// ```
/// # use rs_graphics::prelude::*;
/// # let mut dev = Device::null().open().unwrap();
/// # dev.plot_new();
/// dev.par().xpd(true).draw();
/// assert_eq!(dev.current_par().xpd, Xpd::Figure);
/// dev.text().x(1.1).y(0.5).labels("in the margin").xpd(Xpd::Device).draw();
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Xpd {
    /// The plot region (the default). R's `FALSE`.
    #[default]
    Plot,
    /// The figure region. R's `TRUE`.
    Figure,
    /// The whole device. R's `NA`.
    Device,
}

impl From<bool> for Xpd {
    /// The clip region for a logical `xpd`: `false` is [`Xpd::Plot`],
    /// `true` [`Xpd::Figure`], as in R.
    fn from(xpd: bool) -> Self {
        if xpd { Xpd::Figure } else { Xpd::Plot }
    }
}

/// Whether `axis()` draws on an axis: standard, or suppressed.
///
/// Suppressing the default axis is how a custom one is drawn instead (a
/// `plot()` with `xaxt("n")` before an `axis()` on side 1). R's `"s"`,
/// `"l"` and `"t"` all mean [`AxisType::Standard`]; `"n"` is
/// [`AxisType::None`]. Setters take the strings.
///
/// This is R's `par(xaxt = )` / `par(yaxt = )`.
///
/// # Examples
///
/// ```
/// # use rs_graphics::prelude::*;
/// # let mut dev = Device::null().open().unwrap();
/// dev.plot().x([1, 2, 3]).y([3, 1, 2]).xaxt("n").draw();
/// dev.axis().side(1).at([1, 2, 3]).labels(["a", "b", "c"]).draw();
/// ```
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum AxisType {
    /// The axis is drawn (the default). R's `"s"` (also `"l"`, `"t"`).
    Standard,
    /// The axis isn't drawn. R's `"n"`.
    None,
}

impl AxisType {
    /// Parses an axis type string, or returns `None` if it isn't one.
    ///
    /// `"n"` is [`AxisType::None`]; `"s"`, `"l"` and `"t"` are
    /// [`AxisType::Standard`]. `None` for anything else.
    ///
    /// These are R's `xaxt`/`yaxt` values.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// assert_eq!(AxisType::parse("n"), Some(AxisType::None));
    /// assert_eq!(AxisType::parse("t"), Some(AxisType::Standard));
    /// assert_eq!(AxisType::parse("y"), None);
    /// ```
    pub fn parse(s: &str) -> Option<AxisType> {
        match s {
            "n" => Some(AxisType::None),
            "s" | "l" | "t" => Some(AxisType::Standard),
            _ => None,
        }
    }
}

crate::error::from_str_via_parse!(AxisType, "axis type");
crate::error::from_r_string!(AxisType, "axis type", "\"s\" or \"n\"");

// `Par` and `ParOptions` from one list of `Par`'s fields: `ParOptions`
// holds every field as an `Option` (`None`: unchanged), with its `apply`
// and setters (on `ParOptions` and `ParCall`), each documented with the
// `Par` field's doc. Each field gives its setter kind and type after `=>`;
// `@Some` marks a `Par` field of type `Option<T>` that a `par()` call can
// only set to `Some` (as R, which resets them by setting the alternative,
// e.g. `mar` for `mai`).
macro_rules! par_options {
    (@wrap [] $e:expr) => { $e };
    (@wrap [Some] $e:expr) => { Some($e) };
    (
        $(#[$sattr:meta])*
        pub struct Par {
            $(
                $(#[$fattr:meta])*
                pub $field:ident : $fty:ty => $(@ $some:ident)? $kind:ident $t:ty
            ),* $(,)?
        }
    ) => {
        $(#[$sattr])*
        pub struct Par {
            $( $(#[$fattr])* pub $field: $fty, )*
        }

        $crate::setters::options! {
            /// The graphical parameters one `par()` call changes: every [`Par`] field
            /// as an `Option` (`None`: unchanged), plus `new`, `usr`, `xlog` and `ylog`,
            /// which are device state.
            ///
            /// Build one to reuse a set of parameters with `.with(&opts)`. The page
            /// layout (`mfrow`, `mfcol`, `mfg`, `fig`, `fin`) isn't here: it belongs to
            /// the device, so only a device's `par()` call takes it
            /// ([`ParCall::mfrow`](crate::graphics::ParCall::mfrow) and co.).
            ///
            /// # Examples
            ///
            /// ```
            /// # use rs_graphics::prelude::*;
            /// let compact = ParOptions::default().mar([4, 4, 1, 1]).cex_axis(0.8);
            /// let mut dev = Device::new(SvgBackend::new(7, 7), Par::default());
            /// dev.par().with(&compact).draw();
            /// ```
            #[derive(Clone, Debug, Default, PartialEq)]
            pub struct ParOptions => [ParCall] {
                $(
                    $(#[$fattr])*
                    #[doc = ""]
                    #[doc = concat!("New value of [`Par::", stringify!($field), "`]",
                        $(" (set to `", stringify!($some), "`)",)? "; `None`: unchanged.")]
                    pub $field: Option<$t> => $kind $t,
                )*
                /// Overlays the next plot: with `true`, the next `plot_new()` draws
                /// into the current figure without advancing or clearing it
                /// (e.g. for a second `plot_window()` with an axis on side 4). As
                /// in R, drawing anything (other than `mtext(outer = TRUE)`)
                /// resets it to `false`, `plot_new()` itself does not, and it is
                /// ignored (R warns) before anything is plotted.
                pub new: Option<bool> => option bool,
                /// User coordinates of the plot region, `[x0, x1, y0, y1]`, in
                /// log10 units on a log axis (as [`par("usr")`] reads). The default
                /// tick settings (`xaxp`/`yaxp`) are worked out again from them.
                /// Applied after the other fields and `xlog`/`ylog` in the same
                /// call, so a `usr` given with them is in their units, uses a new
                /// `lab`, and replaces an `xaxp`/`yaxp` given there. A value
                /// `NaN`, `x0 == x1` or `y0 == y1` is an error of the call (R's
                /// "invalid value specified for graphical parameter \"usr\"").
                ///
                /// [`par("usr")`]: crate::device::Device::usr
                pub usr: Option<[f64; 4]> => array [f64; 4],
                /// Makes the x axis logarithmic, or linear again: from now on, x
                /// values are mapped through `log10` first. As in R, `usr` stays
                /// as it is, so set it too. The next `plot_new()` resets it.
                pub xlog: Option<bool> => option bool,
                /// As [`xlog`](ParOptions::xlog), for the y axis.
                pub ylog: Option<bool> => option bool,
            }
        }

        impl ParOptions {
            /// Sets the given fields of `par` (all but `new`, `usr`, `xlog`
            /// and `ylog`, which are device state).
            pub(crate) fn apply(&self, par: &mut Par) {
                $(
                    if let Some(v) = &self.$field {
                        par.$field = par_options!(@wrap [$($some)?] v.clone());
                    }
                )*
                self.apply_units(par);
            }
        }

        impl ParOptions {
            /// R's checks of `usr`, made when the call is drawn or recorded.
            pub(crate) fn check(&self) -> Result<(), $crate::error::Error> {
                if let Some([x0, x1, y0, y1]) = self.usr {
                    let ok = ![x0, x1, y0, y1].iter().any(|v| v.is_nan()) && x0 != x1 && y0 != y1;
                    if !ok {
                        return Err($crate::error::Error::invalid(
                            "invalid value specified for graphical parameter \"usr\"",
                        ));
                    }
                }
                $crate::device::check_lmitre(self.lmitre)?;
                Ok(())
            }
        }
    };
}

par_options! {
    /// The graphical parameters: margins, colours, fonts, line styles, axis
    /// styles and the rest of what `par()` sets.
    ///
    /// A device starts with the `Par` it was made with (usually
    /// `Par::default()`, R's defaults); change it with
    /// [`par()`](crate::graphics::Graphics::par) and read it with
    /// [`Device::current_par`](crate::device::Device::current_par). Field names
    /// match R's where practical (`cex_main` is R's `cex.main`).
    ///
    /// This is R's `par()` state, restricted to what this crate implements.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// let mut dev = Device::new(SvgBackend::new(7, 7), Par::default());
    /// dev.par().las(1).cex_axis(0.8).draw();
    /// assert_eq!(dev.current_par().las, 1);
    /// ```
    #[derive(Clone, Debug)]
    pub struct Par {
        /// Margins around the plot region, in lines: bottom, left, top,
        /// right. Default `c(5.1, 4.1, 4.1, 2.1)`. Used unless `mai` is set.
        /// Applied at the next `plot_new()`.
        pub mar: Sides<f64> => option Sides<f64>,
        /// Margins in inches, instead of lines. `None` (the default): use
        /// `mar`. Unlike `mar`, they don't change with `cex` or `mex`.
        /// Applied at the next `plot_new()`.
        pub mai: Option<Sides<f64>> => @Some option Sides<f64>,
        /// Outer margins, in lines. Default `c(0, 0, 0, 0)`. Used unless `omi`
        /// or `omd` is set. Applied at the next `plot_new()`.
        pub oma: Sides<f64> => option Sides<f64>,
        /// Outer margins in inches, instead of lines. `None` (the default):
        /// use `oma`. Applied at the next `plot_new()`.
        pub omi: Option<Sides<f64>> => @Some option Sides<f64>,
        /// Inner region (the device less the outer margins) as fractions of
        /// the device, `[x0, x1, y0, y1]`. `None` (the default): use `omi` or
        /// `oma`; takes precedence over both. Applied at the next
        /// `plot_new()`.
        pub omd: Option<[f64; 4]> => @Some array [f64; 4],
        /// Plot region as fractions of the figure region, `[x0, x1, y0, y1]`,
        /// instead of the figure less `mar`. `None` (the default): use `mar`
        /// and `pty`. A `par()` call setting `mar`, `mai` or `pty` resets it
        /// to `None`, as in R. Applied at the next `plot_new()`.
        pub plt: Option<[f64; 4]> => option Option<[f64; 4]>,
        /// Plot region size in inches, `[width, height]`, centred in the
        /// figure, instead of the figure less `mar`. `None` (the default): use
        /// `plt`, or `mar` and `pty`; takes precedence over `plt`. A `par()`
        /// call setting `mar`, `mai`, `pty` or `plt` resets it to `None`, as
        /// in R. Applied at the next `plot_new()`.
        pub pin: Option<[f64; 2]> => @Some array [f64; 2],
        /// Shape of the plot region: [`Pty::Square`] makes the `mar`-based
        /// region square. Default [`Pty::Maximal`]. Applied at the next
        /// `plot_new()`.
        pub pty: Pty => option Pty,
        /// Which sides of the plot region `box()` and `plot()`'s frame draw.
        /// Default [`Bty::O`] (all four).
        pub bty: Bty => option Bty,
        /// Range style of the x axis: [`AxisStyle::Regular`] (`"r"`, the
        /// default) extends the range by 4% at each end, [`AxisStyle::Exact`]
        /// (`"i"`) uses it as given.
        pub xaxs: AxisStyle => option AxisStyle,
        /// Range style of the y axis, as `xaxs`.
        pub yaxs: AxisStyle => option AxisStyle,

        /// Default colour of points, lines, `abline()` and `box()`.
        pub col: Color => option Color,
        /// Background colour: of new pages, of the default `legend()` box
        /// fill, and of colour number 0.
        pub bg: Color => option Color,
        /// Default colour of axes, segments, arrows and rectangle/polygon
        /// borders. Unlike R's `par(fg = )`, doesn't set `col` too: set both
        /// for the same effect.
        pub fg: Color => option Color,
        /// Colour of axis tick labels. R's `col.axis`.
        pub col_axis: Color => option Color,
        /// Colour of the axis titles (`xlab`/`ylab`). R's `col.lab`.
        pub col_lab: Color => option Color,
        /// Colour of the main title. R's `col.main`.
        pub col_main: Color => option Color,
        /// Colour of the subtitle. R's `col.sub`.
        pub col_sub: Color => option Color,

        /// Colours that colour numbers (`col = 2`) index, from 1. Default
        /// [`DEFAULT_PALETTE`]. R's `palette()`.
        pub palette: Vec<Color> => option Vec<Color>,

        /// Line width; 1 is 1/96 inch, as in R. Default 1.
        pub lwd: f64 => option f64,
        /// Line type. Default solid.
        pub lty: LineType => option LineType,
        /// Line end style. Default round.
        pub lend: LineEnd => option LineEnd,
        /// Line join style. Default round.
        pub ljoin: LineJoin => option LineJoin,
        /// Mitre limit for mitred joins. Default 10.
        pub lmitre: f64 => option f64,

        /// Default plotting symbol. Default 1 (open circle).
        pub pch: i32 => option i32,

        /// Base text size, in big points. Default 12. Its value when the
        /// device is created is the device's starting size
        /// ([`DeviceBuilder::pointsize`](crate::device::DeviceBuilder::pointsize)
        /// or `ps` passed to `Device::new`; R's `pdf(pointsize = )`), from
        /// which margin lines and symbols are sized; changing it later only
        /// changes text size, as in R.
        pub ps: f64 => option f64,
        /// Base magnification of text and symbols; also scales the margin
        /// "line" unit (see [`Device::line_height_inches`](crate::device::Device::line_height_inches)).
        /// Default 1. R's `cexbase`.
        pub cex: f64 => option f64,
        /// Expansion factor for margin lines, a multiple of `cex`. Default 1.
        pub mex: f64 => option f64,
        /// Size of axis tick labels, relative to `cex`. Default 1. R's
        /// `cex.axis`.
        pub cex_axis: f64 => option f64,
        /// Size of the axis titles (`xlab`/`ylab`), relative to `cex`.
        /// Default 1. R's `cex.lab`.
        pub cex_lab: f64 => option f64,
        /// Size of the main title, relative to `cex`. Default 1.2. R's
        /// `cex.main`.
        pub cex_main: f64 => option f64,
        /// Size of the subtitle, relative to `cex`. Default 1. R's `cex.sub`.
        pub cex_sub: f64 => option f64,

        /// Font face of text ([`FontFace`]; the setter takes R's numbers 1 to
        /// 5, the names or the enum). Default plain.
        pub font: FontFace => option FontFace,
        /// Font face of axis tick labels, as `font`. Default plain. R's
        /// `font.axis`.
        pub font_axis: FontFace => option FontFace,
        /// Font face of the axis titles (`xlab`/`ylab`), as `font`. Default
        /// plain. R's `font.lab`.
        pub font_lab: FontFace => option FontFace,
        /// Font face of the main title, as `font`. Default bold. R's
        /// `font.main`.
        pub font_main: FontFace => option FontFace,
        /// Font face of the subtitle, as `font`. Default plain. R's
        /// `font.sub`.
        pub font_sub: FontFace => option FontFace,

        /// Orientation of axis labels and `mtext()` text: 0 = parallel to the
        /// axis (the default), 1 = horizontal, 2 = perpendicular to the axis,
        /// 3 = vertical.
        pub las: i32 => option i32,
        /// Whether `axis()` draws x axes (sides 1 and 3): [`AxisType::None`]
        /// stops it. Default [`AxisType::Standard`].
        pub xaxt: AxisType => option AxisType,
        /// As `xaxt`, for y axes (sides 2 and 4).
        pub yaxt: AxisType => option AxisType,
        /// Tick mark length, in margin lines; negative points outwards.
        /// Default -0.5, as in R. Used when `tck` is `None`.
        pub tcl: f64 => option f64,
        /// Tick mark length as a fraction of the plot region (if above 0.5,
        /// of its height for an x axis, so `1` draws grid lines; else of its
        /// smaller side); positive points inwards. Overrides `tcl`. `None`
        /// (the default, R's `NA`): use `tcl`. The `par()` setter takes a
        /// number; `f64::NAN` sets it back to `None`.
        pub tck: Option<f64> => na Option<f64>,
        /// Default rotation of `text()` labels, in degrees anticlockwise; also
        /// applies to `legend()` and `pie()` labels, which R draws with
        /// `text()`. Default 0.
        pub srt: f64 => option f64,
        /// Margin lines of the axis title, the tick labels and the axis line.
        /// Default `c(3, 1, 0)`.
        pub mgp: [f64; 3] => array [f64; 3],
        /// Approximate number of tick intervals on the x and y axes for
        /// default ticks; the third value (label length) is unused, as in R.
        /// Default `c(5, 5, 7)`.
        pub lab: [i32; 3] => option [i32; 3],
        /// First and last default x tick and the number of intervals between
        /// them (on a log axis, a tick-rule code instead: see
        /// [`crate::pretty::axis_pars`]). `plot_new()`, `plot_window()` and
        /// `par(usr = )` set it from the axis range and `lab`; change it
        /// afterwards to change the ticks `axis()` draws by default.
        /// Initially `c(0, 1, 5)`.
        pub xaxp: [f64; 3] => array [f64; 3],
        /// As `xaxp`, for the y axis.
        pub yaxp: [f64; 3] => array [f64; 3],
        /// Multiplies the spacing of the lines of text with newlines, when
        /// drawn and in [`Device::strheight`](crate::device::Device::strheight).
        /// Default 1.
        pub lheight: f64 => option f64,
        /// How far text parallel to its side is moved out from its margin
        /// line: `1 - ylbias` lines on sides 1 and 4, `ylbias` on sides 2 and
        /// 3 (divided by `mex`). Used by axis labels, `mtext()` and
        /// `title()`'s `sub`/`xlab`/`ylab`. Default 0.2, as R's pdf, png and
        /// svg devices.
        pub ylbias: f64 => option f64,
        /// Default text justification: 0 left, 0.5 centred (the default), 1
        /// right.
        pub adj: f64 => option f64,
        /// Region drawing is clipped to: [`Xpd::Plot`] (the default; R's
        /// `FALSE`), [`Xpd::Figure`] (`TRUE`) or [`Xpd::Device`] (`NA`). Also
        /// sets how far `abline()` lines, and `axis()` ticks outside the plot
        /// range, reach.
        pub xpd: Xpd => option Xpd,
        /// Whether `plot()` draws its titles and axis labels; also the
        /// default of `barplot()`'s `ann` (with `!add`). Default `true`. As in
        /// R, `hist()` and `boxplot()` have their own `ann` options that don't
        /// depend on it, and `pie()` always draws `main`.
        pub ann: bool => option bool,
    }
}

impl Default for Par {
    fn default() -> Self {
        Par {
            mar: Sides::new(5.1, 4.1, 4.1, 2.1),
            mai: None,
            oma: Sides::new(0.0, 0.0, 0.0, 0.0),
            omi: None,
            omd: None,
            plt: None,
            pin: None,
            pty: Pty::Maximal,
            bty: Bty::O,
            xaxs: AxisStyle::Regular,
            yaxs: AxisStyle::Regular,

            col: Color::BLACK,
            bg: Color::WHITE,
            fg: Color::BLACK,
            col_axis: Color::BLACK,
            col_lab: Color::BLACK,
            col_main: Color::BLACK,
            col_sub: Color::BLACK,

            palette: DEFAULT_PALETTE.to_vec(),

            lwd: 1.0,
            lty: LineType::SOLID,
            lend: LineEnd::Round,
            ljoin: LineJoin::Round,
            lmitre: 10.0,

            pch: 1,

            ps: 12.0,
            cex: 1.0,
            mex: 1.0,
            cex_axis: 1.0,
            cex_lab: 1.0,
            cex_main: 1.2,
            cex_sub: 1.0,

            font: FontFace::Plain,
            font_axis: FontFace::Plain,
            font_lab: FontFace::Plain,
            font_main: FontFace::Bold,
            font_sub: FontFace::Plain,

            las: 0,
            xaxt: AxisType::Standard,
            yaxt: AxisType::Standard,
            tcl: -0.5,
            tck: None,
            srt: 0.0,
            mgp: [3.0, 1.0, 0.0],
            lab: [5, 5, 7],
            xaxp: [0.0, 1.0, 5.0],
            yaxp: [0.0, 1.0, 5.0],
            lheight: 1.0,
            ylbias: 0.2,
            adj: 0.5,
            xpd: Xpd::Plot,
            ann: true,
        }
    }
}

impl Par {
    /// Returns the colour to draw `spec` with, or `None` for nothing.
    ///
    /// Colour 0 is the background, `bg`; like any fully transparent colour it
    /// draws nothing when `bg` is transparent (the PDF default). Numbers index
    /// `palette`.
    ///
    /// R's colour 0 is the background too.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// let par = Par::default();
    /// assert_eq!(par.resolve_color(ColorSpec::Index(2)), Some(par.palette[1]));
    /// ```
    ///
    /// # Panics
    ///
    /// If `spec` is a negative palette number.
    pub fn resolve_color(&self, spec: ColorSpec) -> Option<Color> {
        match spec {
            ColorSpec::Index(0) => Some(self.bg).filter(|c| c.a > 0),
            _ => spec.resolve(&self.palette),
        }
    }
}

impl ParOptions {
    /// R keeps one unit per region (`mUnits`, `oUnits`, `pUnits`,
    /// `defaultPlot`), and the one set last wins. A `par()` call setting
    /// one form clears the others it replaces; when one call sets several,
    /// the more specific wins (`mai` over `mar`, `omd` over `omi` over
    /// `oma`, `pin` over `plt` over the margins).
    fn apply_units(&self, par: &mut Par) {
        // `mar`/`mai`/`pty` set `defaultPlot = TRUE`: the plot region
        // comes from the margins again.
        if (self.mar.is_some() || self.mai.is_some() || self.pty.is_some())
            && self.plt.is_none()
            && self.pin.is_none()
        {
            par.plt = None;
            par.pin = None;
        }
        if self.plt.is_some() && self.pin.is_none() {
            par.pin = None;
        }
        if self.mar.is_some() && self.mai.is_none() {
            par.mai = None;
        }
        if self.oma.is_some() && self.omi.is_none() && self.omd.is_none() {
            par.omi = None;
            par.omd = None;
        }
        if self.omi.is_some() && self.omd.is_none() {
            par.omd = None;
        }
    }

    /// Whether this call sets the outer margins, which in R starts a new
    /// page at the next `plot.new()` ("force eject of multiple figures").
    pub(crate) fn sets_outer_margins(&self) -> bool {
        self.oma.is_some() || self.omi.is_some() || self.omd.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sides_from_array_is_bottom_left_top_right() {
        assert_eq!(
            Sides::from([5.1, 4.1, 4.1, 2.1]),
            Sides::new(5.1, 4.1, 4.1, 2.1)
        );
    }

    #[test]
    fn par_enums_reject_what_r_rejects() {
        assert_eq!(AxisStyle::parse("i"), Some(AxisStyle::Exact));
        assert_eq!(AxisStyle::parse("d"), Some(AxisStyle::Regular));
        assert_eq!(AxisStyle::parse("x"), None);
        assert_eq!(Pty::parse("m"), Some(Pty::Maximal));
        assert_eq!(Pty::parse("q"), None);
        assert_eq!("t".parse::<AxisType>(), Ok(AxisType::Standard));
        assert!("y".parse::<AxisType>().is_err());
    }

    #[test]
    fn par_options_apply_only_what_is_set() {
        let mut par = Par::default();
        let opts = ParOptions::default()
            .las(1)
            .mar([1.0, 2.0, 3.0, 4.0])
            .plt([0.1, 0.9, 0.2, 0.8]);
        opts.apply(&mut par);
        assert_eq!(par.las, 1);
        assert_eq!(par.mar.left, 2.0);
        assert_eq!(par.plt, Some([0.1, 0.9, 0.2, 0.8]));
        assert_eq!(par.cex, Par::default().cex);

        // `plt` back to the mar-based region.
        ParOptions::default().plt(None).apply(&mut par);
        assert_eq!(par.plt, None);
    }
}
