// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! `par()` state: the subset of R's graphics parameters this crate
//! implements, with R's exact default values (cross-checked against
//! `Rscript -e 'str(par())'` and `r-source/src/library/graphics/src/par.c`).
//!
//! `par(cex)` is R's `cexbase` (assigning it resets the per-call `cex` to
//! 1), so `cex` here scales both text *and* the "line" unit that margins
//! are measured in, exactly as `mapNDC2Dev()` does (`yNDCPerLine = mex *
//! cexbase * cra[1] / ...`). Multi-panel layouts
//! (`Device::mfrow()`/`mfcol()`/`layout()`) set `cex` (0.83 for 2 x 2, 0.66
//! beyond that) and `mex` just as R's `par(mfrow=)` does.

use crate::afm::FontFace;
use crate::color::{Color, ColorSpec, DEFAULT_PALETTE};
use crate::lty::LineType;

use super::backend::{LineEnd, LineJoin};

/// The four sides in R's `mar`/`oma` order: bottom, left, top, right.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Sides<T> {
    pub bottom: T,
    pub left: T,
    pub top: T,
    pub right: T,
}

impl<T: Copy> Sides<T> {
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
    /// `[bottom, left, top, right]`, R's order for `mar = c(...)`; any
    /// numbers for a `Sides<f64>` (`[5, 4, 4, 2]`).
    fn from([bottom, left, top, right]: [U; 4]) -> Self {
        Sides {
            bottom: bottom.into(),
            left: left.into(),
            top: top.into(),
            right: right.into(),
        }
    }
}

/// Axis style (`par(xaxs=)`/`par(yaxs=)`). Only `"r"` (the default, 4%
/// range expansion) and `"i"` (exact, no expansion) are implemented. R's
/// `par()` also accepts `"s"`, `"e"` and `"d"`, but `GScale()` then errors
/// ("axis style unimplemented"); here they are treated as `"r"`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum AxisStyle {
    Regular,
    Exact,
}

impl AxisStyle {
    /// R's `xaxs`/`yaxs` values: `"i"` is [`AxisStyle::Exact`]; `"r"`,
    /// `"s"`, `"e"` and `"d"` are [`AxisStyle::Regular`] (R accepts them
    /// in `par()` but errors when plotting). `None` for
    /// anything else (R: "invalid 'xaxs' value").
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

/// `par(pty)`: the shape of the plot region when it comes from `mar`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Pty {
    /// `"m"`: as large as the margins allow (the default).
    Maximal,
    /// `"s"`: square, the longer side shrunk about its centre.
    Square,
}

impl Pty {
    /// `"m"` or `"s"`; `None` for anything else (R: "invalid value
    /// specified for graphical parameter \"pty\"").
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

/// `par(bty)`: which sides of the plot region `box()` (and `plot()`'s
/// frame) draws. As R's `GBox()`, it applies to the plot region only;
/// `box(which = "figure")` and the others always draw all four sides.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Bty {
    /// `"o"`: all four sides (the default).
    #[default]
    O,
    /// `"l"`: left and bottom.
    L,
    /// `"7"`: top and right.
    Seven,
    /// `"c"` (or `"["`): top, left and bottom.
    C,
    /// `"u"`: left, bottom and right.
    U,
    /// `"]"`: top, right and bottom.
    Bracket,
    /// `"n"`: none.
    N,
}

impl Bty {
    /// R's `bty` strings, read as R does from their first character:
    /// `"o"`, `"l"`, `"7"`, `"c"` or `"["`, `"u"`, `"]"`, `"n"` (letters
    /// in either case). `None` for anything else (R: "invalid value
    /// specified for graphical parameter \"bty\"").
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

/// `par(xpd)`: where drawing is clipped to. `true`/`false` convert, so
/// `.xpd(true)` is R's `xpd = TRUE`.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Xpd {
    /// R's `FALSE` (the default): the plot region.
    #[default]
    Plot,
    /// R's `TRUE`: the figure region.
    Figure,
    /// R's `NA`: the whole device.
    Device,
}

impl From<bool> for Xpd {
    /// `false` is [`Xpd::Plot`], `true` [`Xpd::Figure`], as R's logical
    /// `xpd`.
    fn from(xpd: bool) -> Self {
        if xpd { Xpd::Figure } else { Xpd::Plot }
    }
}

/// `par(xaxt/yaxt)`: whether `axis()` draws on that axis. R's `"s"`, `"l"`
/// and `"t"` all mean [`AxisType::Standard`]; `"n"` suppresses the axis
/// (`plot(x, y, xaxt = "n")` before a custom `axis(1, ...)`).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum AxisType {
    Standard,
    None,
}

