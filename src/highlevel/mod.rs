// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! R's high-level plots, `plot()`, `hist()`, `barplot()`, `boxplot()` and
//! `pie()`, as methods of [`crate::device::Device`] ([`Device::plot`](crate::graphics::Graphics::plot),
//! ...), with their options and result types.
//!
//! Each function here is a fairly direct port of the corresponding
//! `*.default` R function in `r-source/src/library/graphics/R/`, reduced to
//! the common/core use cases: there are no formula interfaces or
//! S3-style dispatch on input type, and each module's docs (and the
//! "Out of scope" section of DEVELOPER.md) say what else isn't implemented.
//!
//! Each starts a new plot (moving to the next panel of a multi-panel
//! page). They use only `Device`'s public API, like plots written outside
//! the crate (see the crate docs).

pub mod barplot;
pub mod boxplot;
pub mod hist;
pub mod pie;
pub mod plot;

pub use barplot::{BarHeights, BarplotOptions, OrderH};
pub use boxplot::{BoxStats, BoxplotOptions, boxplot_stats};
pub use hist::{HistOptions, HistResult};
pub use pie::PieOptions;
pub use plot::{PlotOptions, PlotType};

/// Compute the finite min/max of a data slice, matching R's `range(x[is.finite(x)])`
/// idiom used throughout the plotting defaults. Falls back to `(0.0, 1.0)` for
/// empty/all-non-finite input. A single repeated value gives the degenerate
/// `(v, v)`, as R's `range()` does; `Graphics::plot_window` (`GScale()`) widens
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
