// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Multi-figure arrangements: `par(mfrow=)` / `par(mfcol=)` grids and
//! `layout()`.
//!
//! The region arithmetic is a direct port of the "Code for layouts"
//! section of `graphics.c` (`layoutRegions()` and friends, written by Paul
//! Murrell for R's `layout()`), plus the `mfrow`/`mfcol` branch of
//! `mapFigureRegion()`. Everything here works in NIC ("normalized inner
//! region") coordinates, i.e. fractions of the device minus its outer
//! margins; [`crate::device::Device`] turns them into inches.

use std::fmt;

/// R's `MAX_LAYOUT_ROWS` / `MAX_LAYOUT_COLS` / `MAX_LAYOUT_CELLS`.
pub const MAX_LAYOUT_ROWS: usize = 50;
pub const MAX_LAYOUT_COLS: usize = 50;
pub const MAX_LAYOUT_CELLS: usize = 500;

/// A column width or row height for [`Layout`]: relative to the other
/// relative sizes (R's plain numbers) or absolute (R's `lcm(x)`).
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum LayoutSize {
    Relative(f64),
    Cm(f64),
}

impl LayoutSize {
    fn value(self) -> f64 {
        match self {
            LayoutSize::Relative(v) | LayoutSize::Cm(v) => v,
        }
    }

    fn is_cm(self) -> bool {
        matches!(self, LayoutSize::Cm(_))
    }
}

/// Why a [`Layout`] could not be built.
#[derive(Clone, Debug, PartialEq)]
pub struct LayoutError(pub String);

impl fmt::Display for LayoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for LayoutError {}

/// `respect` for `layout()`: R's `rspct` 0 / 1 / 2.
#[derive(Clone, Debug, PartialEq)]
enum Respect {
    No,
    All,
    /// Column-major, like `Layout::order`.
    Matrix(Vec<bool>),
}

/// A `layout()` specification: which figure number occupies each cell of
/// a grid, plus relative/absolute column widths and row heights and the
/// `respect` option (equal x and y scales).
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    nrow: usize,
    ncol: usize,
    /// `mat[i, j] == order[i + j * nrow]` (R's storage).
    order: Vec<usize>,
    num_figures: usize,
    widths: Vec<f64>,
    heights: Vec<f64>,
    cm_widths: Vec<bool>,
    cm_heights: Vec<bool>,
    respect: Respect,
}

impl Layout {
    /// `layout(mat)`; `mat` is given as rows, top to bottom (the way the
    /// matrix reads on screen). Every figure number `1..=max(mat)` must
    /// appear; `0` marks an empty cell. Widths and heights default to
    /// equal relative sizes.
    pub fn new(mat: &[Vec<usize>]) -> Result<Layout, LayoutError> {
        let nrow = mat.len();
        let ncol = mat.first().map_or(0, Vec::len);
        if nrow == 0 || ncol == 0 || mat.iter().any(|r| r.len() != ncol) {
            return Err(LayoutError(
                "layout matrix must be non-empty and rectangular".into(),
            ));
        }
        if nrow > MAX_LAYOUT_ROWS {
            return Err(LayoutError(format!(
                "too many rows in layout, limit {MAX_LAYOUT_ROWS}"
            )));
        }
        if ncol > MAX_LAYOUT_COLS {
            return Err(LayoutError(format!(
                "too many columns in layout, limit {MAX_LAYOUT_COLS}"
            )));
        }
        if nrow * ncol > MAX_LAYOUT_CELLS {
            return Err(LayoutError(format!(
                "too many cells in layout, limit {MAX_LAYOUT_CELLS}"
            )));
        }
        let num_figures = mat.iter().flatten().copied().max().unwrap_or(0);
        if num_figures == 0 {
            return Err(LayoutError(
                "layout matrix must contain at least one figure".into(),
            ));
        }
        for fig in 1..=num_figures {
            if !mat.iter().flatten().any(|&v| v == fig) {
                return Err(LayoutError(format!(
                    "layout matrix must contain at least one reference\n\
                     to each of the values {{1 ... {num_figures}}}"
                )));
            }
        }
        let mut order = vec![0; nrow * ncol];
        for (i, row) in mat.iter().enumerate() {
            for (j, &v) in row.iter().enumerate() {
                order[i + j * nrow] = v;
            }
        }
        Ok(Layout {
            nrow,
            ncol,
            order,
            num_figures,
            widths: vec![1.0; ncol],
            heights: vec![1.0; nrow],
            cm_widths: vec![false; ncol],
            cm_heights: vec![false; nrow],
            respect: Respect::No,
        })
    }

