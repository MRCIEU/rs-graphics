// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! `legend()`: a port of the layout algorithm in R's
//! `graphics/R/legend.R`, including keyword positions (`"topright"`, ...),
//! multi-column / horizontal legends, titles, and keys made of a filled
//! box, a line and a point (drawn through the line, by default).
//!
//! R computes everything in *user* coordinates; since the user -> device
//! mapping is linear (also for log axes, where user coordinates are
//! `log10`-transformed), this port works directly in device inches (y up),
//! and converts what it returns ([`LegendLayout`]) back to user
//! coordinates. That gives R's results on reversed axes (`xlim = c(hi,
//! lo)`) too, where R's widths and heights turn negative together, with
//! one exception: for [`LegendPos::Corners`] on a reversed axis R sorts the
//! corners but then steps by its negative character size, so it draws the
//! entries outside its own box. That is not copied: here they go inside,
//! as on other axes.
//!
//! Labels and the title may be plotmath expressions (`Label::math`). As
//! R draws them with `text()`, they are rotated by `par(srt)`.
//!
//! The per-key arguments are R's vectors, recycled over the keys
//! ([`LegendOptions`]), including shaded boxes (`density`/`angle`). Not
//! implemented: `trace`. Two R bugs are not copied: R's `colwise()` gives any
//! column whose widths are all 0 (e.g. `x.intersp = 0`, or empty labels
//! with `text.width = NA`) the widest column's value, meant for empty
//! columns only, which are the only ones that get it here; and with
//! [`LegendPos::Corners`] and more than two columns R places the third
//! and later columns at `NA`. One small difference: an `lty` that is only
//! `"blank"` gets no line column here, while R (seeing a string) leaves
//! an empty one.
//!
//! `par(cex)` scales everything as in `legend.R`: the character cell
//! (`xc`/`yc`) and the measured and drawn text (R's `strwidth(cex = )` and
//! `text(cex = )` both multiply by `par("cex")`, i.e. `cexbase`).

use crate::backend::Backend;
use crate::color::ColorSpec;
use crate::font::FontFace;
use crate::geometry::{Point, Rect};
use crate::lty::LineType;
use crate::numbers::Pair;

use crate::plotmath::Label;
use crate::setters::{char_pch, pick_na};

use super::hatch::ShapeStyle;
use super::{Device, Xpd};

/// Where to put a legend: R's `legend(x, y)` forms.
///
/// [`legend()`](crate::graphics::Graphics::legend) takes anything that
/// converts into one, so positions can be written as in R:
///
/// | R | Rust shorthand | Variant |
/// |---|---|---|
/// | `legend("topright", ...)` | `"topright"` (or `LegendKeyword::TopRight`) | [`Keyword`](LegendPos::Keyword) |
/// | `legend(5, 20, ...)` | `(5.0, 20.0)` | [`Xy`](LegendPos::Xy) |
/// | `legend(c(5, 15), c(10, 20), ...)` | `((5.0, 15.0), (10.0, 20.0))` | [`Corners`](LegendPos::Corners) |
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LegendPos {
    /// A single point `(x, y)` in user coordinates; anchored by
    /// `xjust`/`yjust` (default: the top-left corner of the legend box).
    Xy(f64, f64),
    /// Two opposite corners of the legend box, `(xa, ya, xb, yb)` in user
    /// coordinates (R's `nx == 2` case): the box is stretched to fit
    /// exactly.
    Corners(f64, f64, f64, f64),
    /// One of R's nine keyword positions, relative to the plot region.
    Keyword(LegendKeyword),
}

impl From<LegendKeyword> for LegendPos {
    fn from(k: LegendKeyword) -> Self {
        LegendPos::Keyword(k)
    }
}

impl From<&str> for LegendPos {
    /// One of R's keywords, matched like R's `match.arg()`
    /// ([`LegendKeyword::parse`]: the full name, or a unique prefix such
    /// as `"topr"`).
    ///
    /// # Panics
    /// If `s` is not a keyword or is an ambiguous prefix (R errors too);
    /// use [`LegendKeyword::parse`] or `str::parse` to handle that
    /// yourself.
    fn from(s: &str) -> Self {
        match LegendKeyword::parse(s) {
            Some(k) => LegendPos::Keyword(k),
            None => panic!(
                "legend: {s:?} is not a legend position; use one of \
                 \"bottomright\", \"bottom\", \"bottomleft\", \"left\", \
                 \"topleft\", \"top\", \"topright\", \"right\" or \"center\""
            ),
        }
    }
}

impl From<String> for LegendPos {
    /// As `From<&str>`.
    fn from(s: String) -> Self {
        LegendPos::from(s.as_str())
    }
}

impl From<&String> for LegendPos {
    /// As `From<&str>`.
    fn from(s: &String) -> Self {
        LegendPos::from(s.as_str())
    }
}

impl<P: Pair> From<P> for LegendPos {
    /// `(x, y)`: R's `legend(x, y)`, a point in user coordinates (any
    /// [`Pair`], e.g. `(5, 20)`).
    fn from(p: P) -> Self {
        let (x, y) = p.into_pair();
        LegendPos::Xy(x, y)
    }
}

