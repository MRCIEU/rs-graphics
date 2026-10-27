// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Colours: RGBA values ([`Color`]), colour arguments ([`ColorSpec`]),
//! the default palette and grey ramps.
//!
//! A colour argument is a string (`"red"`, `"#FF0000"`, `"#FF000080"`), a
//! small integer indexing the current palette ([`ColorSpec::Index`]; the
//! default palette is [`DEFAULT_PALETTE`]), or nothing (`None`, R's
//! `NA`: don't draw). [`Color::parse`] accepts the strings R's
//! `col2rgb()` and graphics functions accept, except numeric strings such
//! as `"2"` (R's palette indices; use [`ColorSpec::Index`]).

use crate::named_colors::NAMED_COLORS;

/// An RGBA colour with 8 bits per channel.
///
/// This is the same representation R uses internally (a packed
/// `AABBGGRR` `u32`). Alpha 255 is opaque, 0 fully transparent. A
/// `Color` converts into a [`ColorSpec`], so it can be passed wherever a
/// colour argument is taken; so can a string, via [`Color::parse`]
/// (`From<&str>` for `Color` panics on an invalid string, as R errors).
///
/// # Examples
///
/// ```
/// # use rs_graphics::prelude::*;
/// let c = Color::parse("steelblue").unwrap();
/// assert_eq!(c, Color::rgb(0x46, 0x82, 0xB4));
/// assert_eq!(c.with_alpha(128).to_hex(), "#4682b480");
/// ```
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Color {
    /// Red, 0-255.
    pub r: u8,
    /// Green, 0-255.
    pub g: u8,
    /// Blue, 0-255.
    pub b: u8,
    /// Alpha (opacity), 0 (transparent) to 255 (opaque).
    pub a: u8,
}

impl Color {
    /// An opaque colour from its red, green and blue channels.
    ///
    /// This is R's `rgb(r, g, b, maxColorValue = 255)`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// assert_eq!(Color::rgb(255, 0, 0).to_hex(), "#ff0000");
    /// ```
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Color { r, g, b, a: 255 }
    }

    /// A colour from its red, green, blue and alpha channels.
    ///
    /// This is R's `rgb(r, g, b, alpha, maxColorValue = 255)`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// assert_eq!(Color::rgba(255, 0, 0, 128).to_hex(), "#ff000080");
    /// ```
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Color { r, g, b, a }
    }

    /// Fully transparent black (alpha 0); nothing is drawn with it.
    pub const TRANSPARENT: Color = Color::rgba(0, 0, 0, 0);
    /// Opaque black.
    pub const BLACK: Color = Color::rgb(0, 0, 0);
    /// Opaque white.
    pub const WHITE: Color = Color::rgb(255, 255, 255);

    /// Parses a colour string, or returns `None` for `"NA"`, an empty
    /// string or an unknown name.
    ///
    /// Accepted: the hex forms `"#rgb"`, `"#rgba"`, `"#rrggbb"` and
    /// `"#rrggbbaa"`; `"transparent"`, which is white with alpha 0
    /// (`#FFFFFF00`); and R's 657 built-in colour names, matched ignoring
    /// case and spaces (`"Light Blue"` is `"lightblue"`). Surrounding
    /// whitespace is ignored. Numeric strings such as `"2"` are not palette
    /// indices here; use [`ColorSpec::Index`].
    ///
    /// This is how R's `col2rgb()` and graphics functions read colour
    /// strings.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// assert_eq!(Color::parse("#f00"), Some(Color::rgb(255, 0, 0)));
    /// assert_eq!(Color::parse("Dark Red"), Some(Color::rgb(139, 0, 0)));
    /// assert_eq!(Color::parse("transparent"), Some(Color::rgba(255, 255, 255, 0)));
    /// assert_eq!(Color::parse("NA"), None);
    /// ```
    pub fn parse(spec: &str) -> Option<Color> {
        let s = spec.trim();
        if s.is_empty() || s.eq_ignore_ascii_case("NA") {
            return None;
        }
        if s.eq_ignore_ascii_case("transparent") {
            return Some(Color::rgba(255, 255, 255, 0));
        }
        if let Some(hex) = s.strip_prefix('#') {
            return Self::parse_hex(hex);
        }
        // R's `StrMatch()` compares names ignoring case and spaces.
        let lower: String = s
            .chars()
            .filter(|&c| c != ' ')
            .map(|c| c.to_ascii_lowercase())
            .collect();
        NAMED_COLORS
            .iter()
            .find(|(name, ..)| *name == lower)
            .map(|&(_, r, g, b)| Color::rgb(r, g, b))
    }

    fn parse_hex(hex: &str) -> Option<Color> {
        let expand = |c: char| -> Option<u8> {
            let v = c.to_digit(16)?;
            Some((v * 16 + v) as u8)
        };
        let pair = |s: &str| u8::from_str_radix(s, 16).ok();
        match hex.len() {
            3 => {
                let mut chars = hex.chars();
                let r = expand(chars.next()?)?;
                let g = expand(chars.next()?)?;
                let b = expand(chars.next()?)?;
                Some(Color::rgb(r, g, b))
            }
            4 => {
                let mut chars = hex.chars();
                let r = expand(chars.next()?)?;
                let g = expand(chars.next()?)?;
                let b = expand(chars.next()?)?;
                let a = expand(chars.next()?)?;
                Some(Color::rgba(r, g, b, a))
            }
            6 => Some(Color::rgb(
                pair(&hex[0..2])?,
                pair(&hex[2..4])?,
                pair(&hex[4..6])?,
            )),
            8 => Some(Color::rgba(
                pair(&hex[0..2])?,
                pair(&hex[2..4])?,
                pair(&hex[4..6])?,
                pair(&hex[6..8])?,
            )),
            _ => None,
        }
    }

    /// The colour as a lower-case hex string: `#rrggbb`, or `#rrggbbaa`
    /// when not fully opaque.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// assert_eq!(Color::rgb(70, 130, 180).to_hex(), "#4682b4");
    /// assert_eq!(Color::rgba(0, 0, 0, 0x80).to_hex(), "#00000080");
    /// ```
    pub fn to_hex(self) -> String {
        if self.a == 255 {
            format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
        } else {
            format!("#{:02x}{:02x}{:02x}{:02x}", self.r, self.g, self.b, self.a)
        }
    }

    /// The same colour with its alpha (opacity) replaced by `a`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// assert_eq!(Color::BLACK.with_alpha(0x80), Color::rgba(0, 0, 0, 0x80));
    /// ```
    pub fn with_alpha(self, a: u8) -> Color {
        Color { a, ..self }
    }
}