impl AxisType {
    /// `"n"` is [`AxisType::None`]; `"s"`, `"l"` and `"t"` are
    /// [`AxisType::Standard`]. `None` for anything else.
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

/// The graphics parameter set (`par()`), restricted to what this crate
/// implements. Field names match R's own where practical.
#[derive(Clone, Debug)]
pub struct Par {
    /// Margins in lines: bottom, left, top, right. Default `c(5.1, 4.1,
    /// 4.1, 2.1)`. Used unless `mai` is set. Applied at the next
    /// `plot_new()`.
    pub mar: Sides<f64>,
    /// `par(mai)`: the margins in inches instead of lines; `None` (the
    /// default) uses `mar`. Unlike `mar`, they don't change with `cex` or
    /// `mex`. Applied at the next `plot_new()`.
    pub mai: Option<Sides<f64>>,
    /// Outer margins in lines. Default `c(0, 0, 0, 0)`. Used unless `omi`
    /// or `omd` is set. Applied at the next `plot_new()`.
    pub oma: Sides<f64>,
    /// `par(omi)`: the outer margins in inches; `None` (the default) uses
    /// `oma`. Applied at the next `plot_new()`.
    pub omi: Option<Sides<f64>>,
    /// `par(omd = c(x0, x1, y0, y1))`: the inner region (the device less
    /// the outer margins) as fractions of the device; `None` (the default)
    /// uses `omi` or `oma`. Takes precedence over both. Applied at the
    /// next `plot_new()`.
    pub omd: Option<[f64; 4]>,
    /// `par(plt = c(x0, x1, y0, y1))`: the plot region as fractions of the
    /// figure region, instead of the figure less `mar`. `None` (the default)
    /// uses `mar` and `pty`. A `par()` call setting `mar`, `mai` or `pty`
    /// resets it to `None`, as in R. Applied at the next `plot_new()`.
    pub plt: Option<[f64; 4]>,
    /// `par(pin = c(width, height))`: the plot region's size in inches,
    /// centred in the figure, instead of the figure less `mar`. `None`
    /// (the default) uses `plt`, or `mar` and `pty`; takes precedence over
    /// `plt`. A `par()` call setting `mar`, `mai`, `pty` or `plt` resets it
    /// to `None`, as in R. Applied at the next `plot_new()`.
    pub pin: Option<[f64; 2]>,
    /// `par(pty)`: [`Pty::Square`] makes the `mar`-based plot region
    /// square. Applied at the next `plot_new()`.
    pub pty: Pty,
    /// `par(bty)`: which sides of the plot region `box()` and `plot()`'s
    /// frame draw. Default [`Bty::O`] (all four).
    pub bty: Bty,
    /// `par(xaxs)`: x axis range style; [`AxisStyle::Regular`] (`"r"`,
    /// the default) extends the range by 4% on each side,
    /// [`AxisStyle::Exact`] (`"i"`) uses it as given.
    pub xaxs: AxisStyle,
    /// `par(yaxs)`: as `xaxs`, for the y axis.
    pub yaxs: AxisStyle,

    /// `par(col)`: default colour of points, lines, `abline()` and `box()`.
    pub col: Color,
    /// `par(bg)`: background colour (new pages, default `legend()` box
    /// fill, colour 0).
    pub bg: Color,
    /// `par(fg)`: default colour of axes, segments, arrows and
    /// rectangle/polygon borders. R's `par(fg = )` sets `col` too; here set
    /// both fields to do the same.
    pub fg: Color,
    /// `par(col.axis)`: colour of axis tick labels.
    pub col_axis: Color,
    /// `par(col.lab)`: colour of `xlab`/`ylab`.
    pub col_lab: Color,
    /// `par(col.main)`: colour of `main`.
    pub col_main: Color,
    /// `par(col.sub)`: colour of `sub`.
    pub col_sub: Color,

    /// R's `palette()`: the colours that numbers (`col = 2`) index,
    /// from 1. Default [`DEFAULT_PALETTE`].
    pub palette: Vec<Color>,

    /// `par(lwd)`: line width (1 = 1/96 inch, as in R). Default 1.
    pub lwd: f64,
    /// `par(lty)`: line type. Default solid.
    pub lty: LineType,
    /// `par(lend)`: line end style. Default round.
    pub lend: LineEnd,
    /// `par(ljoin)`: line join style. Default round.
    pub ljoin: LineJoin,
    /// `par(lmitre)`: mitre limit for mitred joins. Default 10.
    pub lmitre: f64,

    /// `par(pch)`: default plotting symbol. Default 1 (open circle).
    pub pch: i32,