/// `From<((xa, xb), (ya, yb))>` for each mix of `f64` and `i32`.
macro_rules! corners {
    ($(($a:ty, $b:ty, $c:ty, $d:ty)),*) => {
        $(
            impl From<(($a, $b), ($c, $d))> for LegendPos {
                /// `((xa, xb), (ya, yb))`: R's `legend(x = c(xa, xb),
                /// y = c(ya, yb))`, two opposite corners of the box.
                fn from((x, y): (($a, $b), ($c, $d))) -> Self {
                    let ((xa, xb), (ya, yb)) = (x.into_pair(), y.into_pair());
                    LegendPos::Corners(xa, ya, xb, yb)
                }
            }
        )*
    };
}
corners!(
    (f64, f64, f64, f64),
    (f64, f64, f64, i32),
    (f64, f64, i32, f64),
    (f64, f64, i32, i32),
    (f64, i32, f64, f64),
    (f64, i32, f64, i32),
    (f64, i32, i32, f64),
    (f64, i32, i32, i32),
    (i32, f64, f64, f64),
    (i32, f64, f64, i32),
    (i32, f64, i32, f64),
    (i32, f64, i32, i32),
    (i32, i32, f64, f64),
    (i32, i32, f64, i32),
    (i32, i32, i32, f64),
    (i32, i32, i32, i32)
);

/// R's `"bottomright"`, `"bottom"`, ... keyword positions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LegendKeyword {
    BottomRight,
    Bottom,
    BottomLeft,
    Left,
    TopLeft,
    Top,
    TopRight,
    Right,
    Center,
}

impl LegendKeyword {
    const ALL: [(&'static str, LegendKeyword); 9] = [
        ("bottomright", LegendKeyword::BottomRight),
        ("bottom", LegendKeyword::Bottom),
        ("bottomleft", LegendKeyword::BottomLeft),
        ("left", LegendKeyword::Left),
        ("topleft", LegendKeyword::TopLeft),
        ("top", LegendKeyword::Top),
        ("topright", LegendKeyword::TopRight),
        ("right", LegendKeyword::Right),
        ("center", LegendKeyword::Center),
    ];

    /// Parse like R's `match.arg()`: an exact match, or else a unique
    /// prefix (so `"topr"` is `TopRight`, but `"to"` is ambiguous).
    pub fn parse(s: &str) -> Option<LegendKeyword> {
        if let Some(&(_, k)) = Self::ALL.iter().find(|(name, _)| *name == s) {
            return Some(k);
        }
        let mut it = Self::ALL
            .iter()
            .filter(|(name, _)| !s.is_empty() && name.starts_with(s));
        match (it.next(), it.next()) {
            (Some(&(_, k)), None) => Some(k),
            _ => None,
        }
    }
}

crate::error::from_str_via_parse!(LegendKeyword, "legend position");

/// R's `legend(bty = )`: whether the legend has a box around it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LegendBty {
    /// `"o"` (the default): draw the box.
    #[default]
    Box,
    /// `"n"`: no box.
    None,
}

impl LegendBty {
    /// `"o"` or `"n"`.
    pub fn parse(s: &str) -> Option<LegendBty> {
        match s {
            "o" => Some(LegendBty::Box),
            "n" => Some(LegendBty::None),
            _ => None,
        }
    }
}

crate::error::from_str_via_parse!(LegendBty, "legend box type");
crate::error::from_r_string!(LegendBty, "legend box type", "\"o\" or \"n\"");

/// `legend()`'s `pch`: one or several numbers (`19`, `[1, 2]`, `&pch`),
/// characters (`'a'`, `['a', 'b']`) or strings (`["a", "b"]`), recycled
/// over the keys; `None` elements are R's `NA`, a key without a point
/// (`[Some(19), None]`, `[Some("x"), None]`).
///
/// As in R: a string's first character is its symbol (`""` draws none),
/// except that when the first string has several characters, it alone is
/// used, one character per key (`"abc"` gives keys `a`, `b`, `c`, as R's
/// `legend(pch = "abc")`). Characters other than ASCII are stored as minus
/// their Unicode code point (R's negative `pch`), so `'é'` is `-233`.
pub trait LegendPch {
    /// The `pch` codes, in order; `None` for no point.
    fn into_pch(self) -> Vec<Option<i32>>;
}

/// R's `pch` codes for character `pch` values (`None` = `NA`).
fn string_pch<S: AsRef<str>>(strings: &[Option<S>]) -> Vec<Option<i32>> {
    match strings.first() {
        Some(Some(first)) if first.as_ref().chars().count() > 1 => {
            first.as_ref().chars().map(|c| Some(char_pch(c))).collect()
        }
        _ => strings
            .iter()
            .map(|s| {
                s.as_ref()
                    .and_then(|s| s.as_ref().chars().next())
                    .map(char_pch)
            })
            .collect(),
    }
}

