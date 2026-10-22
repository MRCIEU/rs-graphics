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
//! page), unless `add` is set (`hist`, `barplot`, `boxplot`). They are
//! built from the same drawing calls as the public API (`plot_new`,
//! `rect`, `axis`, `title`, ...), as plots written outside the crate are
//! (see the crate docs).

pub mod barplot;
pub mod boxplot;
pub mod hist;
pub mod pie;
pub mod plot;

use crate::font::FontFace;

pub use barplot::{BarHeights, BarplotOptions, OrderH};
pub use boxplot::{BoxStats, BoxplotOptions, boxplot_stats};
pub use hist::{HistBreaks, HistLabels, HistOptions, HistResult};
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
    if lo > hi { (0.0, 1.0) } else { (lo, hi) }
}

/// R's inline axis parameters that a high-level plot passes through its
/// `...` to `axis()` (`las = 1`, `cex.axis = 0.8`, ...); `None` is
/// `axis()`'s default (the `par()` value, except `lwd`: 1). Each plot copies its own fields in, and only those R
/// lets through (e.g. `bxp()` drops `mgp`).
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct InlineAxis {
    pub las: Option<i32>,
    pub cex_axis: Option<f64>,
    pub col_axis: Option<crate::color::ColorSpec>,
    pub font_axis: Option<FontFace>,
    pub mgp: Option<[f64; 3]>,
    pub tck: Option<f64>,
    pub tcl: Option<f64>,
    pub gap_axis: Option<f64>,
    /// `lwd`: the axis line and ticks (`axis(lwd = )`).
    pub lwd: Option<f64>,
    /// `font`: the label font, over `font_axis` (`axis(font = )`).
    pub font: Option<FontFace>,
    /// `fg`: the axis line and ticks' colour (`axis()` takes `fg` as
    /// `col`).
    pub col: Option<crate::color::ColorSpec>,
    /// `lend`: the axis line and ticks' ends (`axis(lend = )`).
    pub lend: Option<crate::backend::LineEnd>,
    /// `xpd`: how far ticks and labels may reach (`axis(xpd = )`).
    pub xpd: Option<crate::par::Xpd>,
}

impl InlineAxis {
    /// `opts` with these parameters filled in where `opts` leaves them unset.
    pub(crate) fn apply(&self, opts: crate::device::AxisOptions) -> crate::device::AxisOptions {
        crate::device::AxisOptions {
            las: opts.las.or(self.las),
            cex_axis: opts.cex_axis.or(self.cex_axis),
            col_axis: opts.col_axis.or(self.col_axis),
            font: opts.font.or(self.font).or(self.font_axis),
            lwd: opts.lwd.or(self.lwd),
            col: opts.col.or(self.col),
            lend: opts.lend.or(self.lend),
            xpd: opts.xpd.or(self.xpd),
            mgp: opts.mgp.or(self.mgp),
            tck: opts.tck.or(self.tck),
            tcl: opts.tcl.or(self.tcl),
            gap_axis: opts.gap_axis.or(self.gap_axis),
            ..opts
        }
    }
}
