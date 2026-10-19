// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! The error type of the crate's `FromStr` impls.

use std::fmt;

/// A string that isn't a valid value of some parameter, e.g. `"zz"` as a
/// colour or `"x"` as `par(xaxs)`. Returned by the `FromStr` impls of
/// [`Color`](crate::color::Color), [`LineType`](crate::lty::LineType),
/// [`LineEnd`](crate::backend::LineEnd), [`LineJoin`](crate::backend::LineJoin),
/// [`PlotType`](crate::highlevel::PlotType),
/// [`LegendKeyword`](crate::device::LegendKeyword),
/// [`Format`](crate::backend::Format) and the `par()` enums, and by
/// [`AnyBackend::for_path`](crate::backend::AnyBackend::for_path).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseValueError {
    /// What was being parsed, e.g. `"colour"` or `"axis style"`.
    pub kind: &'static str,
    /// The rejected input.
    pub value: String,
}

impl ParseValueError {
    pub(crate) fn new(kind: &'static str, value: &str) -> Self {
        ParseValueError {
            kind,
            value: value.to_string(),
        }
    }
}

impl fmt::Display for ParseValueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid {}: {:?}", self.kind, self.value)
    }
}

impl std::error::Error for ParseValueError {}

/// `impl FromStr for $ty` via its `parse(&str) -> Option<$ty>`.
macro_rules! from_str_via_parse {
    ($ty:ty, $kind:literal) => {
        impl std::str::FromStr for $ty {
            type Err = $crate::error::ParseValueError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                <$ty>::parse(s).ok_or_else(|| $crate::error::ParseValueError::new($kind, s))
            }
        }
    };
}
pub(crate) use from_str_via_parse;