macro_rules! legend_pch_impls {
    (numbers: $($n:ty),*; strings: $($s:ty),*) => {
        $(
            legend_pch_impls!(@each $n, |v: $n| Some(i32::from(v)), |v: Option<$n>| v.map(i32::from));
        )*
        $(
            impl LegendPch for $s {
                fn into_pch(self) -> Vec<Option<i32>> {
                    string_pch(&[Some(self)])
                }
            }
            impl LegendPch for Option<$s> {
                fn into_pch(self) -> Vec<Option<i32>> {
                    string_pch(&[self])
                }
            }
            impl<const N: usize> LegendPch for [$s; N] {
                fn into_pch(self) -> Vec<Option<i32>> {
                    string_pch(&self.map(Some))
                }
            }
            impl<const N: usize> LegendPch for [Option<$s>; N] {
                fn into_pch(self) -> Vec<Option<i32>> {
                    string_pch(&self)
                }
            }
            impl LegendPch for &[$s] {
                fn into_pch(self) -> Vec<Option<i32>> {
                    string_pch(&self.iter().map(Some).collect::<Vec<_>>())
                }
            }
            impl<const N: usize> LegendPch for &[$s; N] {
                fn into_pch(self) -> Vec<Option<i32>> {
                    string_pch(&self.iter().map(Some).collect::<Vec<_>>())
                }
            }
            impl LegendPch for Vec<$s> {
                fn into_pch(self) -> Vec<Option<i32>> {
                    string_pch(&self.into_iter().map(Some).collect::<Vec<_>>())
                }
            }
            impl LegendPch for Vec<Option<$s>> {
                fn into_pch(self) -> Vec<Option<i32>> {
                    string_pch(&self)
                }
            }
        )*
    };
    (@each $e:ty, $one:expr, $opt:expr) => {
        impl LegendPch for $e {
            fn into_pch(self) -> Vec<Option<i32>> {
                vec![$one(self)]
            }
        }
        impl LegendPch for Option<$e> {
            fn into_pch(self) -> Vec<Option<i32>> {
                vec![$opt(self)]
            }
        }
        impl<const N: usize> LegendPch for [$e; N] {
            fn into_pch(self) -> Vec<Option<i32>> {
                self.into_iter().map($one).collect()
            }
        }
        impl<const N: usize> LegendPch for [Option<$e>; N] {
            fn into_pch(self) -> Vec<Option<i32>> {
                self.into_iter().map($opt).collect()
            }
        }
        impl LegendPch for &[$e] {
            fn into_pch(self) -> Vec<Option<i32>> {
                self.iter().copied().map($one).collect()
            }
        }
        impl<const N: usize> LegendPch for &[$e; N] {
            fn into_pch(self) -> Vec<Option<i32>> {
                self.iter().copied().map($one).collect()
            }
        }
        impl LegendPch for Vec<$e> {
            fn into_pch(self) -> Vec<Option<i32>> {
                self.into_iter().map($one).collect()
            }
        }
        impl LegendPch for &Vec<$e> {
            fn into_pch(self) -> Vec<Option<i32>> {
                self.iter().copied().map($one).collect()
            }
        }
        impl LegendPch for Vec<Option<$e>> {
            fn into_pch(self) -> Vec<Option<i32>> {
                self.into_iter().map($opt).collect()
            }
        }
    };
}
legend_pch_impls!(numbers: i32, i16, u8, u16; strings: &str, String, &String);
legend_pch_impls!(@each char, |c: char| Some(char_pch(c)), |c: Option<char>| c.map(char_pch));