    /// Base point size, in big points (`par(ps)`). Default `12`. Its value
    /// when the device is created is the device's starting size (R's
    /// `pdf(pointsize = )`, here [`DeviceBuilder::pointsize`](crate::device::DeviceBuilder::pointsize)
    /// or `ps` passed to `Device::new`), from which margin lines and
    /// symbols are sized; changing it later only changes text size, as in R.
    pub ps: f64,
    /// `par(cex)`, i.e. R's `cexbase`. Also scales the margin "line"
    /// unit (see [`Device::line_height_inches`](crate::device::Device::line_height_inches)).
    pub cex: f64,
    /// `par(mex)`: margin-line expansion factor (a multiple of `cex`).
    pub mex: f64,
    /// `par(cex.axis)`: size of axis tick labels, relative to `cex`.
    /// Default 1.
    pub cex_axis: f64,
    /// `par(cex.lab)`: size of `xlab`/`ylab`, relative to `cex`. Default
    /// 1.
    pub cex_lab: f64,
    /// `par(cex.main)`: size of `main`, relative to `cex`. Default 1.2.
    pub cex_main: f64,
    /// `par(cex.sub)`: size of `sub`, relative to `cex`. Default 1.
    pub cex_sub: f64,

    /// `par(font)`: font of text, as R's numbers: 1 plain (the default),
    /// 2 bold, 3 italic, 4 bold italic, 5 Symbol. Other numbers are drawn
    /// plain.
    pub font: i32,
    /// `par(font.axis)`: font of axis tick labels, as `font`. Default 1.
    pub font_axis: i32,
    /// `par(font.lab)`: font of `xlab`/`ylab`, as `font`. Default 1.
    pub font_lab: i32,
    /// `par(font.main)`: font of `main`, as `font`. Default 2 (bold).
    pub font_main: i32,
    /// `par(font.sub)`: font of `sub`, as `font`. Default 1.
    pub font_sub: i32,

    /// Label orientation (`par(las)`): 0 = parallel to axis, 1 =
    /// horizontal, 2 = perpendicular to axis, 3 = vertical.
    pub las: i32,
    /// `par(xaxt)` / `par(yaxt)`: [`AxisType::None`] stops `axis()`
    /// drawing on the x (sides 1, 3) / y (sides 2, 4) axis.
    pub xaxt: AxisType,
    /// `par(yaxt)`: see `xaxt`.
    pub yaxt: AxisType,
    /// Tick mark length in margin lines (negative = outward; R's default
    /// is -0.5). Used when `tck` is `None`.
    pub tcl: f64,
    /// `par(tck)`: tick length as a fraction of the plot region (of its
    /// height for an x axis if above 0.5, so `1` draws grid lines, else of
    /// its smaller side); positive points inwards. Overrides `tcl`.
    /// `None` (R's default `NA`): use `tcl`. The `par()` setter takes a
    /// number; `f64::NAN` sets it back to `NA`.
    pub tck: Option<f64>,
    /// `par(srt)`: default string rotation of `text()` (and so of
    /// `legend()` and `pie()` labels, which R draws with `text()`), in
    /// degrees counterclockwise. Default 0.
    pub srt: f64,
    /// Margin line positions for axis title/labels/line, `c(3, 1, 0)` by
    /// default.
    pub mgp: [f64; 3],
    /// `par(lab)`: the approximate number of tick intervals on the x and y
    /// axes for default ticks (the third value, label length, is unused,
    /// as in R). `c(5, 5, 7)` by default.
    pub lab: [i32; 3],
    /// `par(xaxp)` / `par(yaxp)`: the first and last default tick and the
    /// number of intervals between them (on a log axis, a tick-rule code
    /// instead: see [`crate::pretty::axis_pars`]). `plot_new()` and
    /// `plot_window()` set
    /// them from the axis range and `par(lab)`; change them afterwards to
    /// change the ticks `axis()` draws by default. `c(0, 1, 5)` initially.
    pub xaxp: [f64; 3],
    /// `par(yaxp)`: see `xaxp`.
    pub yaxp: [f64; 3],
    /// `par(lheight)`: multiplies the height of each extra line of text in
    /// [`Device::text_metrics`](crate::device::Device::text_metrics). (Text
    /// is drawn one line at a time, so this doesn't affect drawing.)
    pub lheight: f64,
    /// Default text justification (`par(adj)`).
    pub adj: f64,
    /// `par(xpd)`: where drawing is clipped to: [`Xpd::Plot`] (R's
    /// `FALSE`, the default), [`Xpd::Figure`] (`TRUE`) or [`Xpd::Device`]
    /// (`NA`). Also sets the extent of `abline()` lines and of `axis()`
    /// ticks outside the plot range, as R's `getxlimits()`.
    pub xpd: Xpd,
    /// `par(ann)`: whether `plot()` draws its titles and axis labels, and
    /// the default of `barplot()`'s `ann` (with `!add`). Default `true`. As
    /// in R, `hist()` and `boxplot()` have their own `ann` options that
    /// don't depend on it, and `pie()` always draws `main`.
    pub ann: bool,
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

