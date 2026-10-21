// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Line types (`par(lty=)`), ported from the packed-nibble dash-pattern
//! encoding R uses internally (`R_ext/GraphicsEngine.h`: `LTY_SOLID`,
//! `LTY_DASHED`, etc.) and the pattern-extraction loop every device driver
//! repeats (e.g. `SetLineStyle()` in `devPS.c`):
//!
//! ```c
//! for(i = 0; i < 8 && newlty & 15 ; i++) {
//!     dashlist[i] = newlty & 15;
//!     newlty = newlty >> 4;
//! }
//! ```
//!
//! Each nibble is a segment length in units of `lwd` (line width), with
//! even indices "pen down" and odd indices "pen up".

/// A resolved line type: either "blank" (nothing drawn) or a dash pattern
/// in units of `lwd`. An empty pattern means solid.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum LineType {
    Blank,
    Pattern(&'static [u8]),
}

impl LineType {
    /// `lty = 0` / `"blank"`: nothing drawn.
    pub const BLANK: LineType = LineType::Blank;
    /// `lty = 1` / `"solid"` (the default).
    pub const SOLID: LineType = LineType::Pattern(&[]);
    /// `lty = 2` / `"dashed"`.
    pub const DASHED: LineType = LineType::Pattern(&[4, 4]);
    /// `lty = 3` / `"dotted"`.
    pub const DOTTED: LineType = LineType::Pattern(&[1, 3]);
    /// `lty = 4` / `"dotdash"`.
    pub const DOTDASH: LineType = LineType::Pattern(&[1, 3, 4, 3]);
    /// `lty = 5` / `"longdash"`.
    pub const LONGDASH: LineType = LineType::Pattern(&[7, 3]);
    /// `lty = 6` / `"twodash"`.
    pub const TWODASH: LineType = LineType::Pattern(&[2, 2, 6, 2]);

    /// Parse an `lty` string as R's `par(lty=)` does: a name (`"solid"`,
    /// `"dashed"`, ...) or 2, 4, 6 or 8 hex digits (1-9, A-F; no zeros),
    /// each a segment length in `lwd` units, e.g. `"42"` or `"1343"`. One
    /// extra form: a single digit is a numeric code (`"2"` =
    /// [`LineType::from_code`]`(2)`; `"7"` to `"9"` recycle as it does),
    /// which R rejects as a string. `None` for anything R rejects.
    pub fn parse(spec: &str) -> Option<LineType> {
        let s = spec.trim();
        if s.len() == 1 {
            if let Some(n) = s.chars().next().and_then(|c| c.to_digit(10)) {
                return Some(Self::from_code(n as i32));
            }
        }
        match s.to_ascii_lowercase().as_str() {
            "blank" => Some(LineType::BLANK),
            "solid" => Some(LineType::SOLID),
            "dashed" => Some(LineType::DASHED),
            "dotted" => Some(LineType::DOTTED),
            "dotdash" => Some(LineType::DOTDASH),
            "longdash" => Some(LineType::LONGDASH),
            "twodash" => Some(LineType::TWODASH),
            _ => Self::from_hex_literal(s),
        }
    }

    fn from_hex_literal(s: &str) -> Option<LineType> {
        // R: "must be length 2, 4, 6 or 8", "zeroes are not allowed".
        if !matches!(s.len(), 2 | 4 | 6 | 8)
            || !s.chars().all(|c| c.is_ascii_hexdigit() && c != '0')
        {
            return None;
        }
        // Cache literal patterns as owned data isn't possible for a
        // `&'static` slice without leaking; literal lty strings are rare
        // enough in practice that we re-pack them into the same nibble
        // encoding R uses and resolve that.
        let mut code: i32 = 0;
        for (i, c) in s.chars().enumerate() {
            let nibble = c.to_digit(16).unwrap() as i32;
            code |= nibble << (4 * i);
        }
        Some(Self::from_packed_nibbles(code))
    }

    /// R's numeric `lty` codes: `0` = blank, `1` = solid, `2..=6` the named
    /// dash patterns. Codes above 6 recycle as in R's `GE_LTYpar`
    /// (`(code - 1) % 6 + 1`: `7` is solid, `8` dashed, ...).
    ///
    /// # Panics
    /// If `code` is negative (R: "invalid line type").
    pub fn from_code(code: i32) -> LineType {
        assert!(code >= 0, "invalid line type: lty code {code} is negative");
        let code = if code > 0 { (code - 1) % 6 + 1 } else { 0 };
        match code {
            0 => LineType::BLANK,
            1 => LineType::SOLID,
            2 => LineType::DASHED,
            3 => LineType::DOTTED,
            4 => LineType::DOTDASH,
            5 => LineType::LONGDASH,
            6 => LineType::TWODASH,
            _ => unreachable!("lty code recycled to 0..=6"),
        }
    }

    fn from_packed_nibbles(mut code: i32) -> LineType {
        // Matches the well-known static patterns where possible so we can
        // return a `'static` slice without allocating.
        static KNOWN: &[(i32, &[u8])] = &[
            (0, &[]),
            (0x44, &[4, 4]),
            (0x31, &[1, 3]),
            (0x3431, &[1, 3, 4, 3]),
            (0x37, &[7, 3]),
            (0x6622, &[2, 2, 6, 2]),
        ];
        if let Some(&(_, pat)) = KNOWN.iter().find(|&&(c, _)| c == code) {
            return LineType::Pattern(pat);
        }
        // Fall back: rebuild a pattern and leak it (custom lty strings are
        // rare enough in practice; this keeps the API allocation-free for
        // the common cases above).
        let mut nibbles = Vec::new();
        while code != 0 && nibbles.len() < 8 {
            nibbles.push((code & 0xF) as u8);
            code >>= 4;
        }
        let leaked: &'static [u8] = Vec::leak(nibbles);
        LineType::Pattern(leaked)
    }

