// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Font width metrics from the standard Adobe Helvetica AFM files that R
//! itself ships (`r-source/src/library/grDevices/inst/afm/Helvetica*.afm`)
//! and uses by default for the `pdf()`/`postscript()` devices' string
//! width/height calculations. Using the same tables (including kerning
//! pairs) means `strwidth()` layout math (label centering, legend box
//! sizing, axis label offsets, etc.) matches R almost exactly, regardless
//! of which backend later rasterizes the glyphs.
//!
//! This mirrors `PostScriptLoadFontMetrics()`/`PostScriptStringWidth()` in
//! `r-source/src/library/grDevices/src/devPS.c`, including that
//! implementation's documented quirk of *not* applying kerning to any pair
//! involving the `space` glyph (`GetKPX()`: `if (streql(c1, "space") ||
//! streql(c2, "space")) return 0;`).
//!
//! Note: R's C implementation also has an (undocumented, almost certainly
//! unintentional) indexing bug in the way it builds its kern-pair lookup
//! table: `KPend` in `devPS.c` is stored inclusive but looped over
//! exclusive, so R never applies the *last* kerning pair of each first
//! character. We deliberately do *not* replicate that bug here - we apply
//! every kerning pair the AFM file defines (other than the documented
//! space-exclusion above). Strings containing such a pair (e.g. `F r` in
//! "Frequency") are therefore a fraction of a point wider or narrower than
//! in R's `pdf()` output.

use std::collections::HashMap;
use std::sync::OnceLock;

/// The font faces R exposes via `par(font=)`/`par(font.axis=)` etc.: `1` =
/// plain, `2` = bold, `3` = italic, `4` = bold italic, `5` = symbol.
///
/// Text in [`FontFace::Symbol`] is in Adobe Symbol encoding, one `char`
/// per code (e.g. `'a'` is alpha, `'\u{B1}'` plus-minus), as R's
/// `pdf()` writes it; [`symbol_to_unicode`] maps it for devices that need
/// Unicode.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum FontFace {
    Plain,
    Bold,
    Italic,
    BoldItalic,
    Symbol,
}

impl FontFace {
    pub fn from_r_font_code(code: i32) -> Self {
        match code {
            2 => FontFace::Bold,
            3 => FontFace::Italic,
            4 => FontFace::BoldItalic,
            5 => FontFace::Symbol,
            _ => FontFace::Plain,
        }
    }

    fn afm_source(self) -> &'static str {
        match self {
            FontFace::Plain => include_str!("../assets/afm/Helvetica.afm"),
            FontFace::Bold => include_str!("../assets/afm/Helvetica-Bold.afm"),
            FontFace::Italic => include_str!("../assets/afm/Helvetica-Oblique.afm"),
            FontFace::BoldItalic => include_str!("../assets/afm/Helvetica-BoldOblique.afm"),
            FontFace::Symbol => include_str!("../assets/afm/Symbol.afm"),
        }
    }
}