/// The default palette: the eight colours that `col = 1` to `col = 8`
/// stand for.
///
/// Palette indices are 1-based: `col = 1` is the first entry (black),
/// `col = 2` the second, and so on (see [`ColorSpec::Index`]).
///
/// This is R's default `palette()` since R 4.0.0 (the "R4" palette).
///
/// # Examples
///
/// ```
/// use rs_graphics::color::DEFAULT_PALETTE;
/// # use rs_graphics::prelude::*;
/// assert_eq!(ColorSpec::Index(2).resolve(&DEFAULT_PALETTE), Some(DEFAULT_PALETTE[1]));
/// ```
pub const DEFAULT_PALETTE: [Color; 8] = [
    Color::rgb(0x00, 0x00, 0x00), // black
    Color::rgb(0xDF, 0x53, 0x6B), // red-ish
    Color::rgb(0x61, 0xD0, 0x4F), // green
    Color::rgb(0x22, 0x97, 0xE6), // blue
    Color::rgb(0x28, 0xE2, 0xE5), // cyan
    Color::rgb(0xCD, 0x0B, 0xBC), // magenta
    Color::rgb(0xF5, 0xC7, 0x10), // yellow
    Color::rgb(0x9E, 0x9E, 0x9E), // gray
];

/// A colour argument: an explicit colour, a palette index, or no colour.
///
/// Every `col`/`bg`/`border`-style argument takes one. It converts from a
/// [`Color`], a palette index (`i32`), a colour string (as
/// [`Color::parse`]; a string that doesn't parse, `"NA"` included, gives
/// [`ColorSpec::None`]) and an `Option` of any of these (`None` is
/// [`ColorSpec::None`]), so recycled colour vectors can mix colours and
/// `NA`s: `.col([Some("red"), None])`.
///
/// # Examples
///
/// ```
/// # use rs_graphics::prelude::*;
/// assert_eq!(ColorSpec::from("red"), ColorSpec::Color(Color::rgb(255, 0, 0)));
/// assert_eq!(ColorSpec::from(2), ColorSpec::Index(2));
/// assert_eq!(ColorSpec::from(None::<&str>), ColorSpec::None);
/// ```
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum ColorSpec {
    /// No colour: nothing is drawn. R's `NA`.
    None,
    /// An explicit colour.
    Color(Color),
    /// An index into the current palette, 1-based (R's small-integer
    /// colours). Indices above the palette length recycle, as in R; 0 is
    /// the background colour; a negative index is R's error when drawn
    /// (and [`resolve`](ColorSpec::resolve) panics on it).
    Index(i32),
}