    /// Column widths (`widths =`); padded with relative `1`s if shorter
    /// than the number of columns, extra entries are ignored.
    pub fn widths(mut self, widths: &[LayoutSize]) -> Layout {
        let (w, cm) = pad_sizes(widths, self.ncol);
        self.widths = w;
        self.cm_widths = cm;
        self
    }

    /// Row heights (`heights =`); see [`Layout::widths`].
    pub fn heights(mut self, heights: &[LayoutSize]) -> Layout {
        let (h, cm) = pad_sizes(heights, self.nrow);
        self.heights = h;
        self.cm_heights = cm;
        self
    }

    /// `respect = TRUE/FALSE`: one unit of width equals one unit of
    /// height in every relative-size cell.
    pub fn respect(mut self, respect: bool) -> Layout {
        self.respect = if respect { Respect::All } else { Respect::No };
        self
    }

    /// `respect = <matrix>`: respect only the marked cells (given as rows,
    /// like `mat`).
    pub fn respect_matrix(mut self, respect: &[Vec<bool>]) -> Result<Layout, LayoutError> {
        if respect.len() != self.nrow || respect.iter().any(|r| r.len() != self.ncol) {
            return Err(LayoutError(
                "'respect' must be logical or matrix with same dimension as 'mat'".into(),
            ));
        }
        let mut m = vec![false; self.nrow * self.ncol];
        for (i, row) in respect.iter().enumerate() {
            for (j, &v) in row.iter().enumerate() {
                m[i + j * self.nrow] = v;
            }
        }
        self.respect = Respect::Matrix(m);
        Ok(self)
    }

    pub fn num_figures(&self) -> usize {
        self.num_figures
    }

    pub fn nrow(&self) -> usize {
        self.nrow
    }

    pub fn ncol(&self) -> usize {
        self.ncol
    }

    // ---- port of graphics.c "Code for layouts" ----------------------

    fn respected(&self, i: usize, j: usize) -> bool {
        match &self.respect {
            Respect::Matrix(m) => m[i + j * self.nrow],
            _ => false,
        }
    }

    fn sum_widths(&self) -> f64 {
        sum_flag(&self.widths, &self.cm_widths, false)
    }

    fn sum_cm_widths(&self) -> f64 {
        sum_flag(&self.widths, &self.cm_widths, true)
    }

    fn sum_heights(&self) -> f64 {
        sum_flag(&self.heights, &self.cm_heights, false)
    }

    fn sum_cm_heights(&self) -> f64 {
        sum_flag(&self.heights, &self.cm_heights, true)
    }

    fn tall_layout(&self, cm_width: f64, cm_height: f64) -> bool {
        (cm_height / self.sum_heights()) > (cm_width / self.sum_widths())
    }

    /// `layoutRegion()`: `(width, height)` multipliers for the relative
    /// regions so the layout keeps its aspect ratio.
    fn layout_region(&self, w: &[f64], h: &[f64], cm_width: f64, cm_height: f64) -> (f64, f64) {
        let layout_ar = sum_flag(h, &self.cm_heights, false) / sum_flag(w, &self.cm_widths, false);
        let inner_ar = cm_height / cm_width;
        if layout_ar < inner_ar {
            (1.0, layout_ar / inner_ar)
        } else {
            (inner_ar / layout_ar, 1.0)
        }
    }

