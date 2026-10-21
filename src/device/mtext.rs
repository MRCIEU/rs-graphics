// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! `mtext()`: text in the figure or outer margins, a port of `C_mtext`
//! (`plot.c`, incl. `ComputeAdjValue`/`ComputePAdjValue`/`ComputeAtValue`)
//! and `GMtext()` (`graphics.c`).
//!
//! Positions follow `GConvert()`'s `MAR1..4` / `OMA1..4` units: along the
//! side, `at` is a user coordinate (inner margins) or a fraction of the
//! inner region (outer margins); across it, `line` counts margin lines out
//! from the plot region (inner) or from the inner region (outer). Text is
//! clipped to the figure region (inner), or the device (outer), as R
//! forces `xpd` to 1 or `NA`.

use crate::backend::Backend;
use crate::color::ColorSpec;
use crate::geometry::Point;

use crate::plotmath::Label;

use super::{Device, Side, Xpd};

/// The arguments of R's `mtext()` other than `text` and `side`, with
/// R's defaults. As in R, the text and the vector arguments are recycled
/// over the longest: `mtext(["a", "b"], 3).at([1, 2])` writes two labels.
/// Empty: R's default for every label; a `NaN` entry is R's `NA`, the
/// default for that label.
#[derive(Clone, Debug, Default)]
pub struct MtextOptions {
    /// Margin line, starting at 0 next to the plot region (or the inner
    /// region for `outer`); may be negative or fractional. Empty: 0 (R's
    /// default). A `NaN` entry (R's `NA`) leaves its label out; R writes
    /// it with a `nan` coordinate, an invalid PDF (an R bug, not copied).
    pub line: Vec<f64>,
    /// Draw in the outer margins (`par(oma)`) rather than the figure's.
    /// Default `false`.
    pub outer: bool,
    /// Position along the side: a user coordinate for inner margins, or a
    /// fraction (0 to 1) of the inner region for outer ones. Empty or
    /// `NaN`: from `adj` (text parallel to the side) or centred
    /// (perpendicular).
    pub at: Vec<f64>,
    /// Justification along the text; empty or `NaN`: 0.5 for text
    /// parallel to the side, otherwise justified at the end nearest the
    /// plot (1 on sides 1 and 2, 0 on sides 3 and 4).
    pub adj: Vec<f64>,
    /// Justification across the text; empty or `NaN`: depends on `side`
    /// and `las` (0, i.e. baseline on the line, for `las = 0`).
    pub padj: Vec<f64>,
    /// Empty or `NaN`: 1. Unlike most text, *not* scaled by `par(cex)` (so
    /// neither by the multi-panel reduction), as in R. (For an `NA` after a
    /// finite value, R keeps the previous label's size, as `C_mtext`
    /// forgets to reset it; that bug isn't copied.)
    pub cex: Vec<f64>,
    /// Empty: `par(col)`.
    pub col: Vec<ColorSpec>,
    /// Empty: `par(font)`.
    pub font: Vec<i32>,
    /// Text orientation (R's inline `las`, through `mtext()`'s `...`): 0
    /// parallel to the side, 1 horizontal, 2 perpendicular to the side, 3
    /// vertical. It also sets the default `adj`, `padj` and `at`, as in R.
    /// `None`: `par(las)`.
    pub las: Option<i32>,
}

crate::setters::setters!(MtextOptions => [MtextCall] {
    line: values f64,
    outer: value bool,
    at: values f64,
    adj: values f64,
    padj: values f64,
    cex: values f64,
    col: values ColorSpec,
    font: values i32,
    las: option i32,
});

/// One label's arguments, after recycling (`None`: R's default).
struct One {
    line: Option<f64>,
    outer: bool,
    at: Option<f64>,
    adj: Option<f64>,
    padj: Option<f64>,
    cex: Option<f64>,
    col: Option<ColorSpec>,
    font: Option<i32>,
    las: Option<i32>,
}

/// `mtext()`'s text: one label (`&str`, `String`, [`Label`], an
/// [`Expression`](crate::plotmath::Expression)) or several (a slice, array
/// or `Vec` of them), recycled with the other arguments.
pub trait MtextLabels {
    /// The labels, in order.
    fn into_labels(self) -> Vec<Label>;
}

macro_rules! one_label {
    ($($t:ty),*) => {
        $(
            impl MtextLabels for $t {
                fn into_labels(self) -> Vec<Label> {
                    vec![self.into()]
                }
            }
        )*
    };
}
one_label!(
    &str,
    &&str,
    String,
    &String,
    Label,
    crate::plotmath::Expression
);

impl<L: Clone + Into<Label>> MtextLabels for &[L] {
    fn into_labels(self) -> Vec<Label> {
        self.iter().cloned().map(Into::into).collect()
    }
}

impl<L: Clone + Into<Label>, const N: usize> MtextLabels for [L; N] {
    fn into_labels(self) -> Vec<Label> {
        self.into_iter().map(Into::into).collect()
    }
}

impl<L: Clone + Into<Label>, const N: usize> MtextLabels for &[L; N] {
    fn into_labels(self) -> Vec<Label> {
        self.iter().cloned().map(Into::into).collect()
    }
}

impl<L: Clone + Into<Label>> MtextLabels for Vec<L> {
    fn into_labels(self) -> Vec<Label> {
        self.into_iter().map(Into::into).collect()
    }
}

impl<L: Clone + Into<Label>> MtextLabels for &Vec<L> {
    fn into_labels(self) -> Vec<Label> {
        self.iter().cloned().map(Into::into).collect()
    }
}

