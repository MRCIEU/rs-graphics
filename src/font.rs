// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Font faces: plain, bold, italic, bold italic and Symbol, as the type
//! [`FontFace`].
//!
//! Every font argument (`par()`'s `font`, `font_main`, `font_axis`, ...,
//! `text()`'s `font`, `legend()`'s `text_font`) is a [`FontFace`]. Setters
//! take R's number, the name, or the enum:
//!
//! ```
//! # use rs_graphics::prelude::*;
//! # let mut dev = Device::new(SvgBackend::new(5.0, 5.0), Par::default());
//! dev.plot_new();
//! dev.text().x(&[0.5]).y(&[0.5]).labels("a").font(2).draw(); // R's number
//! dev.text().x(&[0.5]).y(&[0.4]).labels("b").font("bold").draw();
//! dev.text().x(&[0.5]).y(&[0.3]).labels("c").font(FontFace::Bold).draw();
//! ```
//!
//! These are R's `font` numbers 1 to 5.

/// A font face: plain, bold, italic, bold italic or the Symbol font.
///
/// Convert from a number 1 to 5 ([`FontFace::from_code`], `From<i32>`) or
/// a name ([`FontFace::parse`], `From<&str>`): grid's `fontface` names
/// (`"plain"`, `"bold"`, `"italic"`, `"bold.italic"`), plus `"symbol"`.
/// The default is [`FontFace::Plain`].
///
/// Text in [`FontFace::Symbol`] is in Adobe Symbol encoding, one `char`
/// per code (e.g. `'a'` is alpha, `'\u{B1}'` plus-minus), as R's
/// `pdf()` writes it; devices that need Unicode map it.
///
/// These are R's `font` numbers 1 to 5 (`par(font = )`).
///
/// # Examples
///
/// ```
/// # use rs_graphics::prelude::*;
/// assert_eq!(FontFace::from(2), FontFace::Bold);
/// assert_eq!(FontFace::from("bold.italic").code(), 4);
/// assert_eq!(FontFace::default(), FontFace::Plain);
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum FontFace {
    /// Plain (the default); 1, `"plain"`.
    #[default]
    Plain,
    /// Bold; 2, `"bold"`.
    Bold,
    /// Italic; 3, `"italic"`.
    Italic,
    /// Bold italic; 4, `"bold.italic"`.
    BoldItalic,
    /// The Symbol font; 5, `"symbol"`.
    Symbol,
}

impl FontFace {
    /// Returns the face for a font number 1 to 5, or `None` for any other.
    ///
    /// The numbers are R's `font` values.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// assert_eq!(FontFace::from_code(3), Some(FontFace::Italic));
    /// assert_eq!(FontFace::from_code(0), None);
    /// ```
    pub fn from_code(code: i32) -> Option<FontFace> {
        match code {
            1 => Some(FontFace::Plain),
            2 => Some(FontFace::Bold),
            3 => Some(FontFace::Italic),
            4 => Some(FontFace::BoldItalic),
            5 => Some(FontFace::Symbol),
            _ => None,
        }
    }

    /// Returns the face's number, 1 to 5 (the inverse of
    /// [`FontFace::from_code`]).
    ///
    /// The numbers are R's `font` values.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// assert_eq!(FontFace::Bold.code(), 2);
    /// ```
    pub fn code(self) -> i32 {
        match self {
            FontFace::Plain => 1,
            FontFace::Bold => 2,
            FontFace::Italic => 3,
            FontFace::BoldItalic => 4,
            FontFace::Symbol => 5,
        }
    }

    /// Parses a face name or number, or returns `None` if `s` is neither.
    ///
    /// Takes grid's names (`"plain"`, `"bold"`, `"italic"`,
    /// `"bold.italic"`), `"symbol"`, or a number 1 to 5 as a string
    /// (`"2"`). Names are case-sensitive.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// assert_eq!(FontFace::parse("italic"), Some(FontFace::Italic));
    /// assert_eq!(FontFace::parse("3"), Some(FontFace::Italic));
    /// assert_eq!(FontFace::parse("heavy"), None);
    /// ```
    pub fn parse(s: &str) -> Option<FontFace> {
        match s {
            "plain" => Some(FontFace::Plain),
            "bold" => Some(FontFace::Bold),
            "italic" => Some(FontFace::Italic),
            "bold.italic" => Some(FontFace::BoldItalic),
            "symbol" => Some(FontFace::Symbol),
            _ => s.parse().ok().and_then(FontFace::from_code),
        }
    }
}

impl From<i32> for FontFace {
    /// The face for a font number 1 to 5, as [`FontFace::from_code`].
    ///
    /// # Panics
    ///
    /// If `code` isn't 1 to 5 (R errors on an invalid `par(font)`).
    fn from(code: i32) -> Self {
        FontFace::from_code(code)
            .unwrap_or_else(|| panic!("invalid font {code}: R's fonts are 1 (plain) to 5 (symbol)"))
    }
}

crate::error::from_str_via_parse!(FontFace, "font");
crate::error::from_r_string!(
    FontFace,
    "font",
    "\"plain\", \"bold\", \"italic\", \"bold.italic\", \"symbol\" or 1 to 5"
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn faces_convert_like_r() {
        for code in 1..=5 {
            assert_eq!(FontFace::from(code).code(), code);
        }
        assert_eq!(FontFace::default(), FontFace::Plain);
        assert_eq!(FontFace::from("bold.italic"), FontFace::BoldItalic);
        assert_eq!(FontFace::parse("3"), Some(FontFace::Italic));
        assert_eq!("symbol".parse::<FontFace>().ok(), Some(FontFace::Symbol));
        assert_eq!(FontFace::parse("heavy"), None);
        assert_eq!(FontFace::from_code(0), None);
    }

    #[test]
    #[should_panic(expected = "invalid font 7")]
    fn invalid_numbers_panic() {
        let _ = FontFace::from(7);
    }

    #[test]
    #[should_panic(expected = "\"heavy\" is not a valid font")]
    fn invalid_names_panic() {
        let _ = FontFace::from("heavy");
    }
}