    fn all_cm_regions(&self, w: &mut [f64], h: &mut [f64], cm_width: f64, cm_height: f64) {
        alloc_dimension(w, cm_width, &self.cm_widths, true);
        alloc_dimension(h, cm_height, &self.cm_heights, true);
    }

    fn modify_regions(&self, w: &mut [f64], h: &mut [f64], col_mult: f64, row_mult: f64) {
        modify_dimension(w, col_mult, &self.cm_widths);
        modify_dimension(h, row_mult, &self.cm_heights);
    }

    fn regions_without_respect(&self, w: &mut [f64], h: &mut [f64]) {
        let sw = sum_flag(w, &self.cm_widths, false);
        let sh = sum_flag(h, &self.cm_heights, false);
        alloc_dimension(w, sw, &self.cm_widths, false);
        alloc_dimension(h, sh, &self.cm_heights, false);
    }

    fn regions_with_respect(&self, w: &mut [f64], h: &mut [f64], cm_width: f64, cm_height: f64) {
        let (cm, rm) = self.layout_region(w, h, cm_width, cm_height);
        self.regions_without_respect(w, h);
        self.modify_regions(w, h, cm, rm);
    }

    fn widths_respecting_heights(&self, w: &mut [f64], cm_width: f64, cm_height: f64) {
        let mut respected_cols = vec![false; self.ncol];
        w.copy_from_slice(&self.widths);
        for i in 0..self.nrow {
            for j in 0..self.ncol {
                if self.respected(i, j) && !self.cm_widths[j] {
                    respected_cols[j] = true;
                }
            }
        }
        let disrespected: f64 = (0..self.ncol)
            .filter(|&j| !respected_cols[j])
            .map(|j| self.widths[j])
            .sum();
        let width_left =
            self.sum_heights() * cm_width / cm_height - self.sum_widths() + disrespected;
        for j in 0..self.ncol {
            if !respected_cols[j] {
                w[j] = width_left * w[j] / disrespected;
            }
        }
    }

    fn heights_respecting_widths(&self, h: &mut [f64], cm_width: f64, cm_height: f64) {
        let mut respected_rows = vec![false; self.nrow];
        h.copy_from_slice(&self.heights);
        for i in 0..self.nrow {
            for j in 0..self.ncol {
                if self.respected(i, j) && !self.cm_heights[i] {
                    respected_rows[i] = true;
                }
            }
        }
        let disrespected: f64 = (0..self.nrow)
            .filter(|&i| !respected_rows[i])
            .map(|i| self.heights[i])
            .sum();
        let height_left =
            self.sum_widths() * cm_height / cm_width - self.sum_heights() + disrespected;
        for i in 0..self.nrow {
            if !respected_rows[i] {
                h[i] = height_left * h[i] / disrespected;
            }
        }
    }

    fn no_cm_regions(&self, w: &mut [f64], h: &mut [f64], cm_width: f64, cm_height: f64) {
        match self.respect {
            Respect::No => self.regions_without_respect(w, h),
            Respect::All => self.regions_with_respect(w, h, cm_width, cm_height),
            Respect::Matrix(_) => {
                if self.tall_layout(cm_width, cm_height) {
                    self.heights_respecting_widths(h, cm_width, cm_height);
                } else {
                    self.widths_respecting_heights(w, cm_width, cm_height);
                }
                self.regions_with_respect(w, h, cm_width, cm_height);
            }
        }
    }

    fn not_all_cm_regions(&self, w: &mut [f64], h: &mut [f64], cm_width: f64, cm_height: f64) {
        let new_cm_width = cm_width - self.sum_cm_widths();
        let new_cm_height = cm_height - self.sum_cm_heights();
        self.no_cm_regions(w, h, new_cm_width, new_cm_height);
        alloc_dimension(w, cm_width, &self.cm_widths, true);
        alloc_dimension(h, cm_height, &self.cm_heights, true);
        modify_dimension(w, new_cm_width / cm_width, &self.cm_widths);
        modify_dimension(h, new_cm_height / cm_height, &self.cm_heights);
    }

