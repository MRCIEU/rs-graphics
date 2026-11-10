// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! The named palettes of R's `palette()`: the colour sets of
//! `.palette_colors_hex` and the name matching of `palette.match()`, both
//! in `r-source/src/library/grDevices/R/colorstuff.R`, and `"default"` as
//! `palette()` in `src/library/grDevices/src/colors.c` takes it.
//!
//! The sets' sources, as R credits them: R's own defaults (`"R3"` before
//! R 4.0.0, `"R4"` since), `scales::hue_pal()` (`"ggplot2"`), Masataka
//! Okabe and Kei Ito (`"Okabe-Ito"`), ColorBrewer.org by Mark A. Harrower
//! and Cynthia A. Brewer (`"Accent"`, `"Dark 2"`, `"Paired"`, `"Pastel 1"`,
//! `"Pastel 2"`, `"Set 1"`, `"Set 2"`, `"Set 3"`), Tableau by Maureen Stone
//! and Cristy Miller (`"Tableau 10"`, `"Classic Tableau"`), and Polychrome
//! by Kevin R. Coombes, Guy Brock, Zachary B. Abrams and Lynne V. Abruzzo
//! (`"Polychrome 36"`, `"Alphabet"`).
//!
//! **R bug not copied:** a name that is a prefix of several sets (`"set"`,
//! `"r"`, `""`) makes `palette.match()` return 0, and R then fails with
//! "attempt to select less than one element in integerOneIndex". Here an
//! ambiguous name is an error that lists the sets it matches (user's
//! choice), and `""` is R's "unknown palette" error.

use super::{Color, DEFAULT_PALETTE};
use crate::error::Error;

