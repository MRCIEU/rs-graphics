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
//! plots' options and results, the [`Backend`] trait, [`AnyBackend`] and
//! backend option types (the built-in backends themselves are in
//! [`backend`](crate::backend)), and [`ErrorKind`] (the
//! error type itself is [`rs_graphics::Error`](crate::Error), kept out of
//! the glob import so it doesn't clash with other `Error`s).
//!
//! The traits that say what a setter accepts ([`Numbers`](crate::numbers::Numbers),
//! [`Pair`](crate::numbers::Pair), [`Labels`](crate::plotmath::Labels),
//! [`Pch`](crate::setters::Pch), [`Values`](crate::setters::Values), ...)
//! aren't here: setters work without them. Import one from its module
//! (`use rs_graphics::numbers::Numbers;`) to name it in your own functions
//! or call its methods.

pub use crate::backend::{AnyBackend, Backend, Format, LineEnd, LineJoin};
pub use crate::color::{Color, ColorSpec, gray_colors};
pub use crate::device::{
    AblineOptions, ArrowsOptions, AxisOptions, BoxOptions, BoxWhich, Device, DeviceBuilder,
    LegendKeyword, LegendLayout, LegendOptions, LegendPos, MtextOptions, PlotWindowOptions,
    PolygonOptions, RectOptions, SegmentsOptions, Side, StrUnits, TextOptions, TitleOptions, Units,
    XyOptions,
};
pub use crate::error::ErrorKind;
pub use crate::font::FontFace;
pub use crate::geometry::{Point, Rect};
pub use crate::graphics::Graphics;
pub use crate::highlevel::{
    BarHeights, BarplotOptions, BoxStats, BoxplotOptions, HistBreaks, HistLabels, HistOptions,
    HistResult, OrderH, PieOptions, PlotOptions, PlotType, boxplot_stats,
};
pub use crate::layout::{LayoutRespect, LayoutSize};
pub use crate::lty::LineType;
pub use crate::par::{AxisStyle, AxisType, Bty, Par, ParOptions, Pty, Sides, Xpd};
pub use crate::plotmath::{Expression, Label};
pub use crate::recording::{Figure, FigureResult};