impl ColorSpec {
    /// The colour to draw with, or `None` for nothing.
    ///
    /// Nothing is drawn for [`ColorSpec::None`], index 0, and fully
    /// transparent colours (R's devices skip those), nor for an index into
    /// an empty palette. Index 0 is the background colour in R, which this
    /// doesn't know; use
    /// [`Par::resolve_color`](crate::par::Par::resolve_color) for that.
    ///
    /// # Examples
    ///
    /// ```
    /// use rs_graphics::color::DEFAULT_PALETTE;
    /// # use rs_graphics::prelude::*;
    /// // Index 9 recycles to the first palette entry.
    /// assert_eq!(ColorSpec::Index(9).resolve(&DEFAULT_PALETTE), Some(Color::BLACK));
    /// assert_eq!(ColorSpec::from("#FF000000").resolve(&DEFAULT_PALETTE), None);
    /// ```
    ///
    /// # Panics
    ///
    /// For a negative index (R: "numerical color values must be >= 0").
    pub fn resolve(self, palette: &[Color]) -> Option<Color> {
        let c = match self {
            ColorSpec::None => None,
            ColorSpec::Color(c) => Some(c),
            ColorSpec::Index(0) => None,
            ColorSpec::Index(i) if i < 0 => {
                panic!("numerical color values must be >= 0, found {i}")
            }
            ColorSpec::Index(i) => {
                if palette.is_empty() {
                    return None;
                }
                let n = palette.len() as i32;
                let idx = ((i - 1).rem_euclid(n)) as usize;
                Some(palette[idx])
            }
        };
        c.filter(|c| c.a > 0)
    }
}

impl From<Color> for ColorSpec {
    fn from(c: Color) -> Self {
        ColorSpec::Color(c)
    }
}

impl From<i32> for ColorSpec {
    fn from(i: i32) -> Self {
        ColorSpec::Index(i)
    }
}

impl From<&str> for ColorSpec {
    /// A colour name or hex string, as [`Color::parse`]; a string that
    /// doesn't parse (`"NA"` included) gives `ColorSpec::None` (draw
    /// nothing).
    fn from(s: &str) -> Self {
        match Color::parse(s) {
            Some(c) => ColorSpec::Color(c),
            None => ColorSpec::None,
        }
    }
}

impl From<String> for ColorSpec {
    /// As `From<&str>`: a colour name or hex string; unparseable input
    /// gives `ColorSpec::None` (draw nothing).
    fn from(s: String) -> Self {
        ColorSpec::from(s.as_str())
    }
}

impl From<&String> for ColorSpec {
    /// As `From<&str>`.
    fn from(s: &String) -> Self {
        ColorSpec::from(s.as_str())
    }
}

/// `None` is R's `NA` (draw nothing), so recycled colour vectors can mix
/// colours and `NA`s: `.col([Some("red"), None])`.
macro_rules! from_option {
    ($($t:ty),*) => {
        $(
            impl From<Option<$t>> for ColorSpec {
                fn from(c: Option<$t>) -> Self {
                    c.map_or(ColorSpec::None, Into::into)
                }
            }
        )*
    };
}
from_option!(Color, ColorSpec, i32, &str, String, &String);

crate::error::from_str_via_parse!(Color, "colour");
crate::error::from_r_string!(
    Color,
    "colour",
    "a colour name, \"#RRGGBB\", \"#RRGGBBAA\" or \"transparent\""
);

