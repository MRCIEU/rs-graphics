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
//! # let mut dev = Device::null().width(5.0).height(5.0).open().unwrap();
//! dev.plot_new();
//! dev.text().x(&[0.5]).y(&[0.5]).labels("a").font(2).draw(); // R's number
//! dev.text().x(&[0.5]).y(&[0.4]).labels("b").font("bold").draw();
//! dev.text().x(&[0.5]).y(&[0.3]).labels("c").font(FontFace::Bold).draw();
//! ```
//!
//! These are R's `font` numbers 1 to 5.

/// A font face: plain, bold, italic, bold italic or the Symbol font.
///
/// Convert from a number 1 to 5 (`FontFace::try_from(2)`)
/// or a name (`"bold".parse::<FontFace>()`, `TryFrom<&str>`): grid's
/// `fontface` names (`"plain"`, `"bold"`, `"italic"`, `"bold.italic"`), plus
/// `"symbol"`, case-sensitive, or a number 1 to 5 as a string (`"2"`).
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
/// assert_eq!(FontFace::try_from(2), Ok(FontFace::Bold));
/// assert_eq!(FontFace::try_from("bold.italic").unwrap().code(), 4);
/// assert_eq!(FontFace::default(), FontFace::Plain);
/// assert_eq!("italic".parse::<FontFace>().unwrap(), FontFace::Italic);
/// assert_eq!("3".parse::<FontFace>().unwrap(), FontFace::Italic);
/// assert!("heavy".parse::<FontFace>().is_err());
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
    /// The face for a font number 1 to 5, `None` for any other; the public
    /// route is `TryFrom<i32>`.
    pub(crate) fn from_code(code: i32) -> Option<FontFace> {
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
    /// `FontFace::try_from(i32)`).
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

    /// A face name or number as a string, `None` if it is neither; the public
    /// routes are `FromStr`/`TryFrom<&str>`.
    pub(crate) fn parse(s: &str) -> Option<FontFace> {
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

impl TryFrom<i32> for FontFace {
    type Error = crate::error::Error;

    /// The face for a font number 1 to 5, R's `font` values: 1 plain, 2
    /// bold, 3 italic, 4 bold italic, 5 Symbol.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// assert_eq!(FontFace::try_from(3).unwrap(), FontFace::Italic);
    /// assert!(FontFace::try_from(0).is_err());
    /// ```
    ///
    /// # Errors
    ///
    /// If `code` isn't 1 to 5 (R errors on an invalid [`par(font)`]).
    ///
    /// [`par(font)`]: crate::par::Par::font
    fn try_from(code: i32) -> Result<Self, Self::Error> {
        FontFace::from_code(code).ok_or_else(|| {
            crate::error::Error::invalid(format!(
                "invalid font {code}: R's fonts are 1 (plain) to 5 (symbol)"
            ))
        })
    }
}

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
            assert_eq!(FontFace::try_from(code).unwrap().code(), code);
        }
        assert_eq!(FontFace::default(), FontFace::Plain);
        assert_eq!(FontFace::try_from("bold.italic"), Ok(FontFace::BoldItalic));
        assert_eq!(FontFace::parse("3"), Some(FontFace::Italic));
        assert_eq!("symbol".parse::<FontFace>().ok(), Some(FontFace::Symbol));
        assert_eq!(FontFace::parse("heavy"), None);
        assert_eq!(FontFace::from_code(0), None);
    }

    #[test]
    fn invalid_numbers_and_names_are_errors() {
        let e = FontFace::try_from(7).unwrap_err();
        assert!(e.message().starts_with("invalid font 7"), "{e}");
        let e = FontFace::try_from("heavy").unwrap_err();
        assert!(
            e.message().starts_with("\"heavy\" is not a valid font"),
            "{e}"
        );
    }
}
