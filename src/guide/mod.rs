// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Guides: how to use the crate, beyond each item's docs.
//!
//! - [`concepts`]: R's plotting vocabulary (regions of a page, `cex`,
//!   `pch`, `par()`, ...), for those who don't know R.
//! - [`coming_from_r`]: writing R's graphics calls here, with complete
//!   scripts in both languages.
//! - [`r_reference`]: every R function and argument, with its Rust form.
//!
//! [`charts`](crate::charts) draws the built-in values (symbols, line
//! types, colours, ...) so they can be chosen by sight.
//!
//! These modules contain no code.

/// R's plotting vocabulary, for those who don't know R.
///
/// The regions of a page (drawn by [`charts::regions`](crate::charts::regions)):
///
#[doc = include_str!("../charts/regions.svg")]
///
#[doc = include_str!("concepts.md")]
pub mod concepts {}

/// Writing R's graphics calls with this crate.
///
#[doc = include_str!("coming_from_r.md")]
pub mod coming_from_r {}

/// Every R function and argument, with its Rust form.
///
/// One section per R function: the Rust call that starts it, then a row
/// per setter: the R argument it stands for (`(none)` where R has no such
/// argument), the setter, and what it does. Arguments R passes on through
/// `...` (graphical parameters such as `cex`, and `main`, `xlim`, ...)
/// count as the function's. R's arguments with no setter are listed after
/// each table, so a translation can stop rather than guess.
///
/// Generated from the code and R 4.6's argument lists; a test keeps it up
/// to date.
///
#[doc = include_str!("r_reference.md")]
pub mod r_reference {}