/// `ComputeAdjValue()`.
fn default_adj(side: Side, las: i32) -> f64 {
    use Side::*;
    match (las, side) {
        (1, Left) | (2, Bottom) | (2, Left) | (3, Bottom) => 1.0,
        (1, Right) | (2, Top) | (2, Right) | (3, Top) => 0.0,
        _ => 0.5,
    }
}

/// `ComputePAdjValue()`.
fn default_padj(side: Side, las: i32) -> f64 {
    use Side::*;
    match (las, side) {
        (1, Left) | (1, Right) | (2, _) | (3, Bottom) | (3, Top) => 0.5,
        _ => 0.0,
    }
}

/// `ComputeAtValue()` for `at = NA`, as a fraction along the side: `adj`
/// when the text runs parallel to the side, otherwise the middle.
fn default_at_fraction(adj: f64, side: Side, las: i32) -> f64 {
    let horizontal_side = side.is_horizontal();
    let parallel = match las {
        1 => horizontal_side,
        2 => false,
        3 => !horizontal_side,
        _ => true,
    };
    if parallel { adj } else { 0.5 }
}

impl<B: Backend> Device<B> {
    /// Draws R's `mtext()`; the public entry is
    /// [`Graphics::mtext`](crate::graphics::Graphics::mtext), whose docs
    /// describe it.
    pub(crate) fn mtext_impl(&mut self, texts: &[Label], side: Side, opts: &MtextOptions) {
        // `C_mtext`: everything recycled over the longest argument.
        assert!(!texts.is_empty(), "zero-length 'text' specified");
        let n = [
            texts.len(),
            opts.line.len(),
            opts.at.len(),
            opts.adj.len(),
            opts.padj.len(),
            opts.cex.len(),
            opts.col.len(),
            opts.font.len(),
        ]
        .into_iter()
        .max()
        .unwrap_or(1);
        let pick = |v: &[f64], i: usize| {
            (!v.is_empty())
                .then(|| v[i % v.len()])
                .filter(|x| !x.is_nan())
        };
        for i in 0..n {
            // An `NA` line puts the label nowhere: R draws nothing for it.
            let line = (!opts.line.is_empty()).then(|| opts.line[i % opts.line.len()]);
            if line.is_some_and(f64::is_nan) {
                continue;
            }
            let one = One {
                line,
                outer: opts.outer,
                at: pick(&opts.at, i),
                adj: pick(&opts.adj, i),
                padj: pick(&opts.padj, i),
                cex: pick(&opts.cex, i),
                col: (!opts.col.is_empty()).then(|| opts.col[i % opts.col.len()]),
                font: (!opts.font.is_empty()).then(|| opts.font[i % opts.font.len()]),
                las: opts.las,
            };
            self.mtext_one(texts[i % texts.len()].clone(), side, &one);
        }
    }

    /// One label of `mtext()`, as `C_mtext`'s loop body.
    fn mtext_one(&mut self, text: Label, side: Side, opts: &One) {
        let las = opts.las.unwrap_or(self.par.las);
        let adj = opts.adj.unwrap_or_else(|| default_adj(side, las));
        let padj = opts.padj.unwrap_or_else(|| default_padj(side, las));
        let horizontal_side = side.is_horizontal();

        // Position along the side, in device inches.
        let along = if opts.outer {
            let f = opts
                .at
                .unwrap_or_else(|| default_at_fraction(adj, side, las));
            if horizontal_side {
                self.inner.x0 + f * self.inner.width()
            } else {
                self.inner.y0 + f * self.inner.height()
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
        let rotation = if parallel_text == horizontal_side {
            0.0
        } else {
            90.0
        };
        let line = opts.line.unwrap_or(0.0) + self.line_bias(side, parallel_text);

        // Position across the side.
        let lh = self.line_height_inches();
        let region = if opts.outer { self.inner } else { self.plt };
        let across = match side {
            Side::Bottom => region.y0 - line * lh,
            Side::Left => region.x0 - line * lh,
            Side::Top => region.y1 + line * lh,
            Side::Right => region.x1 + line * lh,
        };

        let anchor = if horizontal_side {
            Point::new(along, across)
        } else {
            Point::new(across, along)
        };
        let clip = if opts.outer { Xpd::Device } else { Xpd::Figure };
        let col = opts.col.unwrap_or(ColorSpec::Color(self.par.col));
        let font = opts.font.unwrap_or(self.par.font);
        // R sets the text size from `cex` alone (`C_mtext`: "we ignore any
        // shrinking produced by mfrow / mfcol specs"), whereas `text()`
        // multiplies by `par(cex)`.
        let cex = opts.cex.unwrap_or(1.0) / self.par.cex;
        // Text in the outer margins doesn't count as drawing on the plot,
        // so leaves `par(new)` as it was.
        let new = self.hold_figure;
        self.draw_label(
            clip,
            anchor,
            &text,
            adj,
            Some(padj),
            rotation,
            col,
            cex,
            font,
        );
        if opts.outer {
            self.hold_figure = new;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_follow_r() {
        // las = 0: parallel to every side, centred, baseline on the line.
        for side in Side::ALL {
            assert_eq!(default_adj(side, 0), 0.5);
            assert_eq!(default_padj(side, 0), 0.0);
            assert_eq!(default_at_fraction(0.2, side, 0), 0.2);
        }
        // las = 1 (horizontal): right-justified on side 2, left on side 4,
        // vertically centred and positioned mid-side there.
        assert_eq!(default_adj(Side::Left, 1), 1.0);
        assert_eq!(default_adj(Side::Right, 1), 0.0);
        assert_eq!(default_padj(Side::Left, 1), 0.5);
        assert_eq!(default_at_fraction(0.2, Side::Left, 1), 0.5);
        assert_eq!(default_at_fraction(0.2, Side::Bottom, 1), 0.2);
    }
}
