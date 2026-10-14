//! `mtext()`: text in the figure or outer margins, a port of `C_mtext`
//! (`plot.c`, incl. `ComputeAdjValue`/`ComputePAdjValue`/`ComputeAtValue`)
//! and `GMtext()` (`graphics.c`).
//!
//! Positions follow `GConvert()`'s `MAR1..4` / `OMA1..4` units: along the
//! side, `at` is a user coordinate (inner margins) or a fraction of the
//! inner region (outer margins); across it, `line` counts margin lines out
//! from the plot region (inner) or in from the edge of the outer margin
//! (outer). Text is clipped to the figure region (inner), or the device
//! (outer), as R forces `xpd` to 1 or `NA`.

use crate::backend::Backend;
use crate::color::ColorSpec;
use crate::geometry::Point;

use super::{Clip, Device, Side, Y_LINE_BIAS};

/// The arguments of R's `mtext()` other than `text`, with R's defaults.
/// `None` is R's `NA`.
#[derive(Clone, Debug)]
pub struct MtextOptions {
    /// `1` = bottom, `2` = left, `3` = top (the default), `4` = right.
    pub side: Side,
    /// Margin line, starting at 0 next to the plot region (or the inner
    /// region for `outer`); may be negative or fractional.
    pub line: f64,
    /// Draw in the outer margins (`par(oma)`) rather than the figure's.
    pub outer: bool,
    /// Position along the side: a user coordinate for inner margins, or a
    /// fraction (0 to 1) of the inner region for outer ones. `None`: from
    /// `adj` (text parallel to the side) or centred (perpendicular).
    pub at: Option<f64>,
    /// Justification along the text; `None`: 0.5 for text parallel to the
    /// side, otherwise right-justified towards the plot.
    pub adj: Option<f64>,
    /// Justification across the text; `None`: depends on `side`/`par(las)`
    /// (0, i.e. baseline on the line, for `las = 0`).
    pub padj: Option<f64>,
    /// `None`: 1. Unlike most text, *not* scaled by `par(cex)` (so neither
    /// by the multi-panel reduction), as in R.
    pub cex: Option<f64>,
    /// `None`: `par(col)`.
    pub col: Option<ColorSpec>,
    /// `None`: `par(font)`.
    pub font: Option<i32>,
}

impl Default for MtextOptions {
    fn default() -> Self {
        MtextOptions {
            side: 3,
            line: 0.0,
            outer: false,
            at: None,
            adj: None,
            padj: None,
            cex: None,
            col: None,
            font: None,
        }
    }
}

/// `ComputeAdjValue()`.
fn default_adj(side: Side, las: i32) -> f64 {
    match (las, side) {
        (1, 2) | (2, 1) | (2, 2) | (3, 1) => 1.0,
        (1, 4) | (2, 3) | (2, 4) | (3, 3) => 0.0,
        _ => 0.5,
    }
}

/// `ComputePAdjValue()`.
fn default_padj(side: Side, las: i32) -> f64 {
    match (las, side) {
        (1, 2) | (1, 4) | (2, _) | (3, 1) | (3, 3) => 0.5,
        _ => 0.0,
    }
}

/// `ComputeAtValue()` for `at = NA`, as a fraction along the side: `adj`
/// when the text runs parallel to the side, otherwise the middle.
fn default_at_fraction(adj: f64, side: Side, las: i32) -> f64 {
    let horizontal_side = side == 1 || side == 3;
    let parallel = match las {
        1 => horizontal_side,
        2 => false,
        3 => !horizontal_side,
        _ => true,
    };
    if parallel {
        adj
    } else {
        0.5
    }
}

