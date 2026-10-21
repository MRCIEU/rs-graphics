// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Color parsing and the R color model.
//!
//! R identifies colors via strings (`"red"`, `"#FF0000"`, `"#FF000080"`),
//! small integers that index the current palette ([`ColorSpec::Index`];
//! R's default is [`DEFAULT_PALETTE`]), or `NA`
//! (meaning "don't draw"). This module mirrors that model closely enough
//! that [`Color::parse`] accepts the same strings R's `col2rgb()`/graphics
//! code would accept, except numeric strings such as `"2"` (R's palette
//! indices; use [`ColorSpec::Index`]).

use crate::named_colors::NAMED_COLORS;

/// An RGBA color, 8 bits per channel - the same representation R uses
/// internally (`rcolor` is a packed `AABBGGRR` `u32`).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Color { r, g, b, a: 255 }
    }

    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Color { r, g, b, a }
    }

    pub const TRANSPARENT: Color = Color::rgba(0, 0, 0, 0);
    pub const BLACK: Color = Color::rgb(0, 0, 0);
    pub const WHITE: Color = Color::rgb(255, 255, 255);

    /// Parse a color specification the way R's graphics code would:
    /// `"NA"`/empty => `None`; `"transparent"` => white with alpha 0 (R's
    /// `#FFFFFF00`); `"#rgb"`, `"#rgba"`, `"#rrggbb"`,
    /// `"#rrggbbaa"` hex forms; or one of R's 657 built-in named colors
    /// (case-insensitive and ignoring spaces, as R's `StrMatch()`:
    /// `"Light Blue"` is `"lightblue"`).
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

    /// Render as `#rrggbb` (or `#rrggbbaa` if not fully opaque).
    pub fn to_hex(self) -> String {
        if self.a == 255 {
            format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
        } else {
            format!("#{:02x}{:02x}{:02x}{:02x}", self.r, self.g, self.b, self.a)
        }
    }

    pub fn with_alpha(self, a: u8) -> Color {
        Color { a, ..self }
    }
}

/// R's default `palette()` since R 4.0.0 (the "R4" palette). `col = 1` is
/// the first entry (black), `col = 2` the second, etc. - R's small-integer
/// colors are 1-indexed.
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

/// A resolvable color specification: either an explicit color, a palette
/// index (R's small-integer colors, 1-based; indices above the palette
/// length recycle as in R, and a negative one panics when resolved, as R
/// errors), or "no color" (R's `NA`).
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum ColorSpec {
    None,
    Color(Color),
    Index(i32),
}

impl ColorSpec {
    /// The colour to draw with, or `None` for nothing: `NA`, index 0, and
    /// fully transparent colours (R's devices skip those). Index 0 is the
    /// background colour in R, which this doesn't know; use
    /// [`Par::resolve_color`](crate::par::Par::resolve_color) for that.
    ///
    /// # Panics
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

/// R's `gray.colors(n)` (`start = 0.3`, `end = 0.9`, `gamma = 2.2`).
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