/// Unicode for an Adobe Symbol code (R's `s2unicode` table in `util.c`,
/// the variant without Private Use Area code points). Codes below 32 map
/// to themselves; codes above 255 aren't Symbol codes and are returned
/// unchanged.
pub fn symbol_to_unicode(c: char) -> char {
    #[rustfmt::skip]
    const S2UNICODE: [u16; 224] = [
        0x0020, 0x0021, 0x2200, 0x0023, 0x2203, 0x0025, 0x0026, 0x220D,
        0x0028, 0x0029, 0x2217, 0x002B, 0x002C, 0x2212, 0x002E, 0x002F,
        0x0030, 0x0031, 0x0032, 0x0033, 0x0034, 0x0035, 0x0036, 0x0037,
        0x0038, 0x0039, 0x003A, 0x003B, 0x003C, 0x003D, 0x003E, 0x003F,
        0x2245, 0x0391, 0x0392, 0x03A7, 0x0394, 0x0395, 0x03A6, 0x0393,
        0x0397, 0x0399, 0x03D1, 0x039A, 0x039B, 0x039C, 0x039D, 0x039F,
        0x03A0, 0x0398, 0x03A1, 0x03A3, 0x03A4, 0x03A5, 0x03C2, 0x03A9,
        0x039E, 0x03A8, 0x0396, 0x005B, 0x2234, 0x005D, 0x22A5, 0x005F,
        0x23AF, 0x03B1, 0x03B2, 0x03C7, 0x03B4, 0x03B5, 0x03C6, 0x03B3,
        0x03B7, 0x03B9, 0x03D5, 0x03BA, 0x03BB, 0x03BC, 0x03BD, 0x03BF,
        0x03C0, 0x03B8, 0x03C1, 0x03C3, 0x03C4, 0x03C5, 0x03D6, 0x03C9,
        0x03BE, 0x03C8, 0x03B6, 0x007B, 0x007C, 0x007D, 0x223C, 0x0020,
        0x0020, 0x0020, 0x0020, 0x0020, 0x0020, 0x0020, 0x0020, 0x0020,
        0x0020, 0x0020, 0x0020, 0x0020, 0x0020, 0x0020, 0x0020, 0x0020,
        0x0020, 0x0020, 0x0020, 0x0020, 0x0020, 0x0020, 0x0020, 0x0020,
        0x0020, 0x0020, 0x0020, 0x0020, 0x0020, 0x0020, 0x0020, 0x0020,
        0x20AC, 0x03D2, 0x2032, 0x2264, 0x2044, 0x221E, 0x0192, 0x2663,
        0x2666, 0x2665, 0x2660, 0x2194, 0x2190, 0x2191, 0x2192, 0x2193,
        0x00B0, 0x00B1, 0x2033, 0x2265, 0x00D7, 0x221D, 0x2202, 0x2022,
        0x00F7, 0x2260, 0x2261, 0x2248, 0x2026, 0x23D0, 0x23AF, 0x21B5,
        0x2135, 0x2111, 0x211C, 0x2118, 0x2297, 0x2295, 0x2205, 0x2229,
        0x222A, 0x2283, 0x2287, 0x2284, 0x2282, 0x2286, 0x2208, 0x2209,
        0x2220, 0x2207, 0x00AE, 0x00A9, 0x2122, 0x220F, 0x221A, 0x22C5,
        0x00AC, 0x2227, 0x2228, 0x21D4, 0x21D0, 0x21D1, 0x21D2, 0x21D3,
        0x25CA, 0x2329, 0x00AE, 0x00A9, 0x2122, 0x2211, 0x239B, 0x239C,
        0x239D, 0x23A1, 0x23A2, 0x23A3, 0x23A7, 0x23A8, 0x23A9, 0x23AA,
        0x0020, 0x232A, 0x222B, 0x2320, 0x23AE, 0x2321, 0x239E, 0x239F,
        0x23A0, 0x23A4, 0x23A5, 0x23A6, 0x23AB, 0x23AC, 0x23AD, 0x0020,
    ];
    match c as u32 {
        code @ 32..=255 => char::from_u32(S2UNICODE[code as usize - 32] as u32).unwrap_or(c),
        _ => c,
    }
}

/// Glyph names of Latin-1 code points 0xA0..=0xFF, from R's
/// `grDevices/enc/ISOLatin1.enc` (identical to WinAnsiEncoding in this range).
const LATIN1_UPPER_NAMES: [&str; 96] = [
    "space", "exclamdown", "cent", "sterling", "currency", "yen", "brokenbar", "section",
    "dieresis", "copyright", "ordfeminine", "guillemotleft", "logicalnot", "hyphen", "registered", "macron",
    "degree", "plusminus", "twosuperior", "threesuperior", "acute", "mu", "paragraph", "periodcentered",
    "cedilla", "onesuperior", "ordmasculine", "guillemotright", "onequarter", "onehalf", "threequarters", "questiondown",
    "Agrave", "Aacute", "Acircumflex", "Atilde", "Adieresis", "Aring", "AE", "Ccedilla",
    "Egrave", "Eacute", "Ecircumflex", "Edieresis", "Igrave", "Iacute", "Icircumflex", "Idieresis",
    "Eth", "Ntilde", "Ograve", "Oacute", "Ocircumflex", "Otilde", "Odieresis", "multiply",
    "Oslash", "Ugrave", "Uacute", "Ucircumflex", "Udieresis", "Yacute", "Thorn", "germandbls",
    "agrave", "aacute", "acircumflex", "atilde", "adieresis", "aring", "ae", "ccedilla",
    "egrave", "eacute", "ecircumflex", "edieresis", "igrave", "iacute", "icircumflex", "idieresis",
    "eth", "ntilde", "ograve", "oacute", "ocircumflex", "otilde", "odieresis", "divide",
    "oslash", "ugrave", "uacute", "ucircumflex", "udieresis", "yacute", "thorn", "ydieresis",
];