    fn width_cm_regions(&self, w: &mut [f64], h: &mut [f64], cm_width: f64, cm_height: f64) {
        alloc_dimension(w, cm_width, &self.cm_widths, true);
        let sh = self.sum_heights();
        alloc_dimension(h, sh, &self.cm_heights, false);
        modify_dimension(
            h,
            (cm_height - self.sum_cm_heights()) / cm_height,
            &self.cm_heights,
        );
        alloc_dimension(h, cm_height, &self.cm_heights, true);
    }

    fn height_cm_regions(&self, w: &mut [f64], h: &mut [f64], cm_width: f64, cm_height: f64) {
        alloc_dimension(h, cm_height, &self.cm_heights, true);
        let sw = self.sum_widths();
        alloc_dimension(w, sw, &self.cm_widths, false);
        modify_dimension(
            w,
            (cm_width - self.sum_cm_widths()) / cm_width,
            &self.cm_widths,
        );
        alloc_dimension(w, cm_width, &self.cm_widths, true);
    }

    /// `layoutRegions()`: the width of every column and height of every
    /// row as a fraction of an inner region `cm_width` x `cm_height` cm.
    fn regions(&self, cm_width: f64, cm_height: f64) -> (Vec<f64>, Vec<f64>) {
        let mut w = self.widths.clone();
        let mut h = self.heights.clone();
        let all_cm_w = self.cm_widths.iter().all(|&c| c);
        let all_cm_h = self.cm_heights.iter().all(|&c| c);
        let no_cm = !self.cm_widths.iter().any(|&c| c) && !self.cm_heights.iter().any(|&c| c);
        if all_cm_w && all_cm_h {
            self.all_cm_regions(&mut w, &mut h, cm_width, cm_height);
        } else if no_cm {
            self.no_cm_regions(&mut w, &mut h, cm_width, cm_height);
        } else if all_cm_w {
            self.width_cm_regions(&mut w, &mut h, cm_width, cm_height);
        } else if all_cm_h {
            self.height_cm_regions(&mut w, &mut h, cm_width, cm_height);
        } else {
            self.not_all_cm_regions(&mut w, &mut h, cm_width, cm_height);
        }
        (w, h)
    }

    /// `figureExtent()`: `(min_col, max_col, min_row, max_row)` (0-based)
    /// of the cells holding figure `fig`.
    fn figure_extent(&self, fig: usize) -> Option<(usize, usize, usize, usize)> {
        let mut ext: Option<(usize, usize, usize, usize)> = None;
        for i in 0..self.nrow {
            for j in 0..self.ncol {
                if self.order[i + j * self.nrow] == fig {
                    ext = Some(match ext {
                        None => (j, j, i, i),
                        Some((c0, c1, r0, r1)) => (c0.min(j), c1.max(j), r0.min(i), r1.max(i)),
                    });
                }
            }
        }
        ext
    }

    /// Figure `fig`'s region `(left, right, bottom, top)` in NIC, given
    /// the inner region's size in inches.
    fn figure_region_nic(&self, fig: usize, inner_w_in: f64, inner_h_in: f64) -> [f64; 4] {
        let (w, h) = self.regions(inner_w_in * 2.54, inner_h_in * 2.54);
        let Some((minc, maxc, minr, maxr)) = self.figure_extent(fig) else {
            return [0.0, 1.0, 0.0, 1.0];
        };
        // `subRegion()`
        let total_w: f64 = w.iter().sum();
        let total_h: f64 = h.iter().sum();
        let sum_to = |v: &[f64], n: usize| -> f64 { v[..n].iter().sum() };
        let left = (0.5 - total_w / 2.0) + sum_to(&w, minc);
        let right = (0.5 - total_w / 2.0) + sum_to(&w, maxc + 1);
        let bottom = (0.5 - total_h / 2.0) + total_h - sum_to(&h, maxr + 1);
        let top = (0.5 - total_h / 2.0) + total_h - sum_to(&h, minr);
        [left, right, bottom, top]
    }
}

