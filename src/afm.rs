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
//! table (`KPstart`/`KPend` in `devPS.c`), which can cause it to silently
//! ignore one legitimate kerning pair per first-character in rare cases.
//! We deliberately do *not* replicate that bug here - we apply every
//! kerning pair the AFM file defines (other than the documented
//! space-exclusion above), which keeps layout essentially identical to R
//! while staying simple and correct.

use std::collections::HashMap;
use std::sync::OnceLock;

/// The four standard font faces R exposes via `par(font=)`/`par(font.axis=)`
/// etc.: `1` = plain, `2` = bold, `3` = italic, `4` = bold italic.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum FontFace {
    Plain,
    Bold,
    Italic,
    BoldItalic,
}

impl FontFace {
    pub fn from_r_font_code(code: i32) -> Self {
        match code {
            2 => FontFace::Bold,
            3 => FontFace::Italic,
            4 => FontFace::BoldItalic,
            _ => FontFace::Plain,
        }
    }

    fn afm_source(self) -> &'static str {
        match self {
            FontFace::Plain => include_str!("../assets/afm/Helvetica.afm"),
            FontFace::Bold => include_str!("../assets/afm/Helvetica-Bold.afm"),
            FontFace::Italic => include_str!("../assets/afm/Helvetica-Oblique.afm"),
            FontFace::BoldItalic => include_str!("../assets/afm/Helvetica-BoldOblique.afm"),
        }
    }
}

/// Character widths and kerning pairs (in 1/1000 em units, the AFM
/// convention) for a single font face, indexed by Latin-1 code point for
/// the printable ASCII range (where Adobe Standard Encoding and Latin-1
/// agree).
pub struct FontMetrics {
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
    fn parse(afm: &str) -> Self {
        let mut widths = [0u16; 256];

        let mut bbox_ury = [0i16; 256];
        let mut bbox_lly = [0i16; 256];
        let mut ascent_units = 718.0;
        let mut descent_units = -207.0;
        let mut name_to_code: HashMap<String, u8> = HashMap::new();
        let mut kerning: HashMap<(u8, u8), i16> = HashMap::new();

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
                if let (Some(code), Some(wx)) = (code, wx) {
                    if (0..256).contains(&code) {
                        widths[code as usize] = wx;
                        if let Some((lly, ury)) = bbox_val {
                            bbox_lly[code as usize] = lly;
                            bbox_ury[code as usize] = ury;
                        }
                        if let Some(name) = name {
                            name_to_code.insert(name.to_string(), code as u8);
                        }
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
                    if let (Some(&c1), Some(&c2), Ok(kern)) = (
                        name_to_code.get(n1),
                        name_to_code.get(n2),
                        kern.parse::<i16>(),
                    ) {
                        kerning.insert((c1, c2), kern);
                    }
                }
            }
        }
        // Adobe Standard Encoding and Latin-1 agree on ASCII (32-126); a
        // "space" width is a reasonable fallback for anything else we
        // haven't mapped (accented letters, etc.) rather than 0.
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
    let (cell, face) = match face {
        FontFace::Plain => (&PLAIN, face),
        FontFace::Bold => (&BOLD, face),
        FontFace::Italic => (&ITALIC, face),
        FontFace::BoldItalic => (&BOLD_ITALIC, face),
    };
    cell.get_or_init(|| FontMetrics::parse(face.afm_source()))
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

/// Nominal single-line string height in inches, following R's convention
/// of basing `strheight()` on the font's nominal size rather than a
/// particular string's ink extents.
pub fn str_height_inches(face: FontFace, pointsize: f64, cex: f64) -> f64 {
    let m = metrics_table(face);
    let pt = pointsize * cex;
    (m.ascent_units - m.descent_units) / 1000.0 * pt / 72.0
}

/// `(ascent, descent)` in inches - the distances the font's nominal
/// bounding box extends above and below the baseline, respectively (both
/// non-negative). Used to resolve vertical text justification
/// (`adj`/`pos`) consistently everywhere text is drawn.
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