/// Character widths and kerning pairs (in 1/1000 em units, the AFM
/// convention) for a single font face, indexed by Latin-1 code point
/// (ASCII and 0xA0..=0xFF; other characters get the space width).
pub(crate) struct FontMetrics {
    widths: [u16; 256],
    /// Per-glyph bounding-box `ury` (AFM `B llx lly urx ury`), in 1/1000
    /// em units; `PostScriptMetricInfo()` reports this as the glyph's
    /// ascent.
    bbox_ury: [i16; 256],
    /// Per-glyph bounding-box `lly`; `PostScriptMetricInfo()` reports
    /// `-lly` as the glyph's descent.
    bbox_lly: [i16; 256],
    pub ascent_units: f64,
    pub descent_units: f64,
    default_width: u16,
    kerning: HashMap<(u8, u8), i16>,
}

impl FontMetrics {
    /// `symbol`: index glyphs by the AFM's own codes (the font-specific
    /// Symbol encoding) instead of R's ISOLatin1 text encoding.
    fn parse(afm: &str, symbol: bool) -> Self {
        let mut ascent_units = 718.0;
        let mut descent_units = -207.0;
        // Every glyph's (WX, lly, ury) by name, including the ones the AFM
        // leaves unencoded (`C -1`), and the names of the ASCII codes.
        let mut glyphs: HashMap<String, (u16, i16, i16)> = HashMap::new();
        let mut code_names: [Option<String>; 256] = std::array::from_fn(|_| None);
        let mut kern_pairs: Vec<(String, String, i16)> = Vec::new();

        for line in afm.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("Ascender ") {
                if let Ok(v) = rest.trim().parse::<f64>() {
                    ascent_units = v;
                }
            } else if let Some(rest) = line.strip_prefix("Descender ") {
                if let Ok(v) = rest.trim().parse::<f64>() {
                    descent_units = v;
                }
            } else if line.starts_with("C ") {
                // "C 32 ; WX 278 ; N space ; B 0 0 0 0 ;"
                let mut code: Option<i64> = None;
                let mut wx: Option<u16> = None;
                let mut name: Option<&str> = None;
                let mut bbox_val: Option<(i16, i16)> = None;
                for field in line.split(';') {
                    let field = field.trim();
                    if let Some(v) = field.strip_prefix("B ") {
                        let n: Vec<i16> = v
                            .split_whitespace()
                            .filter_map(|t| t.parse().ok())
                            .collect();
                        if n.len() == 4 {
                            bbox_val = Some((n[1], n[3]));
                        }
                    } else if let Some(v) = field.strip_prefix("C ") {
                        code = v.trim().parse().ok();
                    } else if let Some(v) = field.strip_prefix("WX ") {
                        wx = v.trim().parse().ok();
                    } else if let Some(v) = field.strip_prefix("N ") {
                        name = Some(v.trim());
                    }
                }
                if let (Some(name), Some(wx)) = (name, wx) {
                    let (lly, ury) = bbox_val.unwrap_or((0, 0));
                    glyphs.insert(name.to_string(), (wx, lly, ury));
                    if let Some(code) = code.filter(|c| (0..256).contains(c)) {
                        code_names[code as usize] = Some(name.to_string());
                    }
                }
            } else if let Some(rest) = line.strip_prefix("KPX ") {
                // "KPX A C -30" (see GetKPX() in devPS.c)
                let mut parts = rest.split_whitespace();
                let (n1, n2, kern) = (parts.next(), parts.next(), parts.next());
                if let (Some(n1), Some(n2), Some(kern)) = (n1, n2, kern) {
                    // R's GetKPX() explicitly drops any pair involving "space".
                    if n1 == "space" || n2 == "space" {
                        continue;
                    }
                    if let Ok(kern) = kern.parse::<i16>() {
                        kern_pairs.push((n1.to_string(), n2.to_string(), kern));
                    }
                }
            }
        }