/// The arguments of R's `legend()` other than its position and labels.
/// Empty `Vec`s and `None` are R's defaults, given for each field.
///
/// The per-key arguments (`col`, `pch`, `lty`, `lwd`, `fill`, `border`,
/// `pt_bg`, `pt_cex`, `pt_lwd`, `cex`, `x_intersp`, `seg_len`,
/// `text_col`, `text_font`) are recycled over the keys, as in R. A `None`
/// element is R's `NA` for that key (`.pch([Some(19), None])`): the
/// field says what it does. Which parts a key has follows R's rules:
/// - a box, if `fill` or `density` is given (every key gets one; a `None`
///   fill is an empty box), shaded with lines if `density` is;
/// - a line, if `lty` has a line type other than blank, or `lwd` is given
///   (R: "one of these two must be specified for line drawing"; `lty` is
///   then solid); keys whose `lty` or `lwd` is `None`, or whose `lty` is
///   blank, have none;
/// - a point, if `pch` is given, drawn through the middle of the line
///   unless `merge` is `false`; keys whose `pch` is `None` (or negative and
///   above -32, as in R) have none.
///
#[derive(Clone, Debug, Default)]
pub struct LegendOptions {
    /// Colours of the lines and points; empty: `par("col")`.
    pub col: Vec<ColorSpec>,
    /// Plotting symbols (see [`LegendPch`] for characters); empty: no
    /// points.
    /// `None` (R's `NA`): no point on that key.
    pub pch: Vec<Option<i32>>,
    /// Line types of the lines; empty: solid, if `lwd` asks for lines.
    /// `None` (R's `NA`): no line on that key.
    pub lty: Vec<Option<LineType>>,
    /// Line widths of the lines (giving any draws lines); empty:
    /// `par("lwd")`.
    /// `None` (R's `NA`): no line on that key (its point, if any, uses `par(lwd)`).
    pub lwd: Vec<Option<f64>>,
    /// Fill colours of box keys (giving any draws boxes); empty: no boxes.
    pub fill: Vec<ColorSpec>,
    /// Border colours of box keys; empty: black (R's default).
    pub border: Vec<ColorSpec>,
    /// Shading lines per inch in the box keys (R's `density`; giving any
    /// draws boxes), as for [`rect()`](crate::graphics::Graphics::rect):
    /// the lines take the `fill` colour (empty: `par(fg)`); `None` (R's
    /// `NA`) or negative fills the box instead, 0 leaves it empty. Empty:
    /// no shading.
    pub density: Vec<Option<f64>>,
    /// Angle of the shading lines in degrees (R's `angle`), recycled over
    /// the keys; empty: 45.
    pub angle: Vec<f64>,
    /// `pt.bg`: fill of `pch` 21 to 25; empty: none (R's `NA`).
    pub pt_bg: Vec<ColorSpec>,
    /// `pt.cex`: size of the points; empty: `cex`.
    /// `None` (R's `NA`): no point on that key.
    pub pt_cex: Vec<Option<f64>>,
    /// `pt.lwd`: line width of the points; empty: `lwd`.
    /// `None` (R's `NA`): no point on that key.
    pub pt_lwd: Vec<Option<f64>>,
    /// `bty`: whether to draw the surrounding box; `None`: a box (R's
    /// `"o"`).
    pub bty: Option<LegendBty>,
    /// Box fill (`bg=`); `None` = `par("bg")` (transparent, so unfilled,
    /// on PDF by default: see [`Backend::default_bg`]).
    pub bg: Option<ColorSpec>,
    /// Size of each key's text and symbol, times `par(cex)`; empty: 1.
    /// `None` (R's `NA`): 1.
    pub cex: Vec<Option<f64>>,
    /// Horizontal justification of the box relative to `x`; `None`: R's
    /// default (0, or 0.5 for [`LegendPos::Corners`]).
    pub xjust: Option<f64>,
    /// Vertical justification of the box relative to `y`; `None`: R's
    /// default (1, or 0.5 for [`LegendPos::Corners`]).
    pub yjust: Option<f64>,
    /// Space between each key and its label, in character widths (a
    /// column is as wide as its widest); empty: 1.
    /// `None` (R's `NA`): 1.
    pub x_intersp: Vec<Option<f64>>,
    /// Space between rows, in character heights, recycled over the rows;
    /// empty: 1.
    /// `None` (R's `NA`): 1.
    pub y_intersp: Vec<Option<f64>>,
    /// Length of each key's line, in character widths; empty: 2.
    /// `None` (R's `NA`): 2.
    pub seg_len: Vec<Option<f64>>,
    /// `text.width`, in x *user* units: one width for every column, or
    /// one per column; `f64::NAN` (R's `NA`) makes each column as wide as
    /// its own widest label; empty: every column as wide as the widest
    /// label.
    pub text_width: Vec<f64>,
    /// `adj[1]`: horizontal adjustment of the labels; `None`: 0 (left).
    pub adj: Option<f64>,
    /// `adj[2]`: vertical adjustment of the labels; `None`: 0.5.
    /// `Some(None)` (the setter's `f64::NAN`) is R's `NA`, centring each
    /// label's ink, as R's one-value `adj = x` does.
    pub vadj: Option<Option<f64>>,
    /// `merge`; `None` = R's default (lines and points both drawn).
    pub merge: Option<bool>,
    /// Number of columns; `None`: 1.
    pub ncol: Option<usize>,
    /// Lay the entries out in one row. Default `false`.
    pub horiz: bool,
    /// Text or a plotmath expression above the entries; `None`: no title.
    pub title: Option<Label>,
    /// `title.cex`; `None` = the first `cex`.
    pub title_cex: Option<f64>,
    /// `title.col`; `None` = the first `text_col`.
    pub title_col: Option<ColorSpec>,
    /// `title.adj`: where the title sits across the box (0 left, 1
    /// right) and its justification there; `None`: 0.5 (centred).
    pub title_adj: Option<f64>,
    /// `title.font`; `None` = the first `text_font`.
    pub title_font: Option<FontFace>,
    /// `inset` as fractions of the plot region, `(x, y)`, for keyword
    /// positions; `None`: `(0, 0)`.
    pub inset: Option<(f64, f64)>,
    /// `text.col`: label colours; empty: `par("col")`.
    pub text_col: Vec<ColorSpec>,
    /// `text.font`: label fonts (also for measuring them); empty:
    /// `par("font")`.
    /// `None` (R's `NA`): `par("font")`.
    pub text_font: Vec<Option<FontFace>>,
    /// `box.lwd`: line width of the legend box; `None`: `par("lwd")`.
    pub box_lwd: Option<f64>,
    /// `box.lty`: line type of the legend box; `None`: `par("lty")`.
    pub box_lty: Option<LineType>,
    /// `box.col`: colour of the legend box's border (`ColorSpec::None`:
    /// no border); `None`: `par("fg")`.
    pub box_col: Option<ColorSpec>,
    /// R's inline `xpd` while drawing ([`Xpd`]; the setter also takes
    /// `false`/`true`); `None`: `par(xpd)`.
    pub xpd: Option<Xpd>,
    /// R's `plot`: `false` works out the layout (returned as usual) but
    /// draws nothing; `None`: `true`.
    pub plot: Option<bool>,
}

crate::setters::setters!(LegendOptions => [LegendCall] {
    col: values ColorSpec,
    pch: legend_pch Option<i32>,
    lty: na_values LineType,
    lwd: na_values f64,
    fill: values ColorSpec,
    border: values ColorSpec,
    density: na_values f64,
    angle: values f64,
    pt_bg: values ColorSpec,
    pt_cex: na_values f64,
    pt_lwd: na_values f64,
    bty: option LegendBty,
    bg: option ColorSpec,
    cex: na_values f64,
    xjust: option f64,
    yjust: option f64,
    x_intersp: na_values f64,
    y_intersp: na_values f64,
    seg_len: na_values f64,
    text_width: values f64,
    adj: option f64,
    vadj: na f64,
    merge: option bool,
    ncol: count usize,
    horiz: value bool,
    title: option Label,
    title_cex: option f64,
    title_col: option ColorSpec,
    title_adj: option f64,
    title_font: option FontFace,
    inset: option_pair (f64, f64),
    text_col: values ColorSpec,
    text_font: na_values FontFace,
    box_lwd: option f64,
    box_lty: option LineType,
    box_col: option ColorSpec,
    xpd: option Xpd,
    plot: option bool,
});

/// R's argument checks for `legend()`, so a [`Figure`](crate::recording::Figure)
/// panics when recording, as the device does.
pub(crate) fn check_legend(labels: &[Label], opts: &LegendOptions) {
    assert!(!labels.is_empty(), "'legend' is of length 0");
    let tw = &opts.text_width;
    let any_na = tw.iter().any(|v| v.is_nan());
    let bad = (tw.len() > 1 && any_na) || (!any_na && tw.iter().any(|&v| v < 0.0));
    assert!(!bad, "'text.width' must be numeric, >= 0, or a scalar NA");
}