            font: 1,
            font_axis: 1,
            font_lab: 1,
            font_main: 2,
            font_sub: 1,

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
            adj: 0.5,
            xpd: Xpd::Plot,
            ann: true,
        }
    }
}

impl Par {
    /// The colour to draw `spec` with, or `None` for nothing. Colour 0 is
    /// the background, `par.bg`, as in R (`RGBpar3`); like any fully
    /// transparent colour it draws nothing when `bg` is transparent (the
    /// PDF default).
    pub fn resolve_color(&self, spec: ColorSpec) -> Option<Color> {
        match spec {
            ColorSpec::Index(0) => Some(self.bg).filter(|c| c.a > 0),
            _ => spec.resolve(&self.palette),
        }
    }

    /// The face for R's font code `code` (1 plain, 2 bold, 3 italic, 4
    /// bold italic, 5 Symbol).
    pub(crate) fn font_face(&self, code: i32) -> FontFace {
        FontFace::from_r_font_code(code)
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

// `ParOptions`: every `Par` field as an `Option`, its `apply`, and its
// setters (on `ParOptions` and `ParCall`), from the field lists: `Par`
// fields of type `T` first, then (`optional:`) `Par` fields of type
// `Option<T>` that a `par()` call can only set to `Some` (as R, which
// resets them by setting the alternative, e.g. `mar` for `mai`).
macro_rules! par_options {
    (
        $($field:ident : $kind:ident $t:ty),* $(,)?
        ; optional: $($ofield:ident : $okind:ident $ot:ty),* $(,)?
    ) => {
        /// The [`Par`] fields a `par()` call changes (`None`: unchanged),
        /// plus `new` (R's `par(new = TRUE)`, see
        /// [`Device::set_new`](crate::device::Device::set_new)). R's
        /// `mfrow`/`mfcol` aren't here: they change the device's page
        /// layout, not `Par`, so only a device's `par()` call takes them
        /// ([`ParCall::mfrow`](crate::graphics::ParCall::mfrow)).
        #[derive(Clone, Debug, Default, PartialEq)]
        pub struct ParOptions {
            $(
                #[doc = concat!("New value of [`Par::", stringify!($field), "`].")]
                pub $field: Option<$t>,
            )*
            $(
                #[doc = concat!("New value of [`Par::", stringify!($ofield), "`] (set to `Some`).")]
                pub $ofield: Option<$ot>,
            )*
            /// `par(new = )`: draw the next plot over the current figure.
            pub new: Option<bool>,
        }

        impl ParOptions {
            /// Sets the given fields of `par` (all but `new`, which is
            /// device state).
            pub fn apply(&self, par: &mut Par) {
                $(
                    if let Some(v) = &self.$field {
                        par.$field = v.clone();
                    }
                )*
                $(
                    if let Some(v) = &self.$ofield {
                        par.$ofield = Some(v.clone());
                    }
                )*
                self.apply_units(par);
            }
        }

        $crate::setters::setters!(ParOptions => [ParCall] {
            $($field: $kind $t,)*
            $($ofield: $okind $ot,)*
            new: option bool,
        });
    };
}

par_options!(
    mar: option Sides<f64>,
    oma: option Sides<f64>,
    plt: option Option<[f64; 4]>,
    pty: option Pty,
    bty: option Bty,
    xaxs: option AxisStyle,
    yaxs: option AxisStyle,
    col: option Color,
    bg: option Color,
    fg: option Color,
    col_axis: option Color,
    col_lab: option Color,
    col_main: option Color,
    col_sub: option Color,
    palette: option Vec<Color>,
    lwd: option f64,
    lty: option LineType,
    lend: option LineEnd,
    ljoin: option LineJoin,
    lmitre: option f64,
    pch: option i32,
    ps: option f64,
    cex: option f64,
    mex: option f64,
    cex_axis: option f64,
    cex_lab: option f64,
    cex_main: option f64,
    cex_sub: option f64,
    font: option i32,
    font_axis: option i32,
    font_lab: option i32,
    font_main: option i32,
    font_sub: option i32,
    las: option i32,
    xaxt: option AxisType,
    yaxt: option AxisType,
    tcl: option f64,
    tck: na Option<f64>,
    srt: option f64,
    mgp: array [f64; 3],
    lab: option [i32; 3],
    xaxp: array [f64; 3],
    yaxp: array [f64; 3],
    lheight: option f64,
    adj: option f64,
    xpd: option Xpd,
    ann: option bool,
    ; optional:
    mai: option Sides<f64>,
    omi: option Sides<f64>,
    omd: array [f64; 4],
    pin: array [f64; 2],
);

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