        // Code -> glyph name, as R's pdf()/postscript() devices map single-byte
        // text through their encoding (ISOLatin1.enc by default): ASCII as the
        // AFM's own (Standard Encoding) codes, which agree with ISOLatin1 there
        // apart from `-` (R's `/minus`, deliberately not replicated); the upper
        // half by Latin-1 glyph name; 128-159 unmapped.
        // The Symbol font keeps its own encoding for every code.
        let mut names: [Option<&str>; 256] = [None; 256];
        for (code, name) in code_names.iter().enumerate() {
            if symbol || code < 128 {
                names[code] = name.as_deref();
            }
        }
        if !symbol {
            for (i, name) in LATIN1_UPPER_NAMES.iter().enumerate() {
                names[0xA0 + i] = Some(name);
            }
        }

        let mut widths = [0u16; 256];
        let mut bbox_ury = [0i16; 256];
        let mut bbox_lly = [0i16; 256];
        let mut codes_of: HashMap<&str, Vec<u8>> = HashMap::new();
        for (code, name) in names.iter().enumerate() {
            let Some(name) = name else { continue };
            if let Some(&(wx, lly, ury)) = glyphs.get(*name) {
                widths[code] = wx;
                bbox_lly[code] = lly;
                bbox_ury[code] = ury;
                codes_of.entry(name).or_default().push(code as u8);
            }
        }
        let mut kerning: HashMap<(u8, u8), i16> = HashMap::new();
        for (n1, n2, kern) in &kern_pairs {
            if let (Some(c1s), Some(c2s)) = (codes_of.get(n1.as_str()), codes_of.get(n2.as_str())) {
                for &c1 in c1s {
                    for &c2 in c2s {
                        kerning.insert((c1, c2), *kern);
                    }
                }
            }
        }
        // A "space" width is a reasonable fallback for anything unmapped
        // rather than 0.
        let default_width = widths[b' ' as usize].max(1);
        FontMetrics {
            widths,
            bbox_ury,
            bbox_lly,
            ascent_units,
            descent_units,
            default_width,
            kerning,
        }
    }

    fn char_width_units(&self, c: char) -> f64 {
        let code = c as u32;
        if code < 256 {
            let w = self.widths[code as usize];
            if w > 0 {
                return w as f64;
            }
        }
        self.default_width as f64
    }

    /// Ascent of one glyph's bounding box in 1/1000 em units, as reported
    /// by R's `PostScriptMetricInfo()`. Characters outside Latin-1 fall
    /// back to `0`.
    fn glyph_ascent_units(&self, c: char) -> f64 {
        let code = c as u32;
        if code < 256 {
            self.bbox_ury[code as usize] as f64
        } else {
            0.0
        }
    }

    /// Lowest point of one glyph's bounding box (AFM `lly`), in 1/1000 em
    /// units; `0` outside Latin-1.
    fn glyph_lly_units(&self, c: char) -> f64 {
        let code = c as u32;
        if code < 256 {
            self.bbox_lly[code as usize] as f64
        } else {
            0.0
        }
    }

    fn kern_units(&self, a: char, b: char) -> f64 {
        let (ca, cb) = (a as u32, b as u32);
        if ca < 256 && cb < 256 {
            self.kerning
                .get(&(ca as u8, cb as u8))
                .copied()
                .unwrap_or(0) as f64
        } else {
            0.0
        }
    }

    /// Width of `s` set at 1000-unit em size (i.e. divide by 1000 and
    /// multiply by the point size to get real units), including kerning
    /// adjustments between adjacent characters (matching
    /// `PostScriptStringWidth()`'s `useKerning = true` default).
    pub fn str_width_units(&self, s: &str) -> f64 {
        let chars: Vec<char> = s.chars().collect();
        let mut total = 0.0;
        for i in 0..chars.len() {
            total += self.char_width_units(chars[i]);
            if i + 1 < chars.len() {
                total += self.kern_units(chars[i], chars[i + 1]);
            }
        }
        total
    }
}