/// Where `legend()` drew, as R's (invisible) return value `list(rect =
/// list(w, h, left, top), text = list(x, y))`. All in `usr` units, as in
/// R: log10 values on a log axis (see [`Device::usr`]).
#[derive(Clone, Debug, PartialEq)]
pub struct LegendLayout {
    /// `rect$left`, `rect$top`: the box's top-left corner.
    pub left: f64,
    pub top: f64,
    /// `rect$w`, `rect$h`: the box's width and height.
    pub w: f64,
    pub h: f64,
    /// `text$x`, `text$y`: each label's anchor (its left end and vertical
    /// centre).
    pub text_x: Vec<f64>,
    pub text_y: Vec<f64>,
}

impl<B: Backend> Device<B> {
    /// Draws R's `legend()`; the public entry is
    /// [`Graphics::legend`](crate::graphics::Graphics::legend), whose docs
    /// describe it.
    pub(crate) fn legend_impl(
        &mut self,
        pos: LegendPos,
        labels: &[Label],
        opts: &LegendOptions,
    ) -> LegendLayout {
        check_legend(labels, opts);
        // R's `legend(xpd = )` sets `par(xpd)` for the call and restores it
        // `on.exit`.
        let mut dev = self.par_scope(|now, saved| now.xpd = saved.xpd);
        if let Some(xpd) = opts.xpd {
            dev.par.xpd = xpd;
        }
        dev.draw_legend(pos, labels, opts)
    }

