// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! The crate's error type, [`Error`]: an R error from a drawing call
//! (returned by `.try_draw()`), or a string that isn't a valid value
//! (returned by the string-valued types' `FromStr` and `TryFrom`).

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
/// use rs_graphics::prelude::*; // includes `ErrorKind`
///
/// let mut dev = Device::memory("svg").width(0.5).height(0.5).open().unwrap();
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

/// For conversions that can't fail (`TryFrom` from a `From` impl), so
/// setters can take either kind ([`IntoArg`](crate::setters::IntoArg)).
impl From<std::convert::Infallible> for Error {
    fn from(e: std::convert::Infallible) -> Error {
        match e {}
    }
}

/// An integer that doesn't fit the setter's type (`.code(300)` for a
/// `u8`), so setters for small integers take any integer literal.
impl From<std::num::TryFromIntError> for Error {
    fn from(e: std::num::TryFromIntError) -> Error {
        Error::invalid(e.to_string())
    }
}

/// The string forms of `$ty`, from its crate-private
/// `parse(&str) -> Option<$ty>`: `TryFrom<&str>` (and `String`, `&String`),
/// so setters taking an [`IntoArg<$ty>`](crate::setters::IntoArg) accept
/// R's strings (`.order_h("decr")`), and `FromStr` (`"decr".parse()`). An
/// invalid string is an [`Error`] naming `$kind` and the accepted values
/// `$valid`, as R errors. This is every string-valued type's one route
/// from a string.
macro_rules! from_r_string {
    ($ty:ident, $kind:literal, $valid:literal) => {
        impl std::str::FromStr for $ty {
            type Err = $crate::error::Error;

            /// Converts as `TryFrom<&str>` does.
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                <$ty>::try_from(s)
            }
        }

        impl TryFrom<&str> for $ty {
            type Error = $crate::error::Error;

            #[doc = concat!("Converts the string form of a [`", stringify!($ty), "`] (R's, \
                where R has one).")]
            ///
            /// # Errors
            ///
            /// If `s` isn't one (as R errors), naming the accepted values.
            fn try_from(s: &str) -> Result<Self, Self::Error> {
                <$ty>::parse(s).ok_or_else(|| {
                    $crate::error::Error::invalid(format!(
                        concat!("{:?} is not a valid ", $kind, "; use ", $valid),
                        s
                    ))
                })
            }
        }

        impl TryFrom<String> for $ty {
            type Error = $crate::error::Error;

            /// Converts as `TryFrom<&str>` does.
            fn try_from(s: String) -> Result<Self, Self::Error> {
                <$ty>::try_from(s.as_str())
            }
        }

        impl TryFrom<&String> for $ty {
            type Error = $crate::error::Error;

            /// Converts as `TryFrom<&str>` does.
            fn try_from(s: &String) -> Result<Self, Self::Error> {
                <$ty>::try_from(s.as_str())
            }
        }
    };
}
pub(crate) use from_r_string;

/// `impl FromStr for $ty` through its `TryFrom<&str>`, for types whose
/// string conversion is written by hand (colours, legend positions).
macro_rules! from_str_via_try_from {
    ($ty:ident) => {
        impl std::str::FromStr for $ty {
            type Err = $crate::error::Error;

            /// Converts as `TryFrom<&str>` does.
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                <$ty>::try_from(s)
            }
        }
    };
}
pub(crate) use from_str_via_try_from;

#[cfg(test)]
mod string_route_tests {
    use crate::backend::{Format, LineEnd, LineJoin};
    use crate::color::{Color, ColorSpec};
    use crate::device::{LegendBty, LegendKeyword, LegendPos, StrUnits, Units};
    use crate::font::FontFace;
    use crate::highlevel::{HistBreaks, OrderH, PlotType};
    use crate::lty::LineType;
    use crate::par::{AxisStyle, AxisType, Bty, Pty};
    use std::fmt::Debug;
    use std::str::FromStr;

    /// `FromStr` and `TryFrom<&str>` agree, for a good and a bad string.
    fn same<T>(good: &str, bad: &str)
    where
        T: FromStr<Err = super::Error> + for<'a> TryFrom<&'a str, Error = super::Error> + Debug,
    {
        let (a, b) = (good.parse::<T>(), T::try_from(good));
        assert!(a.is_ok() && b.is_ok(), "{good:?}");
        assert_eq!(format!("{a:?}"), format!("{b:?}"));
        let (a, b) = (bad.parse::<T>(), T::try_from(bad));
        assert_eq!(a.unwrap_err(), b.unwrap_err());
    }

    #[test]
    fn every_string_valued_type_has_one_route() {
        same::<AxisStyle>("i", "x");
        same::<Pty>("s", "q");
        same::<Bty>("l", "x");
        same::<AxisType>("n", "y");
        same::<FontFace>("bold", "heavy");
        same::<Color>("red", "notacolour");
        same::<ColorSpec>("2", "notacolour");
        same::<StrUnits>("in", "cm");
        same::<LineType>("dashed", "wavy");
        same::<PlotType>("b", "x");
        same::<HistBreaks>("FD", "F");
        same::<OrderH>("decr", "up");
        same::<LineEnd>("butt", "flat");
        same::<LineJoin>("mitre", "spiky");
        same::<Format>("png", "jpeg");
        same::<LegendKeyword>("topr", "to");
        same::<LegendPos>("topleft", "middle");
        same::<LegendBty>("n", "x");
        same::<Units>("cm", "pt");
    }
}