fn metrics_table(face: FontFace) -> &'static FontMetrics {
    static PLAIN: OnceLock<FontMetrics> = OnceLock::new();
    static BOLD: OnceLock<FontMetrics> = OnceLock::new();
    static ITALIC: OnceLock<FontMetrics> = OnceLock::new();
    static BOLD_ITALIC: OnceLock<FontMetrics> = OnceLock::new();
    static SYMBOL: OnceLock<FontMetrics> = OnceLock::new();
    let cell = match face {
        FontFace::Plain => &PLAIN,
        FontFace::Bold => &BOLD,
        FontFace::Italic => &ITALIC,
        FontFace::BoldItalic => &BOLD_ITALIC,
        FontFace::Symbol => &SYMBOL,
    };
    cell.get_or_init(|| FontMetrics::parse(face.afm_source(), face == FontFace::Symbol))
}

/// Width of `s`, in inches, set in `face` at `pointsize * cex` points -
/// exactly analogous to R's `strwidth(s, units = "inches")` with the
/// `pdf`/`postscript` Helvetica metrics.
pub fn str_width_inches(s: &str, face: FontFace, pointsize: f64, cex: f64) -> f64 {
    let pt = pointsize * cex;
    metrics_table(face).str_width_units(s) / 1000.0 * pt / 72.0
}

/// Cumulative per-character x-offsets (in inches, from the string's left
/// edge), including kerning between adjacent characters exactly as
/// str_width_inches does. Returns `s.chars().count() + 1` values: the
/// left edge of each character in turn, followed by the total string
/// width (i.e. the last value equals str_width_inches(s, ..)). Used by
/// backends that must position individual glyphs themselves (currently
/// only the raster/PNG backend) while keeping the exact same layout math
/// used everywhere else.
pub fn char_x_offsets_inches(s: &str, face: FontFace, pointsize: f64, cex: f64) -> Vec<f64> {
    let pt = pointsize * cex;
    let scale = pt / 72.0 / 1000.0;
    let m = metrics_table(face);
    let chars: Vec<char> = s.chars().collect();
    let mut offsets = Vec::with_capacity(chars.len() + 1);
    let mut total = 0.0;
    offsets.push(0.0);
    for i in 0..chars.len() {
        total += m.char_width_units(chars[i]);
        if i + 1 < chars.len() {
            total += m.kern_units(chars[i], chars[i + 1]);
        }
        offsets.push(total * scale);
    }
    offsets
}

/// Nominal font height in inches: the AFM's `Ascender - Descender`. This
/// is not R's `strheight()`, which is the ascent of `M` (see
/// [`m_height_inches`]).
pub fn str_height_inches(face: FontFace, pointsize: f64, cex: f64) -> f64 {
    let m = metrics_table(face);
    let pt = pointsize * cex;
    (m.ascent_units - m.descent_units) / 1000.0 * pt / 72.0
}

/// `(ascent, descent)` in inches - the distances the font's nominal
/// bounding box extends above and below the baseline, respectively (both
/// non-negative), from the AFM's `Ascender`/`Descender`. R's text
/// placement doesn't use these: see [`m_height_inches`] and
/// [`glyph_ascent_descent_inches`].
pub fn ascent_descent_inches(face: FontFace, pointsize: f64, cex: f64) -> (f64, f64) {
    let m = metrics_table(face);
    let pt = pointsize * cex;
    let scale = pt / 72.0 / 1000.0;
    (m.ascent_units * scale, -m.descent_units * scale)
}