    fn draw_legend(
        &mut self,
        pos: LegendPos,
        labels: &[Label],
        opts: &LegendOptions,
    ) -> LegendLayout {
        let n = labels.len();
        let plt = self.plt;
        // The per-key arguments, recycled over the keys (R's `rep_len`),
        // with R's defaults for those left unset.
        fn rep<T: Copy>(v: &[T], n: usize, default: T) -> Vec<T> {
            (0..n)
                .map(|i| {
                    if v.is_empty() {
                        default
                    } else {
                        v[i % v.len()]
                    }
                })
                .collect()
        }
        // The same for `NaValues` vectors (`None` = R's `NA`), and for those
        // where `NA` means the default.
        fn rep_na<T: Copy>(v: &[Option<T>], n: usize, default: T) -> Vec<Option<T>> {
            (0..n).map(|i| pick_na(v, i, default)).collect()
        }
        fn rep_or<T: Copy>(v: &[Option<T>], n: usize, default: T) -> Vec<T> {
            (0..n)
                .map(|i| pick_na(v, i, default).unwrap_or(default))
                .collect()
        }
        let cex = rep_or(&opts.cex, n, 1.0);
        let x_intersp = rep_or(&opts.x_intersp, n, 1.0);
        let seg_len = rep_or(&opts.seg_len, n, 2.0);
        let inset = opts.inset.unwrap_or((0.0, 0.0));

        // Number of columns and of rows ("legends per column").
        let n_per_col = if opts.horiz {
            1
        } else {
            n.div_ceil(opts.ncol.unwrap_or(1).max(1))
        };
        let mut ncol = if opts.horiz {
            n
        } else {
            opts.ncol.unwrap_or(1).max(1)
        };

        // One character cell per key: `xyinch(par("cin"))` scaled by the
        // effective cex. For R's pdf device `cin = (0.9, 1.2) * startps / 72`.
        let cin_x = 0.9 * self.start_ps / 72.0;
        let cin_y = 1.2 * self.start_ps / 72.0;
        let xc: Vec<f64> = cex.iter().map(|c| c * self.par.cex * cin_x).collect();
        let yc: Vec<f64> = cex.iter().map(|c| c * self.par.cex * cin_y).collect();

        // `text.font` recycled over the keys, as R's `mapply()`; the
        // title's font defaults to the first.
        let fonts = rep_or(&opts.text_font, n, self.par.font);
        let title_font = opts.title_font.unwrap_or(fonts[0]);
        let label_w: Vec<f64> = (0..n)
            .map(|i| self.label_width(&labels[i], fonts[i], cex[i]))
            .collect();

        // `text.width`: the widest label for every column (`NULL`), each
        // column's own widest (`NA`, which also drops empty columns), or
        // the given widths, one per column.
        let (ux0, ux1, _, _) = self.usr;
        let inches_per_user_x = plt.width() / (ux1 - ux0).abs();
        let auto_text_width = opts.text_width.len() == 1 && opts.text_width[0].is_nan();
        if auto_text_width {
            ncol = n.div_ceil(n_per_col);
        }
        // Column `c` holds keys `c * n_per_col ..`; R's `colwise()` gives an
        // empty column the widest column's value ("space for empty
        // columns").
        let col_keys = |c: usize| (c * n_per_col).min(n)..((c + 1) * n_per_col).min(n);
        let colwise = |v: &[f64]| -> Vec<f64> {
            let maxes: Vec<Option<f64>> = (0..ncol)
                .map(|c| col_keys(c).map(|i| v[i].abs()).reduce(f64::max))
                .collect();
            let widest = maxes.iter().flatten().copied().fold(0.0, f64::max);
            maxes.into_iter().map(|m| m.unwrap_or(widest)).collect()
        };
        let col_text_w: Vec<f64> = if auto_text_width {
            colwise(&label_w)
        } else if opts.text_width.is_empty() {
            vec![label_w.iter().copied().fold(0.0, f64::max); ncol]
        } else {
            let tw = &opts.text_width;
            (0..ncol)
                .map(|c| tw[c % tw.len()].abs() * inches_per_user_x)
                .collect()
        };

        // Row heights: at least a character cell, or the tallest label
        // (an expression can be taller). Row `r` holds keys `r`,
        // `r + n_per_col`, ...
        let row_keys = |r: usize| (r..n).step_by(n_per_col);
        let y_intersp = rep_or(&opts.y_intersp, n_per_col, 1.0);
        let row_yc: Vec<f64> = (0..n_per_col)
            .map(|r| row_keys(r).map(|i| yc[i]).fold(0.0, f64::max))
            .collect();
        let yextra: Vec<f64> = (0..n_per_col)
            .map(|r| row_yc[r] * (y_intersp[r] - 1.0))
            .collect();
        let tallest = (0..n)
            .map(|i| self.label_height(&labels[i], fonts[i], cex[i]) / yc[i])
            .fold(1.0, f64::max);
        let ymax = yc.iter().copied().fold(0.0, f64::max) * tallest;
        let ychar: Vec<f64> = yextra.iter().map(|e| e + ymax).collect();

        // Title.
        let title = opts
            .title
            .as_ref()
            .filter(|t| !matches!(t, Label::Text(s) if s.is_empty()));
        let tcex = opts.title_cex.unwrap_or(cex[0]);
        let title_unit_h = tcex * self.par.cex * cin_y;
        let ymaxtitle = match title {
            Some(t) => title_unit_h.max(self.label_height(t, title_font, tcex)),
            None => title_unit_h,
        };
        let ychartitle = yextra[0] + ymaxtitle;
        let title_extra_h = if title.is_some() { ychartitle } else { 0.0 };

        // R's `mfill <- !missing(fill) || !missing(density)`.
        let has_fill = !opts.fill.is_empty() || !opts.density.is_empty();
        let xbox: Vec<f64> = xc.iter().map(|x| x * 0.8).collect();
        let ybox: Vec<f64> = yc.iter().map(|y| y * 0.5).collect();
        let dx_fill = xbox.iter().copied().fold(0.0, f64::max);
        let do_lines = opts
            .lty
            .iter()
            .any(|&l| l.is_some_and(|l| l != LineType::Blank))
            || !opts.lwd.is_empty();
        let has_pch = !opts.pch.is_empty();
        let merge = opts.merge.unwrap_or(do_lines && has_pch);
        let x_off = if do_lines && merge { -0.7 } else { 0.0 };

        // `w0`: the width of each column.
        let w0: Vec<f64>;
        let (left, top, w, h, xextra);
        match pos {
            LegendPos::Corners(xa, ya, xb, yb) => {
                let pa = self.user_to_inches(xa, ya);
                let pb = self.user_to_inches(xb, yb);
                left = pa.x.min(pb.x);
                top = pa.y.max(pb.y);
                w = (pa.x - pb.x).abs();
                h = (pa.y - pb.y).abs();
                w0 = vec![w / ncol as f64; ncol];
                xextra = 0.0;
            }
            _ => {
                let h_ = ychar.iter().sum::<f64>() + row_yc[n_per_col - 1] + title_extra_h;

                let xch1 = colwise(&xc);
                let x_intersp_col = colwise(&x_intersp);
                let seg_len_col = colwise(&seg_len);
                w0 = (0..ncol)
                    .map(|c| {
                        let mut w = col_text_w[c] + (x_intersp_col[c] + 1.0) * xch1[c];
                        if has_fill {
                            w += dx_fill;
                        }
                        if do_lines {
                            w += (seg_len_col[c] + x_off) * xch1[c];
                        }
                        w
                    })
                    .collect();
                let mut w_ = w0.iter().sum::<f64>() + 0.5 * xch1[ncol - 1];
                let mut xextra_ = 0.0;
                if let Some(t) = title {
                    let tw =
                        self.label_width(t, title_font, tcex) + 0.5 * tcex * self.par.cex * cin_x;
                    if tw > w_ {
                        xextra_ = (tw - w_) / 2.0;
                        w_ = tw;
                    }
                }

                let (l, t) = match pos {
                    LegendPos::Xy(x, y) => {
                        let p = self.user_to_inches(x, y);
                        let xjust = opts.xjust.unwrap_or(0.0);
                        let yjust = opts.yjust.unwrap_or(1.0);
                        (p.x - xjust * w_, p.y + (1.0 - yjust) * h_)
                    }
                    LegendPos::Keyword(k) => {
                        use LegendKeyword::*;
                        let insetx = inset.0 * plt.width();
                        let insety = inset.1 * plt.height();
                        let l = match k {
                            BottomRight | TopRight | Right => plt.x1 - w_ - insetx,
                            BottomLeft | Left | TopLeft => plt.x0 + insetx,
                            Bottom | Top | Center => (plt.x0 + plt.x1 - w_) / 2.0,
                        };
                        let t = match k {
                            BottomRight | Bottom | BottomLeft => plt.y0 + h_ + insety,
                            TopLeft | Top | TopRight => plt.y1 - insety,
                            Left | Right | Center => (plt.y0 + plt.y1 + h_) / 2.0,
                        };
                        (l, t)
                    }
                    LegendPos::Corners(..) => unreachable!(),
                };
                left = l;
                top = t;
                w = w_;
                h = h_;
                xextra = xextra_;
            }
        }

        // The legend itself. Graphics are clipped according to `xpd`, like
        // the `rect()`/`segments()`/`points()` calls R makes. With `plot =
        // FALSE` only the positions are worked out.
        let plot = opts.plot.unwrap_or(true);
        if plot {
            self.push_clip_for(self.par.xpd);
        }
        if plot && opts.bty.unwrap_or_default() == LegendBty::Box {
            let bg = opts.bg.unwrap_or(ColorSpec::Color(self.par.bg));
            let fill = self.resolve(bg).filter(|c| c.a > 0);
            let box_col = opts.box_col.unwrap_or(ColorSpec::Color(self.par.fg));
            let stroke = self.resolve(box_col).map(|c| {
                self.stroke_of(
                    c,
                    opts.box_lwd.unwrap_or(self.par.lwd),
                    opts.box_lty.unwrap_or(self.par.lty),
                )
            });
            self.backend.draw_rect(
                Rect::new(left, top - h, left + w, top),
                fill,
                stroke.as_ref(),
            );
        }

        // Each key's left end and vertical centre.
        let mut col_x = vec![0.0; ncol];
        for c in 1..ncol {
            col_x[c] = col_x[c - 1] + w0[c - 1];
        }
        let mut xt: Vec<f64> = (0..n)
            .map(|i| left + xc[i] + xextra + col_x[i / n_per_col])
            .collect();
        let topspace = 0.5 * ymax + title_extra_h;
        let mut row_y = vec![ychar[0] / 2.0; n_per_col];
        for r in 1..n_per_col {
            row_y[r] = row_y[r - 1] + (ychar[r - 1] + ychar[r]) / 2.0;
        }
        let yt: Vec<f64> = (0..n)
            .map(|i| top - topspace - row_y[i % n_per_col])
            .collect();

        let black = ColorSpec::Color(crate::color::Color::BLACK);
        if has_fill {
            if plot {
                // `rect2(left = xt, top = yt + ybox/2, dx = xbox, dy = ybox,
                // col = fill, density, angle, border)`: one `rect()` call,
                // `fill` recycled over the keys. The corners go to `rect()`
                // in data units, since shading lines are placed in them.
                let corners: Vec<((f64, f64), (f64, f64))> = (0..n)
                    .map(|i| {
                        let top = yt[i] + ybox[i] / 2.0;
                        (
                            self.inches_to_user(Point::new(xt[i], top)),
                            self.inches_to_user(Point::new(xt[i] + xbox[i], top - ybox[i])),
                        )
                    })
                    .collect();
                let x0: Vec<f64> = corners.iter().map(|c| c.0.0).collect();
                let y0: Vec<f64> = corners.iter().map(|c| c.0.1).collect();
                let x1: Vec<f64> = corners.iter().map(|c| c.1.0).collect();
                let y1: Vec<f64> = corners.iter().map(|c| c.1.1).collect();
                let fill = if opts.fill.is_empty() {
                    Vec::new()
                } else {
                    rep(&opts.fill, n, ColorSpec::None)
                };
                let border = rep(&opts.border, n, black);
                let style = ShapeStyle {
                    col: &fill,
                    border: &border,
                    lty: &[],
                    lwd: &[],
                    density: &opts.density,
                    angle: &opts.angle,
                };
                self.rects(&x0, &y0, &x1, &y1, &style);
            }
            for x in xt.iter_mut() {
                *x += dx_fill;
            }
        }

        let col = rep(&opts.col, n, ColorSpec::Color(self.par.col));
        let lwd = rep_na(&opts.lwd, n, self.par.lwd);
        if do_lines {
            // A key with an `NA` (`None`) `lty` or `lwd`, or a blank `lty`,
            // has no line.
            let lty = rep_na(&opts.lty, n, LineType::SOLID);
            for i in 0..n {
                let line = lwd[i].zip(lty[i]).filter(|&(_, l)| l != LineType::Blank);
                if let (true, Some((lwd, lty)), Some(c)) = (plot, line, self.resolve(col[i])) {
                    let stroke = self.stroke_of(c, lwd, lty);
                    let x0 = xt[i] + x_off * xc[i];
                    self.backend.draw_line(
                        Point::new(x0, yt[i]),
                        Point::new(x0 + seg_len[i] * xc[i], yt[i]),
                        &stroke,
                    );
                }
                xt[i] += (seg_len[i] + x_off) * xc[i];
            }
        }

        if plot && has_pch {
            // `pt.cex` defaults to each key's `cex`, `pt.lwd` to its `lwd`
            // (`par(lwd)` where that is `NA`: the point is drawn anyway).
            let pch = rep_na(&opts.pch, n, -1);
            let pt_cex: Vec<Option<f64>> = if opts.pt_cex.is_empty() {
                cex.iter().copied().map(Some).collect()
            } else {
                rep_na(&opts.pt_cex, n, 1.0)
            };
            let pt_bg = rep(&opts.pt_bg, n, ColorSpec::None);
            let pt_lwd: Vec<Option<f64>> = if opts.pt_lwd.is_empty() {
                lwd.iter()
                    .map(|l| Some(l.unwrap_or(self.par.lwd)))
                    .collect()
            } else {
                rep_na(&opts.pt_lwd, n, 1.0)
            };
            for i in 0..n {
                // An `NA` (`None`) `pch`, `pt.cex` or `pt.lwd` skips the
                // point, as a negative `pch` above -32 does in R.
                let (Some(pch), Some(pt_cex), Some(pt_lwd)) = (pch[i], pt_cex[i], pt_lwd[i]) else {
                    continue;
                };
                if !(pch >= 0 || pch <= -32) {
                    continue;
                }
                let Some(c) = self.resolve(col[i]) else {
                    continue;
                };
                let x1 = if merge && do_lines {
                    xt[i] - (seg_len[i] / 2.0) * xc[i]
                } else {
                    xt[i]
                };
                let bg = self.resolve(pt_bg[i]);
                self.draw_pch(pch, Point::new(x1, yt[i]), pt_cex, c, bg, pt_lwd);
            }
        }
        if plot {
            self.pop_clip();
        }

        // `text.col` recycled over the keys; the title's colour defaults
        // to the first.
        let text_col = rep(&opts.text_col, n, ColorSpec::Color(self.par.col));
        // `text(adj = c(title.adj, 0))` and `text(adj = adj)`.
        let clip = self.par.xpd;
        if let (true, Some(t)) = (plot, title) {
            let title_adj = opts.title_adj.unwrap_or(0.5);
            let anchor = Point::new(left + w * title_adj, top - ymaxtitle);
            self.draw_label(
                clip,
                anchor,
                t,
                title_adj,
                Some(0.0),
                self.par.srt,
                opts.title_col.unwrap_or(text_col[0]),
                tcex,
                title_font,
            );
        }
        let adj = opts.adj.unwrap_or(0.0);
        let vadj = opts.vadj.unwrap_or(Some(0.5));
        let mut text_x = Vec::with_capacity(n);
        let mut text_y = Vec::with_capacity(n);
        for i in 0..n {
            let anchor = Point::new(xt[i] + x_intersp[i] * xc[i], yt[i]);
            let (tx, ty) = self.inches_to_usr(anchor);
            text_x.push(tx);
            text_y.push(ty);
            if !plot {
                continue;
            }
            self.draw_label(
                clip,
                anchor,
                &labels[i],
                adj,
                vadj,
                self.par.srt,
                text_col[i],
                cex[i],
                fonts[i],
            );
        }

        let (left_u, top_u) = self.inches_to_usr(Point::new(left, top));
        let (right_u, bottom_u) = self.inches_to_usr(Point::new(left + w, top - h));
        LegendLayout {
            left: left_u,
            top: top_u,
            w: right_u - left_u,
            h: top_u - bottom_u,
            text_x,
            text_y,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positions_convert_like_r() {
        let p = |p: LegendPos| p;
        assert_eq!(
            p("topright".into()),
            LegendPos::Keyword(LegendKeyword::TopRight)
        );
        assert_eq!(
            p("topr".into()),
            LegendPos::Keyword(LegendKeyword::TopRight)
        );
        assert_eq!(
            p(String::from("center").into()),
            LegendPos::Keyword(LegendKeyword::Center)
        );
        assert_eq!(
            p(LegendKeyword::Left.into()),
            LegendPos::Keyword(LegendKeyword::Left)
        );
        assert_eq!(p((5.0, 20.0).into()), LegendPos::Xy(5.0, 20.0));
        assert_eq!(
            p(((5.0, 15.0), (10.0, 20.0)).into()),
            LegendPos::Corners(5.0, 10.0, 15.0, 20.0)
        );
    }

    #[test]
    #[should_panic(expected = "\"middle\" is not a legend position")]
    fn unknown_keyword_panics() {
        let _ = LegendPos::from("middle");
    }

    #[test]
    #[should_panic(expected = "\"to\" is not a legend position")]
    fn ambiguous_prefix_panics() {
        let _ = LegendPos::from("to");
    }

    #[test]
    fn pch_takes_numbers_characters_and_strings_like_r() {
        assert_eq!(19.into_pch(), vec![Some(19)]);
        assert_eq!([1, 2].into_pch(), vec![Some(1), Some(2)]);
        assert_eq!(vec![21u8].into_pch(), vec![Some(21)]);
        assert_eq!('a'.into_pch(), vec![Some(97)]);
        assert_eq!(['a', '\u{e9}'].into_pch(), vec![Some(97), Some(-233)]);
        // One character each; "" draws nothing (R's `NA`).
        assert_eq!(["x", "+", ""].into_pch(), vec![Some(120), Some(43), None]);
        // A first string of several characters is split, the rest unused.
        assert_eq!("abc".into_pch(), vec![Some(97), Some(98), Some(99)]);
        assert_eq!(["ab", "z"].into_pch(), vec![Some(97), Some(98)]);
        assert_eq!(String::from("\u{e9}").into_pch(), vec![Some(-233)]);
        // `None` is R's `NA`, for numbers, characters and strings.
        assert_eq!([Some(19), None].into_pch(), vec![Some(19), None]);
        assert_eq!([None, Some('a')].into_pch(), vec![None, Some(97)]);
        assert_eq!([Some("x"), None].into_pch(), vec![Some(120), None]);
    }

    #[test]
    #[should_panic(expected = "'legend' is of length 0")]
    fn no_labels_panics() {
        check_legend(&[], &LegendOptions::default());
    }

    #[test]
    fn text_width_is_checked_like_r() {
        let labels = [Label::from("a")];
        let check = |tw: Vec<f64>| {
            let opts = LegendOptions {
                text_width: tw,
                ..LegendOptions::default()
            };
            std::panic::catch_unwind(|| check_legend(&labels, &opts)).is_ok()
        };
        assert!(check(vec![]));
        assert!(check(vec![f64::NAN]));
        assert!(check(vec![0.0, 1.5]));
        assert!(!check(vec![-1.0]));
        assert!(!check(vec![1.0, f64::NAN]));
    }

    #[test]
    fn keyword_parsing_is_like_match_arg() {
        assert_eq!(
            LegendKeyword::parse("topright"),
            Some(LegendKeyword::TopRight)
        );
        assert_eq!(LegendKeyword::parse("top"), Some(LegendKeyword::Top));
        assert_eq!(LegendKeyword::parse("topr"), Some(LegendKeyword::TopRight));
        assert_eq!(LegendKeyword::parse("cen"), Some(LegendKeyword::Center));
        assert_eq!(LegendKeyword::parse("to"), None);
        assert_eq!(LegendKeyword::parse("nowhere"), None);
        assert_eq!(LegendKeyword::parse(""), None);
    }
}
