// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Font faces: R's `font` numbers as a named type.
//!
//! Every font argument (`par(font)`, `font.main`, `font.axis`, ...,
//! `text(font = )`, `legend(text.font = )`) is a [`FontFace`]. Setters take
//! R's number, the name, or the enum:
//!
//! ```
//! # use rs_graphics::prelude::*;
//! # let mut dev = Device::new(SvgBackend::new(5.0, 5.0), Par::default());
//! dev.plot_new();
//! dev.text().x(&[0.5]).y(&[0.5]).labels("a").font(2).draw(); // R's number
//! dev.text().x(&[0.5]).y(&[0.4]).labels("b").font("bold").draw();
//! dev.text().x(&[0.5]).y(&[0.3]).labels("c").font(FontFace::Bold).draw();
//! ```

/// One of R's font faces, `par(font = )`'s numbers 1 to 5: plain, bold,
/// italic, bold italic and the Symbol font. The names are grid's
/// `fontface` names (`"plain"`, `"bold"`, `"italic"`, `"bold.italic"`),
/// plus `"symbol"`.
///
/// Text in [`FontFace::Symbol`] is in Adobe Symbol encoding, one `char`
/// per code (e.g. `'a'` is alpha, `'\u{B1}'` plus-minus), as R's
/// `pdf()` writes it; devices that need Unicode map it.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum FontFace {
    /// 1, `"plain"` (the default).
    #[default]
    Plain,
    /// 2, `"bold"`.
    Bold,
    /// 3, `"italic"`.
    Italic,
    /// 4, `"bold.italic"`.
    BoldItalic,
    /// 5, `"symbol"`: the Symbol font.
    Symbol,
}

impl FontFace {
    /// The face for R's font number (1 to 5), or `None` for any other.
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

    /// R's font number, 1 to 5.
    pub fn code(self) -> i32 {
        match self {
            FontFace::Plain => 1,
            FontFace::Bold => 2,
            FontFace::Italic => 3,
            FontFace::BoldItalic => 4,
            FontFace::Symbol => 5,
        }
    }

    /// grid's name (`"plain"`, `"bold"`, `"italic"`, `"bold.italic"`) or
    /// `"symbol"`, or R's number as a string (`"2"`).
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
    /// R's font number.
    ///
    /// # Panics
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