fn pad_sizes(sizes: &[LayoutSize], len: usize) -> (Vec<f64>, Vec<bool>) {
    let mut v = Vec::with_capacity(len);
    let mut cm = Vec::with_capacity(len);
    for k in 0..len {
        let s = sizes.get(k).copied().unwrap_or(LayoutSize::Relative(1.0));
        v.push(s.value());
        cm.push(s.is_cm());
    }
    (v, cm)
}

/// `sum()` in graphics.c: the sum of the cm (`cm = true`) or relative
/// entries.
fn sum_flag(values: &[f64], flags: &[bool], cm: bool) -> f64 {
    values
        .iter()
        .zip(flags)
        .filter(|&(_, &f)| f == cm)
        .map(|(v, _)| v)
        .sum()
}

/// `allocDimension()`.
fn alloc_dimension(dims: &mut [f64], sum: f64, flags: &[bool], cm: bool) {
    for (d, &f) in dims.iter_mut().zip(flags) {
        if f == cm {
            *d /= sum;
        }
    }
}

/// `modifyDimension()`: scales the *relative* entries.
fn modify_dimension(dims: &mut [f64], mult: f64, flags: &[bool]) {
    for (d, &f) in dims.iter_mut().zip(flags) {
        if !f {
            *d *= mult;
        }
    }
}

/// How figures are arranged on the device: the default single figure,
/// an `mfrow`/`mfcol` grid, or a `layout()`.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Arrangement {
    Grid {
        nrow: usize,
        ncol: usize,
        /// `mfcol` (fill down columns) rather than `mfrow`.
        by_col: bool,
    },
    Layout(Layout),
}

impl Arrangement {
    pub(crate) fn nrow(&self) -> usize {
        match self {
            Arrangement::Grid { nrow, .. } => *nrow,
            Arrangement::Layout(l) => l.nrow,
        }
    }

    pub(crate) fn ncol(&self) -> usize {
        match self {
            Arrangement::Grid { ncol, .. } => *ncol,
            Arrangement::Layout(l) => l.ncol,
        }
    }

    /// `lastFigure`.
    pub(crate) fn last_figure(&self) -> usize {
        match self {
            Arrangement::Grid { nrow, ncol, .. } => nrow * ncol,
            Arrangement::Layout(l) => l.num_figures,
        }
    }

    /// `mapFigureRegion()`: the current figure's `[x0, x1, y0, y1]` in
    /// NIC.
    pub(crate) fn figure_region_nic(
        &self,
        current: usize,
        inner_w_in: f64,
        inner_h_in: f64,
    ) -> [f64; 4] {
        match self {
            Arrangement::Layout(l) => l.figure_region_nic(current, inner_w_in, inner_h_in),
            Arrangement::Grid { nrow, ncol, by_col } => {
                let (row, col) = if *by_col {
                    let col = (current - 1) / nrow + 1;
                    (current - (col - 1) * nrow, col)
                } else {
                    let row = (current - 1) / ncol + 1;
                    (row, current - (row - 1) * ncol)
                };
                [
                    (col - 1) as f64 / *ncol as f64,
                    col as f64 / *ncol as f64,
                    (*nrow - row) as f64 / *nrow as f64,
                    (*nrow - row + 1) as f64 / *nrow as f64,
                ]
            }
        }
    }