/// Height in inches of the glyph `M` - what R's `strheight()` returns for
/// a single-line string (`GEStrHeight()` uses the ascent of `M`, not the
/// string's own ink). This is the `height` R's `GEText()` multiplies `yc`
/// by when positioning text with a finite vertical adjustment.
pub fn m_height_inches(face: FontFace, pointsize: f64, cex: f64) -> f64 {
    let m = metrics_table(face);
    m.glyph_ascent_units('M') / 1000.0 * pointsize * cex / 72.0
}

/// `(ascent, descent, width)` of one glyph in inches, as R's
/// `PostScriptMetricInfo()` reports it for a single character: the AFM
/// `B` box's `ury` and `-lly`, and `WX`.
pub fn glyph_metric_inches(c: char, face: FontFace, pointsize: f64) -> (f64, f64, f64) {
    let m = metrics_table(face);
    let scale = pointsize / 72.0 / 1000.0;
    (
        m.glyph_ascent_units(c) * scale,
        -m.glyph_lly_units(c) * scale,
        m.char_width_units(c) * scale,
    )
}

/// `(ascent, descent)` of the ink of `s` in inches, as R's
/// `PostScriptMetricInfo()` reports it: the maximum glyph `ury` and minus
/// the minimum glyph `lly` (so descent is positive below the baseline).
/// `GEText()` uses these to centre text vertically when `adj[2]` is `NA`
/// (e.g. character `pch`).
pub fn glyph_ascent_descent_inches(s: &str, face: FontFace, pointsize: f64, cex: f64) -> (f64, f64) {
    let m = metrics_table(face);
    let scale = pointsize * cex / 72.0 / 1000.0;
    let mut chars = s.chars();
    let Some(first) = chars.next() else {
        return (0.0, 0.0);
    };
    let (mut ury, mut lly) = (m.glyph_ascent_units(first), m.glyph_lly_units(first));
    for c in chars {
        ury = ury.max(m.glyph_ascent_units(c));
        lly = lly.min(m.glyph_lly_units(c));
    }
    (ury * scale, -lly * scale)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbol_font_uses_its_own_encoding() {
        // alpha is code 97 ('a') in Symbol, 631 units wide; minus 549.
        let w = |c| glyph_metric_inches(c, FontFace::Symbol, 72.0).2 * 1000.0;
        assert!((w('a') - 631.0).abs() < 1e-9 && (w('-') - 549.0).abs() < 1e-9);
        assert_eq!(symbol_to_unicode('a'), '\u{3B1}');
        assert_eq!(symbol_to_unicode('\u{B1}'), '\u{B1}');
        assert_eq!(symbol_to_unicode('-'), '\u{2212}');
    }

    #[test]
    fn m_height_is_718_units() {
        let h = m_height_inches(FontFace::Plain, 72.0, 1.0);
        assert!((h - 0.718).abs() < 1e-9);
    }

    #[test]
    fn helvetica_space_is_278_units() {
        let m = metrics_table(FontFace::Plain);
        assert_eq!(m.char_width_units(' '), 278.0);
    }

    #[test]
    fn width_scales_with_pointsize() {
        let w12 = str_width_inches("Hello", FontFace::Plain, 12.0, 1.0);
        let w24 = str_width_inches("Hello", FontFace::Plain, 24.0, 1.0);
        assert!((w24 - 2.0 * w12).abs() < 1e-12);
    }

    #[test]
    fn kerning_applied_for_known_pair() {
        // "AV" kerns by -70 units in Helvetica.
        let m = metrics_table(FontFace::Plain);
        let unkerned = m.char_width_units('A') + m.char_width_units('V');
        let kerned = m.str_width_units("AV");
        assert_eq!(unkerned - kerned, 70.0);
    }

    #[test]
    fn kerning_skipped_for_space() {
        let m = metrics_table(FontFace::Plain);
        let unkerned = m.char_width_units(' ') + m.char_width_units('W');
        let kerned = m.str_width_units(" W");
        assert_eq!(unkerned, kerned);
    }
}
