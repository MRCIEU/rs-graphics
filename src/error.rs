// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! The crate's error types: [`Error`], an R error from a drawing call
//! (returned by `.try_draw()`), and [`ParseValueError`], a string that
//! isn't a valid value (returned by the `FromStr` impls).

use std::fmt;

/// An R error from a drawing call: what R would stop with, such as a
/// missing argument, an invalid value, or regions that don't fit the page.
///
/// Returned by the builders' `try_draw()` (and [`Device`]'s
/// `try_plot_new()`, `try_add_figure()`, ...); the matching `draw()`
/// panics with the same text. It prints like R's own errors, the call's
/// name then R's message: `plot.new(): figure margins too large`.
///
/// This is R's `Error in plot.new() : figure margins too large`.
///
/// [`Device`]: crate::device::Device
///
/// # Examples
///
/// ```
/// # use rs_graphics::prelude::*;
/// use rs_graphics::error::ErrorKind;
///
/// let mut dev = Device::new(SvgBackend::new(0.5, 0.5), Par::default());
/// let err = dev.plot().x([1, 2, 3]).try_draw().unwrap_err();
/// assert_eq!(err.call(), "plot.new");
/// assert_eq!(err.kind(), ErrorKind::DoesNotFit);
/// assert_eq!(err.to_string(), "plot.new(): figure margins too large");
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error {
    call: &'static str,
    kind: ErrorKind,
    message: String,
}

/// What kind of R error an [`Error`] is.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ErrorKind {
    /// A required argument wasn't set (`argument "x" is missing, with no
    /// default`).
    MissingArgument,
    /// An argument's value is invalid, or the data can't be drawn as given
    /// (`'x' and 'y' lengths differ`, `'x' values must be positive`, ...).
    InvalidArgument,
    /// The regions don't fit the page: margins too large for the device or
    /// panel, or a figure or plot region larger than allowed. A bigger page,
    /// fewer panels or smaller margins fix it.
    DoesNotFit,
}

impl Error {
    /// An invalid argument or data, with R's message.
    pub(crate) fn invalid(message: impl Into<String>) -> Error {
        Error {
            call: "",
            kind: ErrorKind::InvalidArgument,
            message: message.into(),
        }
    }

    /// A missing required argument.
    pub(crate) fn missing(name: &str) -> Error {
        Error {
            call: "",
            kind: ErrorKind::MissingArgument,
            message: format!("argument \"{name}\" is missing, with no default"),
        }
    }

    /// Regions that don't fit the page, from `call`.
    pub(crate) fn does_not_fit(call: &'static str, message: impl Into<String>) -> Error {
        Error {
            call,
            kind: ErrorKind::DoesNotFit,
            message: message.into(),
        }
    }

    /// This error, attributed to `call` unless it already names one (an
    /// error from `plot.new()` inside `plot()` stays `plot.new()`'s, as in
    /// R).
    pub(crate) fn in_call(mut self, call: &'static str) -> Error {
        if self.call.is_empty() {
            self.call = call;
        }
        self
    }

    /// Returns the name of the R function that raised the error, such as
    /// `"plot.new"` or `"hist"`.
    pub fn call(&self) -> &str {
        self.call
    }

    /// Returns what kind of error it is.
    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    /// Returns R's message, without the call's name.
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.call.is_empty() {
            f.write_str(&self.message)
        } else {
            write!(f, "{}(): {}", self.call, self.message)
        }
    }
}

impl std::error::Error for Error {}

/// An error for a string that isn't a valid value of some parameter.
///
/// E.g. `"zz"` as a colour or `"x"` as an axis style (`par(xaxs)`).
/// Returned by the `FromStr` impls of
/// [`Color`](crate::color::Color), [`LineType`](crate::lty::LineType),
/// [`LineEnd`](crate::backend::LineEnd), [`LineJoin`](crate::backend::LineJoin),
/// [`PlotType`](crate::highlevel::PlotType),
/// [`LegendKeyword`](crate::device::LegendKeyword),
/// [`LegendBty`](crate::device::LegendBty),
/// [`OrderH`](crate::highlevel::OrderH), [`Bty`](crate::par::Bty),
/// [`FontFace`](crate::font::FontFace),
/// [`Format`](crate::backend::Format), [`Units`](crate::device::Units)
/// and the `par()` enums. It displays as `invalid <kind>: "<value>"`.
///
/// # Examples
///
/// ```
/// # use rs_graphics::prelude::*;
/// use rs_graphics::error::ParseValueError;
///
/// let err: ParseValueError = "wavy".parse::<LineType>().unwrap_err();
/// assert_eq!(err.kind, "line type");
/// assert_eq!(err.to_string(), "invalid line type: \"wavy\"");
/// ```
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

/// `impl From<&str>` (and `String`, `&String`) for `$ty` via its
/// `parse(&str) -> Option<$ty>`, so setters taking `impl Into<$ty>` accept
/// R's strings (`.order_h("decr")`). Panics on an invalid string, naming
/// `$kind` and the accepted values `$valid`, as R errors.
macro_rules! from_r_string {
    ($ty:ident, $kind:literal, $valid:literal) => {
        impl From<&str> for $ty {
            #[doc = concat!("Converts the string form of a [`", stringify!($ty), "`] (R's, \
                where R has one), via [`", stringify!($ty), "::parse`].")]
            ///
            /// # Panics
            ///
            #[doc = concat!("If `s` isn't one (as R errors); use [`",
                        stringify!($ty), "::parse`] or `str::parse` to handle that yourself.")]
            fn from(s: &str) -> Self {
                match <$ty>::parse(s) {
                    Some(v) => v,
                    None => panic!(concat!("{:?} is not a valid ", $kind, "; use ", $valid), s),
                }
            }
        }

        impl From<String> for $ty {
            /// Converts as `From<&str>` does, panicking on the same strings.
            fn from(s: String) -> Self {
                <$ty>::from(s.as_str())
            }
        }

        impl From<&String> for $ty {
            /// Converts as `From<&str>` does, panicking on the same strings.
            fn from(s: &String) -> Self {
                <$ty>::from(s.as_str())
            }
        }
    };
}
pub(crate) use from_r_string;