    /// Dash pattern lengths in points, given `lwd` (R's line-width unit;
    /// `1 lwd` renders as `1/96 inch` on most devices, i.e. `0.75` points).
    /// Unless `butt_cap`, "on" segments are one `lwd` shorter and "off"
    /// segments one `lwd` longer, as `PDFSetLineTexture()` in `devPS.c`
    /// does for round and square caps. Lengths below 0 become 0. `None`
    /// for blank and solid, and when every length is <= 0.01pt (R then
    /// writes an empty, i.e. solid, dash array).
    #[doc(hidden)]
    pub fn dash_lengths_pt(&self, lwd: f64, butt_cap: bool) -> Option<Vec<f64>> {
        match self {
            LineType::Blank => None,
            LineType::Pattern(p) if p.is_empty() => None,
            LineType::Pattern(p) => {
                let base_lwd = lwd * 0.75; // matches devPS.c's `newlwd * 0.75`
                let a = if butt_cap { 0.0 } else { 1.0 };
                let nlty = p.len();
                let dash: Vec<f64> = p
                    .iter()
                    .enumerate()
                    .map(|(i, &seg)| {
                        let seg = seg as f64;
                        let d = base_lwd
                            * if i % 2 == 1 {
                                seg + a
                            } else if nlty == 1 && seg == 1.0 {
                                1.0
                            } else {
                                seg - a
                            };
                        d.max(0.0)
                    })
                    .collect();
                dash.iter().any(|&d| d > 0.01).then_some(dash)
            }
        }
    }
}

impl Default for LineType {
    /// [`LineType::SOLID`], R's default `par(lty)`.
    fn default() -> Self {
        LineType::SOLID
    }
}

impl From<i32> for LineType {
    /// R's numeric `lty` codes, as [`LineType::from_code`] (codes above 6
    /// recycle; a negative code panics).
    fn from(code: i32) -> Self {
        LineType::from_code(code)
    }
}

crate::error::from_str_via_parse!(LineType, "line type");
crate::error::from_r_string!(
    LineType,
    "line type",
    "a name (\"solid\", \"dashed\", ...) or 2, 4, 6 or 8 hex digits"
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_and_numeric_agree() {
        assert_eq!(LineType::parse("dashed"), Some(LineType::from_code(2)));
        assert_eq!(LineType::parse("2"), Some(LineType::from_code(2)));
    }

    #[test]
    fn named_consts_are_rs_codes() {
        let named = [
            LineType::BLANK,
            LineType::SOLID,
            LineType::DASHED,
            LineType::DOTTED,
            LineType::DOTDASH,
            LineType::LONGDASH,
            LineType::TWODASH,
        ];
        for (code, lty) in named.into_iter().enumerate() {
            assert_eq!(LineType::from(code as i32), lty);
        }
        assert_eq!(LineType::default(), LineType::SOLID);
        assert_eq!("dotdash".parse::<LineType>(), Ok(LineType::DOTDASH));
        let err = "wavy".parse::<LineType>().unwrap_err();
        assert_eq!(err.to_string(), "invalid line type: \"wavy\"");
    }

    #[test]
    fn hex_strings_follow_r() {
        assert_eq!(LineType::parse("42"), Some(LineType::Pattern(&[4, 2])));
        assert_eq!(LineType::parse("44"), Some(LineType::from_code(2)));
        assert_eq!(
            LineType::parse("4F2A"),
            Some(LineType::Pattern(&[4, 15, 2, 10]))
        );
        // R: "must be length 2, 4, 6 or 8" / "zeroes are not allowed".
        for bad in ["123", "10", "123456789", "4G", ""] {
            assert_eq!(LineType::parse(bad), None, "{bad}");
        }
    }

    #[test]
    fn blank_has_no_dash_pattern() {
        assert_eq!(LineType::Blank.dash_lengths_pt(1.0, false), None);
    }

    #[test]
    fn solid_has_no_dash_pattern() {
        assert_eq!(LineType::from_code(1).dash_lengths_pt(1.0, false), None);
    }

    #[test]
    fn codes_above_6_recycle_as_r() {
        // R (pdf()): lty 7 and 13 are solid, 8 dashed, 12 twodash.
        assert_eq!(LineType::from_code(7), LineType::SOLID);
        assert_eq!(LineType::from_code(8), LineType::DASHED);
        assert_eq!(LineType::from_code(12), LineType::TWODASH);
        assert_eq!(LineType::from_code(13), LineType::SOLID);
        assert_eq!(LineType::from(8), LineType::DASHED);
        assert_eq!(LineType::parse("7"), Some(LineType::SOLID));
        // Hex strings are still patterns, not codes.
        assert_eq!(LineType::parse("77"), Some(LineType::Pattern(&[7, 7])));
    }

    #[test]
    #[should_panic(expected = "invalid line type")]
    fn negative_code_panics() {
        let _ = LineType::from_code(-1);
    }
}