    /// The `cexbase` R's `par(mfrow=)`/`layout()` imposes for this
    /// arrangement (`mex` is always 1).
    pub(crate) fn cex_base(&self) -> f64 {
        let (nrow, ncol) = (self.nrow(), self.ncol());
        if nrow > 2 || ncol > 2 {
            0.66
        } else if nrow == 2 && ncol == 2 {
            0.83
        } else {
            1.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-12
    }

    #[test]
    fn grid_mfrow_and_mfcol_order() {
        let g = Arrangement::Grid {
            nrow: 2,
            ncol: 3,
            by_col: false,
        };
        // Figure 2 is top row, middle column.
        let r = g.figure_region_nic(2, 6.0, 4.0);
        assert!(close(r[0], 1.0 / 3.0) && close(r[1], 2.0 / 3.0));
        assert!(close(r[2], 0.5) && close(r[3], 1.0));
        let g = Arrangement::Grid {
            nrow: 2,
            ncol: 3,
            by_col: true,
        };
        // mfcol: figure 2 is bottom row, first column.
        let r = g.figure_region_nic(2, 6.0, 4.0);
        assert!(close(r[0], 0.0) && close(r[1], 1.0 / 3.0));
        assert!(close(r[2], 0.0) && close(r[3], 0.5));
    }

    #[test]
    fn cex_base_follows_r() {
        let g = |nrow, ncol| Arrangement::Grid {
            nrow,
            ncol,
            by_col: false,
        };
        assert_eq!(g(1, 1).cex_base(), 1.0);
        assert_eq!(g(2, 1).cex_base(), 1.0);
        assert_eq!(g(2, 2).cex_base(), 0.83);
        assert_eq!(g(3, 1).cex_base(), 0.66);
        assert_eq!(g(1, 3).cex_base(), 0.66);
    }

    #[test]
    fn layout_spanning_figure_and_relative_sizes() {
        // layout(matrix(c(1, 1, 2, 3), 2, byrow = TRUE), heights = c(1, 3))
        let l = Layout::new(&[vec![1, 1], vec![2, 3]])
            .unwrap()
            .heights(&[LayoutSize::Relative(1.0), LayoutSize::Relative(3.0)]);
        let top = l.figure_region_nic(1, 6.0, 4.0);
        assert!(close(top[0], 0.0) && close(top[1], 1.0));
        assert!(close(top[2], 0.75) && close(top[3], 1.0));
        let bl = l.figure_region_nic(2, 6.0, 4.0);
        assert!(close(bl[1], 0.5) && close(bl[2], 0.0) && close(bl[3], 0.75));
    }

    #[test]
    fn layout_cm_width_is_absolute() {
        // A 2 cm column in a 6 in (15.24 cm) wide region.
        let l = Layout::new(&[vec![1, 2]])
            .unwrap()
            .widths(&[LayoutSize::Cm(2.0), LayoutSize::Relative(1.0)]);
        let r = l.figure_region_nic(1, 6.0, 4.0);
        assert!(close(r[1], 2.0 / (6.0 * 2.54)));
        let r2 = l.figure_region_nic(2, 6.0, 4.0);
        assert!(close(r2[1], 1.0));
    }

    #[test]
    fn layout_respect_keeps_aspect_ratio() {
        // 2 x 2 equal cells on a 6 x 4 in region, respect = TRUE: the
        // layout shrinks to a centred square-celled block.
        let l = Layout::new(&[vec![1, 2], vec![3, 4]])
            .unwrap()
            .respect(true);
        let r = l.figure_region_nic(1, 6.0, 4.0);
        let w_in = (r[1] - r[0]) * 6.0;
        let h_in = (r[3] - r[2]) * 4.0;
        assert!(close(w_in, h_in));
    }

    #[test]
    fn layout_validation() {
        assert!(Layout::new(&[vec![1, 3]]).is_err()); // 2 missing
        assert!(Layout::new(&[vec![1, 2], vec![1]]).is_err()); // ragged
        assert!(Layout::new(&[vec![0, 0]]).is_err());
        assert!(Layout::new(&[vec![1, 0]]).is_ok());
    }
}