/// R's `.palette_colors_hex`, in R's order (as `palette.pals()` lists it).
pub(crate) const PALETTES: [(&str, &[Color]); 16] = [
    (
        "R3",
        &[
            Color::rgb(0x00, 0x00, 0x00),
            Color::rgb(0xFF, 0x00, 0x00),
            Color::rgb(0x00, 0xCD, 0x00),
            Color::rgb(0x00, 0x00, 0xFF),
            Color::rgb(0x00, 0xFF, 0xFF),
            Color::rgb(0xFF, 0x00, 0xFF),
            Color::rgb(0xFF, 0xFF, 0x00),
            Color::rgb(0xBE, 0xBE, 0xBE),
        ],
    ),
    (
        "R4",
        &[
            Color::rgb(0x00, 0x00, 0x00),
            Color::rgb(0xDF, 0x53, 0x6B),
            Color::rgb(0x61, 0xD0, 0x4F),
            Color::rgb(0x22, 0x97, 0xE6),
            Color::rgb(0x28, 0xE2, 0xE5),
            Color::rgb(0xCD, 0x0B, 0xBC),
            Color::rgb(0xF5, 0xC7, 0x10),
            Color::rgb(0x9E, 0x9E, 0x9E),
        ],
    ),
    (
        "ggplot2",
        &[
            Color::rgb(0x00, 0x00, 0x00),
            Color::rgb(0xF8, 0x76, 0x6D),
            Color::rgb(0x00, 0xBA, 0x38),
            Color::rgb(0x61, 0x9C, 0xFF),
            Color::rgb(0x00, 0xBF, 0xC4),
            Color::rgb(0xF5, 0x64, 0xE3),
            Color::rgb(0xB7, 0x9F, 0x00),
            Color::rgb(0x9E, 0x9E, 0x9E),
        ],
    ),
    (
        "Okabe-Ito",
        &[
            Color::rgb(0x00, 0x00, 0x00),
            Color::rgb(0xE6, 0x9F, 0x00),
            Color::rgb(0x56, 0xB4, 0xE9),
            Color::rgb(0x00, 0x9E, 0x73),
            Color::rgb(0xF0, 0xE4, 0x42),
            Color::rgb(0x00, 0x72, 0xB2),
            Color::rgb(0xD5, 0x5E, 0x00),
            Color::rgb(0xCC, 0x79, 0xA7),
            Color::rgb(0x99, 0x99, 0x99),
        ],
    ),
    (
        "Accent",
        &[
            Color::rgb(0x7F, 0xC9, 0x7F),
            Color::rgb(0xBE, 0xAE, 0xD4),
            Color::rgb(0xFD, 0xC0, 0x86),
            Color::rgb(0xFF, 0xFF, 0x99),
            Color::rgb(0x38, 0x6C, 0xB0),
            Color::rgb(0xF0, 0x02, 0x7F),
            Color::rgb(0xBF, 0x5B, 0x17),
            Color::rgb(0x66, 0x66, 0x66),
        ],
    ),
    (
        "Dark 2",
        &[
            Color::rgb(0x1B, 0x9E, 0x77),
            Color::rgb(0xD9, 0x5F, 0x02),
            Color::rgb(0x75, 0x70, 0xB3),
            Color::rgb(0xE7, 0x29, 0x8A),
            Color::rgb(0x66, 0xA6, 0x1E),
            Color::rgb(0xE6, 0xAB, 0x02),
            Color::rgb(0xA6, 0x76, 0x1D),
            Color::rgb(0x66, 0x66, 0x66),
        ],
    ),
    (
        "Paired",
        &[
            Color::rgb(0xA6, 0xCE, 0xE3),
            Color::rgb(0x1F, 0x78, 0xB4),
            Color::rgb(0xB2, 0xDF, 0x8A),
            Color::rgb(0x33, 0xA0, 0x2C),
            Color::rgb(0xFB, 0x9A, 0x99),
            Color::rgb(0xE3, 0x1A, 0x1C),
            Color::rgb(0xFD, 0xBF, 0x6F),
            Color::rgb(0xFF, 0x7F, 0x00),
            Color::rgb(0xCA, 0xB2, 0xD6),
            Color::rgb(0x6A, 0x3D, 0x9A),
            Color::rgb(0xFF, 0xFF, 0x99),
            Color::rgb(0xB1, 0x59, 0x28),
        ],
    ),
    (
        "Pastel 1",
        &[
            Color::rgb(0xFB, 0xB4, 0xAE),
            Color::rgb(0xB3, 0xCD, 0xE3),
            Color::rgb(0xCC, 0xEB, 0xC5),
            Color::rgb(0xDE, 0xCB, 0xE4),
            Color::rgb(0xFE, 0xD9, 0xA6),
            Color::rgb(0xFF, 0xFF, 0xCC),
            Color::rgb(0xE5, 0xD8, 0xBD),
            Color::rgb(0xFD, 0xDA, 0xEC),
            Color::rgb(0xF2, 0xF2, 0xF2),
        ],
    ),
    (
        "Pastel 2",
        &[
            Color::rgb(0xB3, 0xE2, 0xCD),
            Color::rgb(0xFD, 0xCD, 0xAC),
            Color::rgb(0xCB, 0xD5, 0xE8),
            Color::rgb(0xF4, 0xCA, 0xE4),
            Color::rgb(0xE6, 0xF5, 0xC9),
            Color::rgb(0xFF, 0xF2, 0xAE),
            Color::rgb(0xF1, 0xE2, 0xCC),
            Color::rgb(0xCC, 0xCC, 0xCC),
        ],
    ),
    (
        "Set 1",
        &[
            Color::rgb(0xE4, 0x1A, 0x1C),
            Color::rgb(0x37, 0x7E, 0xB8),
            Color::rgb(0x4D, 0xAF, 0x4A),
            Color::rgb(0x98, 0x4E, 0xA3),
            Color::rgb(0xFF, 0x7F, 0x00),
            Color::rgb(0xFF, 0xFF, 0x33),
            Color::rgb(0xA6, 0x56, 0x28),
            Color::rgb(0xF7, 0x81, 0xBF),
            Color::rgb(0x99, 0x99, 0x99),
        ],
    ),
    (
        "Set 2",
        &[
            Color::rgb(0x66, 0xC2, 0xA5),
            Color::rgb(0xFC, 0x8D, 0x62),
            Color::rgb(0x8D, 0xA0, 0xCB),
            Color::rgb(0xE7, 0x8A, 0xC3),
            Color::rgb(0xA6, 0xD8, 0x54),
            Color::rgb(0xFF, 0xD9, 0x2F),
            Color::rgb(0xE5, 0xC4, 0x94),
            Color::rgb(0xB3, 0xB3, 0xB3),
        ],
    ),
    (
        "Set 3",
        &[
            Color::rgb(0x8D, 0xD3, 0xC7),
            Color::rgb(0xFF, 0xFF, 0xB3),
            Color::rgb(0xBE, 0xBA, 0xDA),
            Color::rgb(0xFB, 0x80, 0x72),
            Color::rgb(0x80, 0xB1, 0xD3),
            Color::rgb(0xFD, 0xB4, 0x62),
            Color::rgb(0xB3, 0xDE, 0x69),
            Color::rgb(0xFC, 0xCD, 0xE5),
            Color::rgb(0xD9, 0xD9, 0xD9),
            Color::rgb(0xBC, 0x80, 0xBD),
            Color::rgb(0xCC, 0xEB, 0xC5),
            Color::rgb(0xFF, 0xED, 0x6F),
        ],
    ),
    (
        "Tableau 10",
        &[
            Color::rgb(0x4E, 0x79, 0xA7),
            Color::rgb(0xF2, 0x8E, 0x2B),
            Color::rgb(0xE1, 0x57, 0x59),
            Color::rgb(0x76, 0xB7, 0xB2),
            Color::rgb(0x59, 0xA1, 0x4F),
            Color::rgb(0xED, 0xC9, 0x48),
            Color::rgb(0xB0, 0x7A, 0xA1),
            Color::rgb(0xFF, 0x9D, 0xA7),
            Color::rgb(0x9C, 0x75, 0x5F),
            Color::rgb(0xBA, 0xB0, 0xAC),
        ],
    ),
    (
        "Classic Tableau",
        &[
            Color::rgb(0x1F, 0x77, 0xB4),
            Color::rgb(0xFF, 0x7F, 0x0E),
            Color::rgb(0x2C, 0xA0, 0x2C),
            Color::rgb(0xD6, 0x27, 0x28),
            Color::rgb(0x94, 0x67, 0xBD),
            Color::rgb(0x8C, 0x56, 0x4B),
            Color::rgb(0xE3, 0x77, 0xC2),
            Color::rgb(0x7F, 0x7F, 0x7F),
            Color::rgb(0xBC, 0xBD, 0x22),
            Color::rgb(0x17, 0xBE, 0xCF),
        ],
    ),
    (
        "Polychrome 36",
        &[
            Color::rgb(0x5A, 0x51, 0x56),
            Color::rgb(0xE4, 0xE1, 0xE3),
            Color::rgb(0xF6, 0x22, 0x2E),
            Color::rgb(0xFE, 0x00, 0xFA),
            Color::rgb(0x16, 0xFF, 0x32),
            Color::rgb(0x32, 0x83, 0xFE),
            Color::rgb(0xFE, 0xAF, 0x16),
            Color::rgb(0xB0, 0x00, 0x68),
            Color::rgb(0x1C, 0xFF, 0xCE),
            Color::rgb(0x90, 0xAD, 0x1C),
            Color::rgb(0x2E, 0xD9, 0xFF),
            Color::rgb(0xDE, 0xA0, 0xFD),
            Color::rgb(0xAA, 0x0D, 0xFE),
            Color::rgb(0xF8, 0xA1, 0x9F),
            Color::rgb(0x32, 0x5A, 0x9B),
            Color::rgb(0xC4, 0x45, 0x1C),
            Color::rgb(0x1C, 0x83, 0x56),
            Color::rgb(0x85, 0x66, 0x0D),
            Color::rgb(0xB1, 0x0D, 0xA1),
            Color::rgb(0xFB, 0xE4, 0x26),
            Color::rgb(0x1C, 0xBE, 0x4F),
            Color::rgb(0xFA, 0x00, 0x87),
            Color::rgb(0xFC, 0x1C, 0xBF),
            Color::rgb(0xF7, 0xE1, 0xA0),
            Color::rgb(0xC0, 0x75, 0xA6),
            Color::rgb(0x78, 0x2A, 0xB6),
            Color::rgb(0xAA, 0xF4, 0x00),
            Color::rgb(0xBD, 0xCD, 0xFF),
            Color::rgb(0x82, 0x2E, 0x1C),
            Color::rgb(0xB5, 0xEF, 0xB5),
            Color::rgb(0x7E, 0xD7, 0xD1),
            Color::rgb(0x1C, 0x7F, 0x93),
            Color::rgb(0xD8, 0x5F, 0xF7),
            Color::rgb(0x68, 0x3B, 0x79),
            Color::rgb(0x66, 0xB0, 0xFF),
            Color::rgb(0x3B, 0x00, 0xFB),
        ],
    ),
    (
        "Alphabet",
        &[
            Color::rgb(0xAA, 0x0D, 0xFE),
            Color::rgb(0x32, 0x83, 0xFE),
            Color::rgb(0x85, 0x66, 0x0D),
            Color::rgb(0x78, 0x2A, 0xB6),
            Color::rgb(0x56, 0x56, 0x56),
            Color::rgb(0x1C, 0x83, 0x56),
            Color::rgb(0x16, 0xFF, 0x32),
            Color::rgb(0xF7, 0xE1, 0xA0),
            Color::rgb(0xE2, 0xE2, 0xE2),
            Color::rgb(0x1C, 0xBE, 0x4F),
            Color::rgb(0xC4, 0x45, 0x1C),
            Color::rgb(0xDE, 0xA0, 0xFD),
            Color::rgb(0xFE, 0x00, 0xFA),
            Color::rgb(0x32, 0x5A, 0x9B),
            Color::rgb(0xFE, 0xAF, 0x16),
            Color::rgb(0xF8, 0xA1, 0x9F),
            Color::rgb(0x90, 0xAD, 0x1C),
            Color::rgb(0xF6, 0x22, 0x2E),
            Color::rgb(0x1C, 0xFF, 0xCE),
            Color::rgb(0x2E, 0xD9, 0xFF),
            Color::rgb(0xB1, 0x0D, 0xA1),
            Color::rgb(0xC0, 0x75, 0xA6),
            Color::rgb(0xFC, 0x1C, 0xBF),
            Color::rgb(0xB0, 0x00, 0x68),
            Color::rgb(0xFB, 0xE4, 0x26),
            Color::rgb(0xFA, 0x00, 0x87),
        ],
    ),
];