impl<B: Backend> Device<B> {
    /// `mtext(text, side, line, outer, at, adj, padj, cex, col, font)`, with
    /// rotation from `par(las)`; see [`MtextOptions`].
    pub fn mtext(&mut self, text: &str, opts: &MtextOptions) {
        let side = opts.side;
        let las = self.par.las;
        let adj = opts.adj.unwrap_or_else(|| default_adj(side, las));
        let padj = opts.padj.unwrap_or_else(|| default_padj(side, las));
        let horizontal_side = side == 1 || side == 3;

        // Position along the side, in device inches.
        let along = if opts.outer {
            let omi = self.par.omi();
            let f = opts.at.unwrap_or_else(|| default_at_fraction(adj, side, las));
            if horizontal_side {
                omi.left + f * (self.width_in - omi.left - omi.right)
            } else {
                omi.bottom + f * (self.height_in - omi.bottom - omi.top)
            }
        } else {
            match opts.at {
                Some(at) => {
                    let (ux0, _, uy0, _) = self.usr;
                    let x0 = if self.xlog { 10f64.powf(ux0) } else { ux0 };
                    let y0 = if self.ylog { 10f64.powf(uy0) } else { uy0 };
                    if horizontal_side {
                        self.user_to_inches(at, y0).x
                    } else {
                        self.user_to_inches(x0, at).y
                    }
                }
                None => {
                    let f = default_at_fraction(adj, side, las);
                    if horizontal_side {
                        self.plt.x0 + f * self.plt.width()
                    } else {
                        self.plt.y0 + f * self.plt.height()
                    }
                }
            }
        };

        // `GMtext()`: rotation, and the `yLineBias` nudge for text parallel
        // to the side.
        let parallel_text = if horizontal_side {
            !(las == 2 || las == 3)
        } else {
            !(las == 1 || las == 2)
        };
        let rotation = if parallel_text == horizontal_side { 0.0 } else { 90.0 };
        let bias = match side {
            1 | 4 => 1.0 - Y_LINE_BIAS,
            _ => Y_LINE_BIAS,
        };
        let line = if parallel_text {
            opts.line + bias / self.par.mex
        } else {
            opts.line
        };

        // Position across the side.
        let lh = self.line_height();
        let across = if opts.outer {
            let oma = self.par.oma;
            match side {
                1 => (oma.bottom - line) * lh,
                2 => (oma.left - line) * lh,
                3 => self.height_in - (oma.top - line) * lh,
                _ => self.width_in - (oma.right - line) * lh,
            }
        } else {
            match side {
                1 => self.plt.y0 - line * lh,
                2 => self.plt.x0 - line * lh,
                3 => self.plt.y1 + line * lh,
                _ => self.plt.x1 + line * lh,
            }
        };

        let anchor = if horizontal_side {
            Point::new(along, across)
        } else {
            Point::new(across, along)
        };
        let clip = if opts.outer { Clip::Device } else { Clip::Figure };
        let col = opts.col.unwrap_or(ColorSpec::Color(self.par.col));
        let font = opts.font.unwrap_or(self.par.font);
        // R sets the text size from `cex` alone (`C_mtext`: "we ignore any
        // shrinking produced by mfrow / mfcol specs"), whereas `text_at`
        // multiplies by `par(cex)`.
        let cex = opts.cex.unwrap_or(1.0) / self.par.cex;
        self.text_at_clipped(clip, anchor, text, adj, Some(padj), rotation, col, cex, font);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_follow_r() {
        // las = 0: parallel to every side, centred, baseline on the line.
        for side in 1..=4 {
            assert_eq!(default_adj(side, 0), 0.5);
            assert_eq!(default_padj(side, 0), 0.0);
            assert_eq!(default_at_fraction(0.2, side, 0), 0.2);
        }
        // las = 1 (horizontal): right-justified on side 2, left on side 4,
        // vertically centred and positioned mid-side there.
        assert_eq!(default_adj(2, 1), 1.0);
        assert_eq!(default_adj(4, 1), 0.0);
        assert_eq!(default_padj(2, 1), 0.5);
        assert_eq!(default_at_fraction(0.2, 2, 1), 0.5);
        assert_eq!(default_at_fraction(0.2, 1, 1), 0.2);
    }
}