/// `n` grey levels from dark (0.3) to light (0.9), gamma-corrected (2.2).
///
/// These are the default colours of a matrix `barplot()`.
///
/// This is R's `gray.colors(n)` (`start = 0.3`, `end = 0.9`, `gamma =
/// 2.2`).
///
/// # Examples
///
/// ```
/// # use rs_graphics::prelude::*;
/// let greys = gray_colors(3);
/// assert_eq!(greys[0], ColorSpec::Color(Color::parse("#4D4D4D").unwrap()));
/// assert_eq!(greys[2], ColorSpec::Color(Color::parse("#E6E6E6").unwrap()));
/// ```
pub fn gray_colors(n: usize) -> Vec<ColorSpec> {
    let (gamma, a, b) = (2.2_f64, 0.3_f64.powf(2.2), 0.9_f64.powf(2.2));
    (0..n)
        .map(|i| {
            let t = if n > 1 {
                i as f64 / (n - 1) as f64
            } else {
                0.0
            };
            let level = (a + (b - a) * t).powf(1.0 / gamma);
            let v = (255.0 * level + 0.5) as u8;
            ColorSpec::Color(Color::rgb(v, v, v))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hex() {
        assert_eq!(Color::parse("#FF0000"), Some(Color::rgb(255, 0, 0)));
        assert_eq!(Color::parse("#f00"), Some(Color::rgb(255, 0, 0)));
        assert_eq!(Color::parse("#00000080"), Some(Color::rgba(0, 0, 0, 0x80)));
    }

    #[test]
    fn parses_named() {
        assert_eq!(Color::parse("white"), Some(Color::rgb(255, 255, 255)));
        assert_eq!(Color::parse("Red"), Some(Color::rgb(255, 0, 0)));
        assert_eq!(
            Color::parse("steelblue"),
            Some(Color::rgb(0x46, 0x82, 0xB4))
        );
    }

    #[test]
    fn na_and_transparent() {
        assert_eq!(Color::parse("NA"), None);
        assert_eq!(
            Color::parse("transparent"),
            Some(Color::rgba(255, 255, 255, 0))
        );
        // Neither draws anything.
        for spec in ["NA", "transparent", "#FF000000"] {
            assert_eq!(
                ColorSpec::from(spec).resolve(&DEFAULT_PALETTE),
                None,
                "{spec}"
            );
        }
        assert!(
            ColorSpec::from("#FF000001")
                .resolve(&DEFAULT_PALETTE)
                .is_some()
        );
    }

    #[test]
    fn palette_recycles() {
        let spec = ColorSpec::Index(9);
        assert_eq!(spec.resolve(&DEFAULT_PALETTE), Some(DEFAULT_PALETTE[0]));
    }

    #[test]
    fn from_str_and_string_conversions() {
        assert_eq!("red".parse::<Color>(), Ok(Color::rgb(255, 0, 0)));
        assert!("notacolour".parse::<Color>().is_err());
        let name = String::from("blue");
        let blue = ColorSpec::Color(Color::rgb(0, 0, 255));
        assert_eq!(ColorSpec::from(&name), blue);
        assert_eq!(ColorSpec::from(name), blue);
    }

    #[test]
    fn gray_colors_match_r() {
        // R: gray.colors(1) = "#4D4D4D", gray.colors(3) =
        // "#4D4D4D" "#AEAEAE" "#E6E6E6".
        let hex = |c: &ColorSpec| match c {
            ColorSpec::Color(c) => c.to_hex().to_uppercase(),
            _ => unreachable!(),
        };
        assert_eq!(hex(&gray_colors(1)[0]), "#4D4D4D");
        let g3: Vec<String> = gray_colors(3).iter().map(hex).collect();
        assert_eq!(g3, ["#4D4D4D", "#AEAEAE", "#E6E6E6"]);
    }

    #[test]
    fn names_ignore_spaces_as_r() {
        // R: col2rgb(c("light blue", "Dark Red", " dark red")).
        let light_blue = Some(Color::rgb(173, 216, 230));
        assert_eq!(Color::parse("light blue"), light_blue);
        assert_eq!(Color::parse("Light Blue"), light_blue);
        let dark_red = Some(Color::rgb(139, 0, 0));
        assert_eq!(Color::parse("Dark Red"), dark_red);
        assert_eq!(Color::parse(" dark red"), dark_red);
        assert_eq!(Color::parse("dark  red"), dark_red);
        assert_eq!(Color::parse("dark redd"), None);
    }

    #[test]
    #[should_panic(expected = "numerical color values must be >= 0, found -1")]
    fn negative_index_panics() {
        let _ = ColorSpec::Index(-1).resolve(&DEFAULT_PALETTE);
    }
}
