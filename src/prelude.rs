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
//! The device, the `Graphics` trait (R's graphics functions) and their
//! options types, recorded `Figure`s, `par()`, points and rectangles in
//! device inches, colours, line types, labels, layouts, the high-level
//! plots' options and results, and the backends.

pub use crate::backend::pdf::PdfBackend;
pub use crate::backend::raster::RasterBackend;
pub use crate::backend::svg::SvgBackend;
pub use crate::backend::{AnyBackend, Backend, Format, LineEnd, LineJoin};
pub use crate::color::{Color, ColorSpec, gray_colors};
pub use crate::device::{
    AblineOptions, ArrowsOptions, AxisOptions, BoxOptions, BoxWhich, Device, DeviceBuilder,
    LegendBty, LegendEntry, LegendKeyword, LegendLayout, LegendOptions, LegendPos, MtextLabels,
    MtextOptions, PlotWindowOptions, PolygonOptions, RectOptions, SegmentsOptions, Side,
    TextOptions, TitleOptions, XyOptions,
};
pub use crate::geometry::{Point, Rect};
pub use crate::graphics::Graphics;
pub use crate::highlevel::{
    BarHeights, BarplotOptions, BoxStats, BoxplotOptions, HistBreaks, HistOptions, HistResult,
    OrderH, PieOptions, PlotOptions, PlotType, boxplot_stats,
};
pub use crate::layout::{Layout, LayoutSize};
pub use crate::lty::LineType;
pub use crate::numbers::{Groups, Numbers, Pair};
pub use crate::par::{AxisStyle, AxisType, Bty, Par, ParOptions, Pty, Sides, Xpd};
pub use crate::plotmath::{Expression, Label};
pub use crate::recording::{Figure, FigureResult};
