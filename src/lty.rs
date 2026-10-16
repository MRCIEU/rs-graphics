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
    /// Parse an `lty` string as R's `par(lty=)` does: a name (`"solid"`,
    /// `"dashed"`, ...) or 2, 4, 6 or 8 hex digits (1-9, A-F; no zeros),
    /// each a segment length in `lwd` units, e.g. `"42"` or `"1343"`. One
    /// extra form: a single digit is R's numeric code (`"2"` =
    /// [`LineType::from_code`]`(2)`), which R rejects as a string. `None`
    /// for anything R rejects.
    pub fn parse(spec: &str) -> Option<LineType> {
        let s = spec.trim();
        if s.len() == 1 {
            if let Some(n) = s.chars().next().and_then(|c| c.to_digit(10)) {
                return Some(Self::from_code(n as i32));
            }
        }
        match s.to_ascii_lowercase().as_str() {
            "blank" => Some(LineType::Blank),
            "solid" => Some(LineType::Pattern(&[])),
            "dashed" => Some(LineType::Pattern(&[4, 4])),
            "dotted" => Some(LineType::Pattern(&[1, 3])),
            "dotdash" => Some(LineType::Pattern(&[1, 3, 4, 3])),
            "longdash" => Some(LineType::Pattern(&[7, 3])),
            "twodash" => Some(LineType::Pattern(&[2, 2, 6, 2])),
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
        // enough in practice that we resolve them through `from_code`
        // instead, by re-packing into the same nibble encoding R uses.
        let mut code: i32 = 0;
        for (i, c) in s.chars().enumerate() {
            let nibble = c.to_digit(16).unwrap() as i32;
            code |= nibble << (4 * i);
        }
        Some(Self::from_code(code))
    }

    /// Resolve R's small-integer `lty` codes (`0` = blank, `1` = solid,
    /// `2..=6` the named dash patterns) as well as the packed-nibble
    /// encoding used for literal/custom patterns.
    pub fn from_code(code: i32) -> LineType {
        match code {
            0 => LineType::Blank,
            1 => LineType::Pattern(&[]),
            2 => LineType::Pattern(&[4, 4]),
            3 => LineType::Pattern(&[1, 3]),
            4 => LineType::Pattern(&[1, 3, 4, 3]),
            5 => LineType::Pattern(&[7, 3]),
            6 => LineType::Pattern(&[2, 2, 6, 2]),
            _ => LineType::from_packed_nibbles(code),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_and_numeric_agree() {
        assert_eq!(LineType::parse("dashed"), Some(LineType::from_code(2)));
        assert_eq!(LineType::parse("2"), Some(LineType::from_code(2)));
    }

    #[test]
    fn hex_strings_follow_r() {
        assert_eq!(LineType::parse("42"), Some(LineType::Pattern(&[4, 2])));
        assert_eq!(LineType::parse("44"), Some(LineType::from_code(2)));
        assert_eq!(LineType::parse("4F2A"), Some(LineType::Pattern(&[4, 15, 2, 10])));
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
}
