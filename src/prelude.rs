// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Everything needed for everyday plotting, in one import:
//!
//! ```
//! use rs_graphics::prelude::*;
//! ```
//!
//! The device and its options types, `par()`, colours, line types,
//! labels, layouts, the high-level plots, and the backends.

pub use crate::backend::pdf::PdfBackend;
pub use crate::backend::raster::RasterBackend;
pub use crate::backend::svg::SvgBackend;
pub use crate::backend::{AnyBackend, Backend, Format, LineEnd, LineJoin};
pub use crate::color::{Color, ColorSpec};
pub use crate::device::{
    AblineOptions, ArrowsOptions, AxisOptions, BoxOptions, BoxWhich, Device, LegendEntry,
    LegendKeyword, LegendOptions, LegendPos, MtextOptions, PlotWindowOptions, PolygonOptions,
    RectOptions, SegmentsOptions, TextOptions, TitleOptions, XyOptions,
};
pub use crate::highlevel::{
    BarplotOptions, BoxStats, BoxplotOptions, HistOptions, HistResult, PieOptions, PlotOptions,
    PlotType, barplot, barplot_matrix, boxplot, boxplot_stats, gray_colors, hist, pie, plot,
};
pub use crate::layout::{Layout, LayoutSize};
pub use crate::lty::LineType;
pub use crate::par::{AxisStyle, AxisType, Par, Pty, Sides};
pub use crate::plotmath::{Expression, Label};
