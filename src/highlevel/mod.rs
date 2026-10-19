// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! High-level plotting functions built on top of [`crate::device::Device`],
//! mirroring R's `plot()`, `hist()`, `barplot()`, `boxplot()`, and `pie()`.
//!
//! Each function here is a fairly direct port of the corresponding
//! `*.default` R function in `r-source/src/library/graphics/R/`, reduced to
//! the common/core use cases: there are no formula interfaces or
//! S3-style dispatch on input type, and each module's docs (and the
//! README's "Out of scope" section) say what else isn't implemented.
//!
//! They use only `Device`'s public API, so they are free functions taking
//! `&mut Device`, called the same way as plots written outside the crate
//! (see the crate docs for why the low-level functions are methods).

pub mod barplot;
pub mod boxplot;
pub mod hist;
pub mod pie;
pub mod plot;

pub use barplot::{barplot, barplot_matrix, gray_colors, BarplotOptions};
pub use boxplot::{boxplot, boxplot_stats, BoxStats, BoxplotOptions};
pub use hist::{hist, HistOptions, HistResult};
pub use pie::{pie, PieOptions};
pub use plot::{plot, PlotOptions, PlotType};

/// Compute the finite min/max of a data slice, matching R's `range(x[is.finite(x)])`
/// idiom used throughout the plotting defaults. Falls back to `(0.0, 1.0)` for
/// empty/all-non-finite input. A single repeated value gives the degenerate
/// `(v, v)`, as R's `range()` does; `Device::plot_window` (`GScale()`) widens
/// it, so no padding is done here.
pub(crate) fn data_range(xs: &[f64]) -> (f64, f64) {
    let mut lo = f64::INFINITY;
    let mut hi = f64::NEG_INFINITY;
    for &v in xs {
        if v.is_finite() {
            lo = lo.min(v);
            hi = hi.max(v);
        }
    }
    if lo > hi {
        (0.0, 1.0)
    } else {
        (lo, hi)
    }
}