/// `palette.match()`'s normalisation: lower case, without `-`, `,`, ` `,
/// `_`, `(`, `)` and `.`.
fn normalise(name: &str) -> String {
    name.chars()
        .filter(|c| !"-, _().".contains(*c))
        .flat_map(char::to_lowercase)
        .collect()
}

/// The colours R's `palette(name)` sets for one string: a set from
/// [`PALETTES`] by its name, ignoring case and punctuation, or a unique
/// prefix of it (`palette.match()`'s `charmatch()`), else `"default"`
/// ignoring case and spaces (`StrMatch()`).
///
/// # Errors
///
/// R's "unknown palette (need >= 2 colors)" for any other string, and an
/// error listing the sets for a prefix of several (see the module doc).
pub(crate) fn named_palette(name: &str) -> Result<Vec<Color>, Error> {
    if name == "default" {
        return Ok(DEFAULT_PALETTE.to_vec());
    }
    let wanted = normalise(name);
    if !wanted.is_empty() {
        if let Some((_, colors)) = PALETTES.iter().find(|(n, _)| normalise(n) == wanted) {
            return Ok(colors.to_vec());
        }
        let prefixed: Vec<&(&str, &[Color])> = PALETTES
            .iter()
            .filter(|(n, _)| normalise(n).starts_with(&wanted))
            .collect();
        match prefixed.as_slice() {
            [(_, colors)] => return Ok(colors.to_vec()),
            [] => {}
            several => {
                let names: Vec<String> = several.iter().map(|(n, _)| format!("\"{n}\"")).collect();
                return Err(Error::invalid(format!(
                    "ambiguous palette name '{name}': it could be {}",
                    names.join(", ")
                )));
            }
        }
    }
    let spaceless: String = name.chars().filter(|c| *c != ' ').collect();
    if spaceless.eq_ignore_ascii_case("default") {
        return Ok(DEFAULT_PALETTE.to_vec());
    }
    Err(Error::invalid("unknown palette (need >= 2 colors)"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn first(name: &str) -> Result<Color, String> {
        named_palette(name)
            .map(|p| p[0])
            .map_err(|e| e.message().to_string())
    }

    #[test]
    fn names_match_as_r_palette_match() {
        let okabe = Ok(Color::rgb(0x00, 0x00, 0x00));
        // Case and punctuation ignored, and a unique prefix.
        for name in [
            "Okabe-Ito",
            "OKABE-ITO",
            "okabe ito",
            "okabe.ito",
            "(okabe)",
            "okabe",
        ] {
            assert_eq!(first(name), okabe, "{name}");
            assert_eq!(named_palette(name).unwrap().len(), 9, "{name}");
        }
        assert_eq!(first("set 3"), Ok(Color::rgb(0x8D, 0xD3, 0xC7)));
        assert_eq!(first("classic"), Ok(Color::rgb(0x1F, 0x77, 0xB4)));
        // "default" in any case or spacing is R4.
        for name in ["default", "Default", " DEFAULT", "R4"] {
            assert_eq!(
                named_palette(name).unwrap(),
                DEFAULT_PALETTE.to_vec(),
                "{name}"
            );
        }
        assert_eq!(
            first("set"),
            Err("ambiguous palette name 'set': it could be \"Set 1\", \"Set 2\", \"Set 3\"".into())
        );
        for name in ["", "red", "#FF0000", "nonsense"] {
            assert_eq!(
                first(name),
                Err("unknown palette (need >= 2 colors)".into()),
                "{name}"
            );
        }
    }
}
