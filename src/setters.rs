// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Chainable setters for the options structs, and the traits ([`Values`],
//! [`NaValues`]) that let them take one value or several.
//!
//! The fields are private: setters are the one way to fill an options
//! struct, so every value goes through the setter's conversion and checks.
//! A struct is built once and reused with `.with(&opts)`:
//!
//! ```
//! # use rs_graphics::prelude::*;
//! # let mut dev = Device::null().open().unwrap();
//! # dev.plot_new();
//! let opts = XyOptions::default().pch(19).col(["red", "blue"]).cex(1.5);
//! dev.points().x([0.2, 0.8]).y([0.5, 0.5]).with(&opts).draw();
//! ```
//!
//! Each setter is named after R's argument and documents it. Optional
//! settings take the value itself (`.las(1)`, `.main("Title")`) or an
//! `Option` of it (see [`OptionArg`]); settings R recycles per element take one
//! value or several (see [`Values`]), and in style vectors (symbols, line
//! types and widths, sizes, fonts, colours) an element may be `None`,
//! R's `NA`: `.pch([Some(19), None])`, `.col([Some("red"), None])` (see
//! [`NaValues`]); limits take two numbers
//! (`.xlim(0.0, 10.0)`). As in R, numbers may be integers or floats
//! (`.cex(2)`, `.xlim(0, 10)`, `.mar([5, 4, 4, 2])`). Settings holding one of R's string-valued choices
//! also take R's string (`.lty("dashed")`, `.type_("b")`, `.bty("n")`,
//! `.order_h("decr")`).
//!
//! A value that can't be converted (`.lty("wavy")`, `.side(5)`,
//! `.col("notacolour")`) doesn't panic: the setter keeps the error, and
//! the call's `try_draw()` returns it (`.draw()` panics with it), as R
//! errors when the function runs. On an options struct used with
//! `.with(&opts)`, the error goes with it.
//!
//! ```
//! # use rs_graphics::prelude::*;
//! # let mut dev = Device::null().open().unwrap();
//! # dev.plot().x([1, 2, 3]).draw();
//! let e = dev.lines().x([1, 2]).y([1, 2]).lty("wavy").try_draw().unwrap_err();
//! assert_eq!(e.call(), "lines");
//! ```

use crate::color::{Color, ColorSpec};
use crate::error::Error;
use crate::font::FontFace;
use crate::lty::LineType;

/// A value a setter can take for a `T`: anything that converts to it,
/// possibly failing (`"dashed"` for a [`LineType`], `5` for a
/// [`Side`](crate::device::Side)).
///
/// Implemented for every type with `TryInto<T>` (so also `Into<T>`) whose
/// error is an [`Error`]. A failed conversion doesn't panic in the
/// setter: the error is kept and returned by the call's `try_draw()`.
///
/// # Examples
///
/// ```
/// use rs_graphics::setters::IntoArg;
/// # use rs_graphics::prelude::*;
/// let lty: LineType = "dashed".into_arg().unwrap();
/// assert_eq!(lty, LineType::DASHED);
/// assert!(IntoArg::<LineType>::into_arg("wavy").is_err());
/// ```
pub trait IntoArg<T> {
    /// The value, or why it isn't one.
    ///
    /// # Errors
    ///
    /// If the conversion fails, with R's message where R has one.
    fn into_arg(self) -> Result<T, Error>;
}

impl<T, U> IntoArg<T> for U
where
    U: TryInto<T>,
    Error: From<U::Error>,
{
    fn into_arg(self) -> Result<T, Error> {
        self.try_into().map_err(Error::from)
    }
}

/// Marker types that let a setter take a value or an `Option` of one
/// ([`OptionArg`], [`OptionNum`]); they are inferred, never written.
pub mod markers {
    /// A value (the setting is set to it).
    pub struct Value;
    /// An `Option` of a value (`None` leaves the setting unset).
    pub struct OptionOf;
}

/// A value, or an `Option` of one, for a setting R may leave unset.
///
/// Takes anything that converts to the setting's type ([`IntoArg`]), or an
/// `Option` of it: `None` leaves the setting at its default (for
/// [`par()`](crate::graphics::Graphics::par), unchanged), so an
/// `Option` held in a variable can be passed as it is. A bare `None`
/// needs its type (`None::<&str>`). The marker `M` is inferred.
///
/// # Examples
///
/// ```
/// # use rs_graphics::prelude::*;
/// # let mut dev = Device::null().open().unwrap();
/// let title: Option<&str> = None; // no title
/// dev.plot().x([1, 2, 3]).main(title).xlab("x").draw();
/// ```
pub trait OptionArg<T, M> {
    /// The value, `None`, or why it isn't one.
    ///
    /// # Errors
    ///
    /// If the conversion fails; see [`IntoArg`].
    fn into_option_arg(self) -> Result<Option<T>, Error>;
}

impl<T, U: IntoArg<T>> OptionArg<T, markers::Value> for U {
    fn into_option_arg(self) -> Result<Option<T>, Error> {
        self.into_arg().map(Some)
    }
}

impl<T, U: IntoArg<T>> OptionArg<T, markers::OptionOf> for Option<U> {
    fn into_option_arg(self) -> Result<Option<T>, Error> {
        self.map(IntoArg::into_arg).transpose()
    }
}

/// A number of any type ([`Num`](crate::numbers::Num)), or an `Option` of
/// one (`None`: the setting's default), for a numeric setting R may leave
/// unset. The marker `M` is inferred.
///
/// # Examples
///
/// ```
/// # use rs_graphics::prelude::*;
/// # let mut dev = Device::null().open().unwrap();
/// # dev.plot_new();
/// let tck: Option<f64> = Some(0.02);
/// dev.axis().side(1).tck(tck).line(2).draw();
/// ```
pub trait OptionNum<M> {
    /// The number as an `f64`, or `None`.
    fn into_option_num(self) -> Option<f64>;
}

impl<N: crate::numbers::Num> OptionNum<markers::Value> for N {
    fn into_option_num(self) -> Option<f64> {
        Some(self.to_f64())
    }
}

impl<N: crate::numbers::Num> OptionNum<markers::OptionOf> for Option<N> {
    fn into_option_num(self) -> Option<f64> {
        self.map(crate::numbers::Num::to_f64)
    }
}

/// Numbers ([`Numbers`](crate::numbers::Numbers)), or an `Option` of
/// them (`None`: the setting's default), for a numeric vector R may leave
/// unset. The marker `M` is inferred.
///
/// # Examples
///
/// ```
/// # use rs_graphics::prelude::*;
/// let at: Option<Vec<f64>> = Some(vec![1.0, 2.0]);
/// let mut dev = Device::null().open().unwrap();
/// dev.plot_new();
/// let ticks = dev.axis().side(1).at(at).draw();
/// assert_eq!(ticks, [1.0, 2.0]);
/// ```
pub trait OptionNumbers<'a, M> {
    /// The numbers as `f64`s, or `None`.
    fn into_option_numbers(self) -> Option<Vec<f64>>;
}

impl<'a, N: crate::numbers::Numbers<'a>> OptionNumbers<'a, markers::Value> for N {
    fn into_option_numbers(self) -> Option<Vec<f64>> {
        Some(self.into_numbers().into_owned())
    }
}

impl<'a, N: crate::numbers::Numbers<'a>> OptionNumbers<'a, markers::OptionOf> for Option<N> {
    fn into_option_numbers(self) -> Option<Vec<f64>> {
        self.map(|n| n.into_numbers().into_owned())
    }
}

/// One value or several, for a setting R recycles per element.
///
/// Takes a single value or a `Vec`, array or slice of them, by value or
/// by reference: `"red"`, `["red", "blue"]`, `[1, 2]`, ... (Numeric
/// settings take [`Numbers`](crate::numbers::Numbers) instead: any number
/// type, and ranges.) Colours take anything that converts to a
/// [`ColorSpec`] (one palette index as an `i32`); line types take a
/// [`LineType`], R's string or code; a character `pch` is its ASCII code,
/// or minus its Unicode code point for other characters (`'é'` is -233),
/// as in R.
///
/// # Examples
///
/// ```
/// use rs_graphics::setters::Values;
/// # use rs_graphics::prelude::*;
/// let one: Vec<f64> = 2_i32.into_values().unwrap();
/// assert_eq!(one, [2.0]);
/// let cols: Vec<ColorSpec> = ["red", "blue"].into_values().unwrap();
/// assert_eq!(cols[1], ColorSpec::Color(Color::parse("blue").unwrap()));
/// ```
pub trait Values<T> {
    /// The values, in order.
    ///
    /// # Errors
    ///
    /// If a value doesn't convert (an unknown colour name, ...); see
    /// [`IntoArg`].
    fn into_values(self) -> Result<Vec<T>, Error>;
}

impl<T, U: IntoArg<T>> Values<T> for Vec<U> {
    fn into_values(self) -> Result<Vec<T>, Error> {
        self.into_iter().map(IntoArg::into_arg).collect()
    }
}

impl<T, U: IntoArg<T>, const N: usize> Values<T> for [U; N] {
    fn into_values(self) -> Result<Vec<T>, Error> {
        self.into_iter().map(IntoArg::into_arg).collect()
    }
}

impl<T, U: IntoArg<T> + Clone> Values<T> for &[U] {
    fn into_values(self) -> Result<Vec<T>, Error> {
        self.iter().cloned().map(IntoArg::into_arg).collect()
    }
}

impl<T, U: IntoArg<T> + Clone, const N: usize> Values<T> for &[U; N] {
    fn into_values(self) -> Result<Vec<T>, Error> {
        self.iter().cloned().map(IntoArg::into_arg).collect()
    }
}

impl<T, U: IntoArg<T> + Clone> Values<T> for &Vec<U> {
    fn into_values(self) -> Result<Vec<T>, Error> {
        self.iter().cloned().map(IntoArg::into_arg).collect()
    }
}

/// `impl Values<$t> for $single` (one value) for each listed type.
macro_rules! single_values {
    ($t:ty: $($single:ty),*) => {
        $(
            impl Values<$t> for $single {
                fn into_values(self) -> Result<Vec<$t>, Error> {
                    Ok(vec![self.into_arg()?])
                }
            }
        )*
    };
}

single_values!(ColorSpec: ColorSpec, Color, &str, String, &String, i32);
single_values!(f64: f64, f32, i32, u32);
single_values!(i32: i32);
single_values!(LineType: LineType, &str, String, &String, i32);

/// A character `pch`, e.g. `'.'` or `'+'`: its ASCII code, or minus its
/// Unicode code point for other characters (`'é'` is -233), as in R.
impl Values<i32> for char {
    fn into_values(self) -> Result<Vec<i32>, Error> {
        Ok(vec![char_pch(self)])
    }
}

/// One value or several for a recycled style vector whose elements may be
/// missing (`None`, R's `NA`).
///
/// Takes a single value, an `Option` of one, or an array, `Vec` or slice
/// of either, by value or by reference: `19`, `[1, 2]`,
/// `[Some(19), None]`, `"dashed"`, `[None, Some("solid")]`, `&lwd[..]`,
/// ... Numeric settings take any number type ([`Num`](crate::numbers::Num))
/// and integer ranges (`1..=3`). Rust arrays hold one element type, so
/// once one element is `None` the others are written `Some(..)`.
///
/// `None` skips the element: no symbol, line or segment is drawn for it
/// (for text sizes and fonts, and `legend()`'s spacing, it means the
/// default instead; each setter says which).
///
/// # Examples
///
/// ```
/// use rs_graphics::setters::NaValues;
/// # use rs_graphics::prelude::*;
/// # let mut dev = Device::null().open().unwrap();
/// # dev.plot_new();
/// let pch: Vec<Option<i32>> = [Some(19_i32), None].into_na_values().unwrap();
/// assert_eq!(pch, [Some(19), None]);
/// // The second point has no symbol.
/// dev.points().x([0.2, 0.8]).y([0.5, 0.5]).pch([Some(19), None]).draw();
/// ```
pub trait NaValues<T> {
    /// The values, in order (`None`: R's `NA`).
    ///
    /// # Errors
    ///
    /// If a value doesn't convert (an unknown line type, ...); see
    /// [`IntoArg`].
    fn into_na_values(self) -> Result<Vec<Option<T>>, Error>;
}

/// `NaValues<$t>` for each element type, one value or a collection of them,
/// also as `Option`s. `$conv` converts one element to a `Result<$t, Error>`.
macro_rules! na_values {
    ($t:ty: $conv:expr; $($e:ty),*) => {
        $(
            impl NaValues<$t> for $e {
                fn into_na_values(self) -> Result<Vec<Option<$t>>, Error> {
                    Ok(vec![Some($conv(self)?)])
                }
            }
            impl NaValues<$t> for Option<$e> {
                fn into_na_values(self) -> Result<Vec<Option<$t>>, Error> {
                    Ok(vec![self.map($conv).transpose()?])
                }
            }
            na_values!(@many $t: $conv; $e);
        )*
    };
    (@many $t:ty: $conv:expr; $e:ty) => {
        na_values!(@impls $t; $e; |v: $e| $conv(v).map(Some));
        na_values!(@impls $t; Option<$e>; |v: Option<$e>| v.map($conv).transpose());
    };
    (@impls $t:ty; $e:ty; $one:expr) => {
        impl<const N: usize> NaValues<$t> for [$e; N] {
            fn into_na_values(self) -> Result<Vec<Option<$t>>, Error> {
                self.into_iter().map($one).collect()
            }
        }
        impl NaValues<$t> for Vec<$e> {
            fn into_na_values(self) -> Result<Vec<Option<$t>>, Error> {
                self.into_iter().map($one).collect()
            }
        }
        impl NaValues<$t> for &[$e] {
            fn into_na_values(self) -> Result<Vec<Option<$t>>, Error> {
                self.iter().cloned().map($one).collect()
            }
        }
        impl<const N: usize> NaValues<$t> for &[$e; N] {
            fn into_na_values(self) -> Result<Vec<Option<$t>>, Error> {
                self.iter().cloned().map($one).collect()
            }
        }
        impl NaValues<$t> for &Vec<$e> {
            fn into_na_values(self) -> Result<Vec<Option<$t>>, Error> {
                self.iter().cloned().map($one).collect()
            }
        }
    };
}

/// `NaValues<f64>` for one number, or a collection, of any [`Num`] type,
/// also as `Option`s; integer ranges.
mod na_numbers {
    use super::{Error, NaValues};
    use crate::numbers::{Num, Numbers};
    use std::ops::{Range, RangeInclusive};

    impl<N: Num> NaValues<f64> for N {
        fn into_na_values(self) -> Result<Vec<Option<f64>>, Error> {
            Ok(vec![Some(self.to_f64())])
        }
    }
    impl<N: Num> NaValues<f64> for Option<N> {
        fn into_na_values(self) -> Result<Vec<Option<f64>>, Error> {
            Ok(vec![self.map(Num::to_f64)])
        }
    }
    impl<N: Num, const M: usize> NaValues<f64> for [N; M] {
        fn into_na_values(self) -> Result<Vec<Option<f64>>, Error> {
            self.as_slice().into_na_values()
        }
    }
    impl<N: Num, const M: usize> NaValues<f64> for [Option<N>; M] {
        fn into_na_values(self) -> Result<Vec<Option<f64>>, Error> {
            self.as_slice().into_na_values()
        }
    }
    impl<N: Num> NaValues<f64> for Vec<N> {
        fn into_na_values(self) -> Result<Vec<Option<f64>>, Error> {
            Ok(self.into_iter().map(|v| Some(v.to_f64())).collect())
        }
    }
    impl<N: Num> NaValues<f64> for Vec<Option<N>> {
        fn into_na_values(self) -> Result<Vec<Option<f64>>, Error> {
            Ok(self.into_iter().map(|v| v.map(Num::to_f64)).collect())
        }
    }
    impl<N: Num> NaValues<f64> for &[N] {
        fn into_na_values(self) -> Result<Vec<Option<f64>>, Error> {
            Ok(self.iter().map(|v| Some(v.to_f64())).collect())
        }
    }
    impl<N: Num> NaValues<f64> for &[Option<N>] {
        fn into_na_values(self) -> Result<Vec<Option<f64>>, Error> {
            Ok(self.iter().map(|v| v.map(Num::to_f64)).collect())
        }
    }
    impl<N: Num, const M: usize> NaValues<f64> for &[N; M] {
        fn into_na_values(self) -> Result<Vec<Option<f64>>, Error> {
            self.as_slice().into_na_values()
        }
    }
    impl<N: Num, const M: usize> NaValues<f64> for &[Option<N>; M] {
        fn into_na_values(self) -> Result<Vec<Option<f64>>, Error> {
            self.as_slice().into_na_values()
        }
    }
    impl<N: Num> NaValues<f64> for &Vec<N> {
        fn into_na_values(self) -> Result<Vec<Option<f64>>, Error> {
            self.as_slice().into_na_values()
        }
    }
    impl<N: Num> NaValues<f64> for &Vec<Option<N>> {
        fn into_na_values(self) -> Result<Vec<Option<f64>>, Error> {
            self.as_slice().into_na_values()
        }
    }
    macro_rules! ranges {
        ($($t:ty),*) => {
            $(
                impl NaValues<f64> for Range<$t> {
                    fn into_na_values(self) -> Result<Vec<Option<f64>>, Error> {
                        Ok(self.into_numbers().iter().map(|&v| Some(v)).collect())
                    }
                }
                impl NaValues<f64> for RangeInclusive<$t> {
                    fn into_na_values(self) -> Result<Vec<Option<f64>>, Error> {
                        Ok(self.into_numbers().iter().map(|&v| Some(v)).collect())
                    }
                }
            )*
        };
    }
    ranges!(i32, i64, u32, u64, usize);
}
na_values!(i32: IntoArg::<i32>::into_arg; i32, i16, u8, u16);
na_values!(i32: |c: char| Ok::<i32, Error>(char_pch(c)); char);
na_values!(LineType: IntoArg::<LineType>::into_arg; LineType, &str, String, &String, i32);
na_values!(FontFace: IntoArg::<FontFace>::into_arg; FontFace, i32, &str, String, &String);

/// Plotting symbols for a `pch` setter: numbers, characters or strings.
///
/// One or several numbers (`19`, `[1, 2]`, `&pch`), characters (`'a'`,
/// `['a', 'b']`) or strings (`"a"`, `["a", "b"]`), by value or by
/// reference; `None` elements are R's `NA`, no symbol (`[Some(19), None]`,
/// `[Some("x"), None]`). Numbers are R's symbol codes 0 to 25 (see
/// [`charts::pch`](crate::charts::pch)). A character is drawn as itself:
/// ASCII ones are their character codes, other characters are stored as
/// minus their Unicode code point (R's negative `pch`), so `'é'` is `-233`.
/// A string's first character is its symbol, and `""` draws none, as in R.
///
/// `legend()` reads one case differently, as R's does: when the first
/// string has several characters, it alone is used, one character per key
/// (`"abc"` gives keys `a`, `b`, `c`; [`into_legend_pch`](Pch::into_legend_pch)).
///
/// This is R's `pch` argument.
///
/// # Examples
///
/// ```
/// use rs_graphics::prelude::*;
///
/// assert_eq!([19, 1].into_pch(), [Some(19), Some(1)]);
/// assert_eq!("abc".into_pch(), [Some(97)]);
/// assert_eq!("abc".into_legend_pch(), [Some(97), Some(98), Some(99)]);
/// assert_eq!([Some("x"), None].into_pch(), [Some(120), None]);
/// assert_eq!('é'.into_pch(), [Some(-233)]);
/// # let mut dev = Device::null().open().unwrap();
/// # dev.plot_new();
/// dev.points().x([0.2, 0.5, 0.8]).y([0.5; 3]).pch(["a", "b", "c"]).draw();
/// ```
pub trait Pch {
    /// The `pch` codes as `points()` reads them, in order; `None` for no
    /// symbol.
    fn into_pch(self) -> Vec<Option<i32>>;

    /// The `pch` codes as `legend()` reads them: as [`into_pch`](Pch::into_pch),
    /// except that a first string of several characters gives one code per
    /// character.
    fn into_legend_pch(self) -> Vec<Option<i32>>;
}

/// An element a [`Pch`] value is made of: an integer symbol code, a
/// character or a string.
///
/// Sealed: implemented only in this crate.
pub trait PchElement: pch_sealed::Sealed + Sized {
    /// The codes of `items` (`None` = `NA`); `legend` applies `legend()`'s
    /// rule for a first string of several characters.
    #[doc(hidden)]
    fn codes(items: &[Option<Self>], legend: bool) -> Vec<Option<i32>>;
}

mod pch_sealed {
    pub trait Sealed {}
}

macro_rules! pch_codes {
    ($($t:ty),*) => {
        $(
            impl pch_sealed::Sealed for $t {}
            impl PchElement for $t {
                fn codes(items: &[Option<Self>], _legend: bool) -> Vec<Option<i32>> {
                    items.iter().map(|v| v.map(i32::from)).collect()
                }
            }
        )*
    };
}
pch_codes!(i32, i16, u8, u16);

impl pch_sealed::Sealed for char {}
impl PchElement for char {
    fn codes(items: &[Option<Self>], _legend: bool) -> Vec<Option<i32>> {
        items.iter().map(|c| c.map(char_pch)).collect()
    }
}

macro_rules! pch_strings {
    ($($t:ty),*) => {
        $(
            impl pch_sealed::Sealed for $t {}
            impl PchElement for $t {
                fn codes(items: &[Option<Self>], legend: bool) -> Vec<Option<i32>> {
                    string_pch(items, legend)
                }
            }
        )*
    };
}
pch_strings!(&str, String, &String);

/// R's `pch` codes for strings (`None` = `NA`): each string's first
/// character (`""`: none), or with `legend` a first string of several
/// characters split into one code per character.
fn string_pch<S: AsRef<str>>(strings: &[Option<S>], legend: bool) -> Vec<Option<i32>> {
    match strings.first() {
        Some(Some(first)) if legend && first.as_ref().chars().count() > 1 => {
            first.as_ref().chars().map(|c| Some(char_pch(c))).collect()
        }
        _ => strings
            .iter()
            .map(|s| {
                s.as_ref()
                    .and_then(|s| s.as_ref().chars().next())
                    .map(char_pch)
            })
            .collect(),
    }
}

/// `Pch` for one shape of value: `$items` turns it into `Option` elements.
macro_rules! pch_shape {
    ($([$($g:tt)*] $ty:ty => |$v:ident| $items:expr;)*) => {
        $(
            impl<$($g)*> Pch for $ty {
                fn into_pch(self) -> Vec<Option<i32>> {
                    let $v = self;
                    E::codes(&$items, false)
                }
                fn into_legend_pch(self) -> Vec<Option<i32>> {
                    let $v = self;
                    E::codes(&$items, true)
                }
            }
        )*
    };
}
pch_shape! {
    [E: PchElement] E => |v| [Some(v)];
    [E: PchElement] Option<E> => |v| [v];
    [E: PchElement, const N: usize] [E; N] => |v| v.map(Some);
    [E: PchElement, const N: usize] [Option<E>; N] => |v| v;
    [E: PchElement] Vec<E> => |v| v.into_iter().map(Some).collect::<Vec<_>>();
    [E: PchElement] Vec<Option<E>> => |v| v;
    [E: PchElement + Clone] &[E] => |v| v.iter().cloned().map(Some).collect::<Vec<_>>();
    [E: PchElement] &[Option<E>] => |v| v;
    [E: PchElement + Clone, const N: usize] &[E; N] => |v| v.iter().cloned().map(Some).collect::<Vec<_>>();
    [E: PchElement, const N: usize] &[Option<E>; N] => |v| v.as_slice();
    [E: PchElement + Clone] &Vec<E> => |v| v.iter().cloned().map(Some).collect::<Vec<_>>();
    [E: PchElement] &Vec<Option<E>> => |v| v;
}

/// A character's `pch` code, as R's `GEstring_to_pch()` in a UTF-8 locale:
/// its ASCII code, or minus its Unicode code point for any other character
/// (`'é'` is -233).
pub(crate) fn char_pch(c: char) -> i32 {
    if c.is_ascii() { c as i32 } else { -(c as i32) }
}

/// Declares an options struct and its chainable setters from one list, so
/// each field's doc is written once:
///
/// ```text
/// options! {
///     /// Arguments of `segments()`.
///     #[derive(Clone, Debug, Default)]
///     pub struct SegmentsOptions => [SegmentsCall] {
///         /// Recycled per segment; empty: `par(fg)` (R's default).
///         pub col: Vec<ColorSpec> => values ColorSpec,
///     }
/// }
/// ```
///
/// gives the struct, a setter per field on it, and the same setters on
/// each listed builder in [`crate::graphics`] (writing to its `opts`),
/// plus `with(&Struct)` there. Fields are `pub(crate)` (whatever the
/// declaration says), so outside the crate the setters are the one way in. Each setter's doc is the field's own doc,
/// then (for kinds that take several forms) a line on what it takes.
/// Field docs should therefore have no headings (`# Panics`): that line
/// would land under them. Every field
/// needs its `=> kind Type`, so a field can't be added without a setter
/// by mistake; one set by a constructor is marked `=> no_setter`. The
/// builder list `=> [..]` may be left out.
///
/// Setter kinds: `value` (plain field), `option` (`Option<T>` from a
/// value or an `Option`, [`OptionArg`]), `option_value`
/// (`Option<T>` from a value only, for types where an `Option` is a value
/// itself: a colour's `None` is R's `NA`, as in colour vectors, and
/// [`par(plt)`]'s `None` is the `mar`-based region),
/// `number` (`Option<f64>` from any [`Num`](crate::numbers::Num) or an
/// `Option` of one, [`OptionNum`]),
/// `values` (`Vec<T>` recycled by R), `numbers` (`Vec<f64>` recycled by
/// R, from [`Numbers`](crate::numbers::Numbers)), `na_values`
/// (`Vec<Option<T>>` recycled by R, `None` = `NA`), `option_values`
/// (`Option<Vec<T>>`), `option_numbers` (`Option<Vec<f64>>` from
/// [`Numbers`](crate::numbers::Numbers)), `list` (`Option<Vec<Label>>` from one label or
/// several, [`LabelValues`](crate::plotmath::LabelValues)), `labels`
/// (`Option<Vec<Option<Label>>>` from [`Labels`](crate::plotmath::Labels),
/// `None` = `NA`), `pair` (`Option<(f64, f64)>` from two
/// numbers), `option_pair` (`Option<(f64, f64)>` from one
/// [`Pair`](crate::numbers::Pair) tuple), `array` (`Option<[f64; N]>`
/// from an array of any numbers), `na` (`Option<Option<f64>>`, R's
/// `NA`-able number, from one number; `f64::NAN` is `NA`), `count`
/// (`Option<usize>` from any integer), `pch` (`Vec<Option<i32>>` from
/// [`Pch`]) and `legend_pch` (the same, read as `legend()` does).
///
/// [`par(plt)`]: crate::par::Par::plt
macro_rules! options {
    (
        $(#[$sattr:meta])*
        $svis:vis struct $ty:ident $(=> [$($call:ident),+ $(,)?])? { $($body:tt)* }
    ) => {
        $crate::setters::options!(@struct [$(#[$sattr])*] [$svis] $ty { $($body)* });
        $crate::setters::options!(@opts $ty { $($body)* });
        $crate::setters::options!(@calls $ty [$($($call)+)?] { $($body)* });
    };
    (@calls $ty:ident [$($call:ident)*] $body:tt) => {
        $( $crate::setters::options!(@call $ty $call $body); )*
    };
    (@struct [$($sattr:tt)*] [$($svis:tt)*] $ty:ident {
        $( $(#[$fattr:meta])* $fvis:vis $field:ident : $fty:ty => $kind:ident $($t:ty)? ),* $(,)?
    }) => {
        $($sattr)*
        $($svis)* struct $ty {
            $( $(#[$fattr])* pub(crate) $field: $fty, )*
            /// The first value a setter couldn't convert, returned by the
            /// call's `try_draw()`.
            pub(crate) deferred_error: Option<$crate::error::Error>,
        }
    };
    (@opts $ty:ident {
        $( $(#[$fattr:meta])* $fvis:vis $field:ident : $fty:ty => $kind:ident $($t:ty)? ),* $(,)?
    }) => {
        // R's argument names, such as `sub` (a subtitle) and `add`, win over
        // clippy's wish to keep `std::ops` method names free.
        #[allow(clippy::should_implement_trait)]
        impl $ty {
            $( $crate::setters::options!(@field [] [$(#[$fattr])*] $ty $field $kind $($t)?); )*

            /// Keeps `e` for `try_draw()`, unless an earlier error is kept.
            pub(crate) fn defer(&mut self, e: $crate::error::Error) {
                if self.deferred_error.is_none() {
                    self.deferred_error = Some(e);
                }
            }
        }
    };
    (@call $ty:ident $call:ident {
        $( $(#[$fattr:meta])* $fvis:vis $field:ident : $fty:ty => $kind:ident $($t:ty)? ),* $(,)?
    }) => {
        #[allow(clippy::should_implement_trait)]
        impl<'a, G: $crate::graphics::Graphics> $crate::graphics::$call<'a, G> {
            #[doc = concat!("Uses a copy of `opts` (a reusable [`", stringify!($ty),
                "`]) for this call. Setters called afterwards change only this \
                call; setters called before are replaced.")]
            pub fn with(mut self, opts: &$ty) -> Self {
                self.opts = opts.clone();
                self
            }
            $( $crate::setters::options!(@field [opts] [$(#[$fattr])*] $ty $field $kind $($t)?); )*
        }
    };
    // A field without a setter.
    (@field [$($p:ident)?] [$($a:tt)*] $ty:ident $field:ident no_setter) => {};
    (@field [$($p:ident)?] [$($a:tt)*] $ty:ident $field:ident $kind:ident $t:ty) => {
        $crate::setters::options!(@one [$($p)?] [$($a)*] $ty $field $kind $t);
    };
    (@one [$($p:ident)?] [$($a:tt)*] $ty:ident $field:ident value $t:ty) => {
        $($a)*
        pub fn $field(mut self, value: impl $crate::setters::IntoArg<$t>) -> Self {
            match value.into_arg() {
                Ok(v) => self$(.$p)?.$field = v,
                Err(e) => self$(.$p)?.defer(e),
            }
            self
        }
    };
    (@one [$($p:ident)?] [$($a:tt)*] $ty:ident $field:ident option $t:ty) => {
        $($a)*
        #[doc = ""]
        #[doc = "Takes a value or an `Option` of one; `None` leaves the default (see [`OptionArg`](crate::setters::OptionArg))."]
        pub fn $field<M>(mut self, value: impl $crate::setters::OptionArg<$t, M>) -> Self {
            match value.into_option_arg() {
                Ok(v) => self$(.$p)?.$field = v,
                Err(e) => self$(.$p)?.defer(e),
            }
            self
        }
    };
    (@one [$($p:ident)?] [$($a:tt)*] $ty:ident $field:ident option_value $t:ty) => {
        $($a)*
        pub fn $field(mut self, value: impl $crate::setters::IntoArg<$t>) -> Self {
            match value.into_arg() {
                Ok(v) => self$(.$p)?.$field = Some(v),
                Err(e) => self$(.$p)?.defer(e),
            }
            self
        }
    };
    (@one [$($p:ident)?] [$($a:tt)*] $ty:ident $field:ident number $t:ty) => {
        $($a)*
        #[doc = ""]
        #[doc = "Takes any number, or an `Option` of one; `None` leaves the default (see [`OptionNum`](crate::setters::OptionNum))."]
        pub fn $field<M>(mut self, value: impl $crate::setters::OptionNum<M>) -> Self {
            self$(.$p)?.$field = value.into_option_num();
            self
        }
    };
    (@one [$($p:ident)?] [$($a:tt)*] $ty:ident $field:ident numbers $t:ty) => {
        $($a)*
        #[doc = ""]
        #[doc = "Takes one number or several, of any number type (see [`Numbers`](crate::numbers::Numbers))."]
        pub fn $field<'n>(mut self, values: impl $crate::numbers::Numbers<'n>) -> Self {
            self$(.$p)?.$field = values.into_numbers().into_owned();
            self
        }
    };
    (@one [$($p:ident)?] [$($a:tt)*] $ty:ident $field:ident option_numbers $t:ty) => {
        $($a)*
        #[doc = ""]
        #[doc = "Takes one number or several, of any number type, or an `Option` of them; `None` leaves the default (see [`OptionNumbers`](crate::setters::OptionNumbers))."]
        pub fn $field<'n, M>(mut self, values: impl $crate::setters::OptionNumbers<'n, M>) -> Self {
            self$(.$p)?.$field = values.into_option_numbers();
            self
        }
    };
    (@one [$($p:ident)?] [$($a:tt)*] $ty:ident $field:ident values $t:ty) => {
        $($a)*
        #[doc = ""]
        #[doc = "Takes one value or several (see [`Values`](crate::setters::Values))."]
        pub fn $field(mut self, values: impl $crate::setters::Values<$t>) -> Self {
            match values.into_values() {
                Ok(v) => self$(.$p)?.$field = v,
                Err(e) => self$(.$p)?.defer(e),
            }
            self
        }
    };
    (@one [$($p:ident)?] [$($a:tt)*] $ty:ident $field:ident count $t:ty) => {
        $($a)*
        #[doc = ""]
        #[doc = "Takes any integer; a negative one is an error of the call."]
        pub fn $field<N>(mut self, value: N) -> Self
        where
            N: TryInto<$t> + Copy + std::fmt::Display,
        {
            match value.try_into() {
                Ok(v) => self$(.$p)?.$field = Some(v),
                Err(_) => self$(.$p)?.defer($crate::error::Error::invalid(format!(
                    concat!("'", stringify!($field), "' must be a count, not {}"),
                    value
                ))),
            }
            self
        }
    };
    (@one [$($p:ident)?] [$($a:tt)*] $ty:ident $field:ident pch $t:ty) => {
        $($a)*
        #[doc = ""]
        #[doc = "Takes numbers, characters or strings; `None` is R's `NA` (see [`Pch`](crate::setters::Pch))."]
        pub fn $field(mut self, values: impl $crate::setters::Pch) -> Self {
            self$(.$p)?.$field = values.into_pch();
            self
        }
    };
    (@one [$($p:ident)?] [$($a:tt)*] $ty:ident $field:ident legend_pch $t:ty) => {
        $($a)*
        #[doc = ""]
        #[doc = "Takes numbers, characters or strings; `None` is R's `NA` (see [`Pch`](crate::setters::Pch))."]
        pub fn $field(mut self, values: impl $crate::setters::Pch) -> Self {
            self$(.$p)?.$field = values.into_legend_pch();
            self
        }
    };
    (@one [$($p:ident)?] [$($a:tt)*] $ty:ident $field:ident na_values $t:ty) => {
        $($a)*
        #[doc = ""]
        #[doc = "Takes one value or several; `None` is R's `NA` (see [`NaValues`](crate::setters::NaValues))."]
        pub fn $field(mut self, values: impl $crate::setters::NaValues<$t>) -> Self {
            match values.into_na_values() {
                Ok(v) => self$(.$p)?.$field = v,
                Err(e) => self$(.$p)?.defer(e),
            }
            self
        }
    };
    (@one [$($p:ident)?] [$($a:tt)*] $ty:ident $field:ident option_values $t:ty) => {
        $($a)*
        #[doc = ""]
        #[doc = "Takes one value or several (see [`Values`](crate::setters::Values))."]
        pub fn $field(mut self, values: impl $crate::setters::Values<$t>) -> Self {
            match values.into_values() {
                Ok(v) => self$(.$p)?.$field = Some(v),
                Err(e) => self$(.$p)?.defer(e),
            }
            self
        }
    };
    (@one [$($p:ident)?] [$($a:tt)*] $ty:ident $field:ident list $t:ty) => {
        $($a)*
        #[doc = ""]
        #[doc = "Takes one label or several (see [`LabelValues`](crate::plotmath::LabelValues))."]
        pub fn $field(mut self, values: impl $crate::plotmath::LabelValues) -> Self {
            self$(.$p)?.$field = Some(values.into_label_values());
            self
        }
    };
    (@one [$($p:ident)?] [$($a:tt)*] $ty:ident $field:ident labels $t:ty) => {
        $($a)*
        #[doc = ""]
        #[doc = "Takes one label or several; `None` is R's `NA` (see [`Labels`](crate::plotmath::Labels))."]
        pub fn $field(mut self, values: impl $crate::plotmath::Labels) -> Self {
            self$(.$p)?.$field = Some(values.into_labels());
            self
        }
    };
    (@one [$($p:ident)?] [$($a:tt)*] $ty:ident $field:ident pair $t:ty) => {
        $($a)*
        pub fn $field(
            mut self,
            a: impl $crate::numbers::Num,
            b: impl $crate::numbers::Num,
        ) -> Self {
            self$(.$p)?.$field = Some((a.to_f64(), b.to_f64()));
            self
        }
    };
    (@one [$($p:ident)?] [$($a:tt)*] $ty:ident $field:ident option_pair $t:ty) => {
        $($a)*
        pub fn $field(mut self, value: impl $crate::numbers::Pair) -> Self {
            self$(.$p)?.$field = Some(value.into_pair());
            self
        }
    };
    (@one [$($p:ident)?] [$($a:tt)*] $ty:ident $field:ident na $t:ty) => {
        $($a)*
        #[doc = ""]
        #[doc = "`f64::NAN` is R's `NA`."]
        pub fn $field(mut self, value: impl $crate::numbers::Num) -> Self {
            let v = value.to_f64();
            self$(.$p)?.$field = Some(v.is_finite().then_some(v));
            self
        }
    };
    (@one [$($p:ident)?] [$($a:tt)*] $ty:ident $field:ident array $t:ty) => {
        $($a)*
        pub fn $field<U: $crate::numbers::Num, const N: usize>(mut self, values: [U; N]) -> Self
        where
            [f64; N]: Into<$t>,
        {
            let values: [f64; N] = values.map($crate::numbers::Num::to_f64);
            self$(.$p)?.$field = Some(values.into());
            self
        }
    };
}
pub(crate) use options;

#[cfg(test)]
mod tests {
    use super::*;

    fn values<T>(v: impl Values<T>) -> Vec<T> {
        v.into_values().unwrap()
    }

    #[test]
    fn one_value_or_several() {
        let red = ColorSpec::try_from("red").unwrap();
        assert_eq!(values::<ColorSpec>("red"), vec![red]);
        assert_eq!(values::<ColorSpec>(2), vec![ColorSpec::Index(2)]);
        assert_eq!(values::<ColorSpec>(["red", "blue"]).len(), 2);
        assert_eq!(values::<ColorSpec>(vec![red, red]), vec![red, red]);
        assert_eq!(values::<f64>(1.5), vec![1.5]);
        assert_eq!(values::<f64>(&[1.0, 2.0][..]), vec![1.0, 2.0]);
        assert_eq!(values::<i32>('.'), vec![46]);
        assert_eq!(values::<LineType>([1, 2]).len(), 2);
        // A value that doesn't convert is an error, not a panic.
        assert!(Values::<ColorSpec>::into_values(["red", "notacolour"]).is_err());
        assert!(Values::<LineType>::into_values(-1).is_err());
    }

    #[test]
    #[allow(clippy::needless_borrows_for_generic_args)] // the `&` forms on purpose
    fn option_vectors_take_array_references() {
        use crate::device::{AblineOptions, XyOptions};
        let ab = AblineOptions::default().h(&[1, 2]).col(&["red", "blue"]);
        assert_eq!(ab.h, vec![1.0, 2.0]);
        assert_eq!(
            ab.col,
            vec![
                ColorSpec::try_from("red").unwrap(),
                ColorSpec::try_from("blue").unwrap()
            ]
        );
        let xy = XyOptions::default().pch(&[1, 2]).lwd(&[Some(1.0), None]);
        assert_eq!(xy.pch, vec![Some(1), Some(2)]);
        assert_eq!(xy.lwd, vec![Some(1.0), None]);
    }

    #[test]
    #[allow(clippy::needless_borrows_for_generic_args)] // the `&` forms on purpose
    fn pch_setters_take_every_shape() {
        use crate::device::XyOptions;
        let abc = vec![Some("a"), None, Some("bc")];
        let codes = [Some(97), None, Some(98)];
        assert_eq!(abc.clone().into_pch(), codes);
        assert_eq!((&abc).into_pch(), codes);
        assert_eq!(abc.as_slice().into_pch(), codes);
        assert_eq!((&[Some('a'), None]).into_pch(), [Some(97), None]);
        assert_eq!((&vec![1_u8, 2]).into_pch(), [Some(1), Some(2)]);
        assert_eq!(Some(String::from("z")).into_pch(), [Some(122)]);
        // The setters of points/lines/plot/boxplot take them all, and read
        // strings as `points()` does.
        assert_eq!(XyOptions::default().pch("abc").pch, [Some(97)]);
        assert_eq!(XyOptions::default().pch(&abc).pch, codes);
    }

    #[test]
    fn option_setters_take_a_value_or_an_option() {
        use crate::device::{AxisOptions, TitleOptions};
        use crate::par::ParOptions;
        use crate::plotmath::Label;
        let main: Option<String> = Some("Title".into());
        let t = TitleOptions::default().main(main).xlab(None::<&str>);
        assert_eq!(t.main, Some(Label::from("Title")));
        assert!(t.xlab.is_none());
        // `None` resets a value set before.
        let t = TitleOptions::default().main("a").main(None::<Label>);
        assert!(t.main.is_none());
        // Numbers, and enums from R's strings, the same way.
        let a = AxisOptions::default()
            .line(Some(2_u8))
            .las(Some(1))
            .lend(Some("butt"));
        assert_eq!((a.line, a.las), (Some(2.0), Some(1)));
        assert_eq!(a.lend, Some(crate::backend::LineEnd::Butt));
        assert!(
            AxisOptions::default()
                .line(2)
                .line(None::<f64>)
                .line
                .is_none()
        );
        assert_eq!(ParOptions::default().cex(Some(1.5)).cex, Some(1.5));
        // A failed conversion inside the `Option` is kept for `try_draw()`.
        assert!(
            AxisOptions::default()
                .lend(Some("wavy"))
                .deferred_error
                .is_some()
        );
        // Colours keep `None` as R's `NA` (draw nothing), as colour vectors do.
        let a = AxisOptions::default().col(None::<&str>);
        assert_eq!(a.col, Some(ColorSpec::None));
    }

    #[test]
    fn numeric_setters_take_any_number_type() {
        use crate::device::{AblineOptions, AxisOptions, LegendOptions, XyOptions};
        use crate::par::ParOptions;
        // Vectors: any number type, ranges, by value or by reference.
        let ab = AblineOptions::default().h(1_i64).v(vec![2_u64, 3]);
        assert_eq!((ab.h, ab.v), (vec![1.0], vec![2.0, 3.0]));
        let axis = AxisOptions::default()
            .at(1..=3)
            .tck(1_usize)
            .mgp([3_u8, 1, 0]);
        assert_eq!(axis.at, Some(vec![1.0, 2.0, 3.0]));
        assert_eq!((axis.tck, axis.mgp), (Some(1.0), Some([3.0, 1.0, 0.0])));
        // `NA`-able vectors: the same, plus `Option`s.
        let xy = XyOptions::default().cex(2_u8).lwd([Some(1_i64), None]);
        assert_eq!((xy.cex, xy.lwd), (vec![Some(2.0)], vec![Some(1.0), None]));
        assert_eq!(XyOptions::default().cex(1..3).cex, [Some(1.0), Some(2.0)]);
        // Pairs and single values.
        let legend = LegendOptions::default()
            .inset((0.1_f32, 1_u8))
            .cex(&[1_i16][..]);
        assert_eq!(legend.inset, Some((0.1_f32 as f64, 1.0)));
        assert_eq!(legend.cex, [Some(1.0)]);
        assert_eq!(ParOptions::default().cex(2_i64).cex, Some(2.0));
    }

    #[test]
    fn na_values_take_plain_values_and_none() {
        use crate::device::{SegmentsOptions, XyOptions};
        let xy = XyOptions::default().pch(19).cex([1, 2]).lwd(vec![1.5_f32]);
        assert_eq!(xy.pch, vec![Some(19)]);
        assert_eq!(xy.cex, vec![Some(1.0), Some(2.0)]);
        assert_eq!(xy.lwd, vec![Some(1.5)]);
        let xy = XyOptions::default()
            .pch([Some(1), None, Some(3)])
            .pch([Some('a'), None])
            .cex([None, Some(2)]);
        assert_eq!(xy.pch, vec![Some(97), None]);
        assert_eq!(xy.cex, vec![None, Some(2.0)]);
        let seg = SegmentsOptions::default()
            .lty([None, Some("dashed")])
            .lwd(&[Some(2.0), None][..])
            .col([Some("red"), None]);
        assert_eq!(seg.lty, vec![None, Some(LineType::DASHED)]);
        assert_eq!(seg.lwd, vec![Some(2.0), None]);
        assert_eq!(
            seg.col,
            vec![ColorSpec::try_from("red").unwrap(), ColorSpec::None]
        );
    }

    #[test]
    fn label_setters_take_one_label_or_several() {
        use crate::device::AxisOptions;
        use crate::highlevel::{BarplotOptions, BoxplotOptions, PieOptions};
        use crate::plotmath::Label;

        let a = Label::from("A");
        let bars = BarplotOptions::default()
            .names("A")
            .legend_text(String::from("A"));
        assert_eq!(bars.names, Some(vec![a.clone()]));
        assert_eq!(bars.legend_text, Some(vec![a.clone()]));
        let names = vec!["A".to_string(), "B".to_string()];
        let boxes = BoxplotOptions::default().names(&names);
        assert_eq!(boxes.names, Some(vec![a.clone(), Label::from("B")]));
        let pie = PieOptions::default().labels(Label::math("x^2"));
        assert_eq!(pie.labels, Some(vec![Label::math("x^2")]));
        // Axis labels may hold R's `NA`.
        let axis = AxisOptions::default().labels("A");
        assert_eq!(axis.labels, Some(vec![Some(a.clone())]));
        let axis = AxisOptions::default().labels([Some("A"), None]);
        assert_eq!(axis.labels, Some(vec![Some(a), None]));
    }

    #[test]
    fn setters_set_the_fields() {
        use crate::device::{AblineOptions, AxisOptions, TextOptions, XyOptions};
        use crate::highlevel::{BarplotOptions, PlotOptions};
        use crate::plotmath::Label;

        let xy = XyOptions::default()
            .pch('.')
            .col(["red", "blue"])
            .lty(LineType::DASHED);
        assert_eq!(xy.pch, vec![Some(46)]);
        assert_eq!(
            xy.col,
            vec![
                ColorSpec::try_from("red").unwrap(),
                ColorSpec::try_from("blue").unwrap()
            ]
        );
        assert_eq!(xy.lty, Some(LineType::DASHED));

        let axis = AxisOptions::default()
            .at([1.0, 2.0])
            .labels(["a", "b"])
            .las(1);
        assert_eq!(axis.at, Some(vec![1.0, 2.0]));
        assert_eq!(
            axis.labels,
            Some(vec![Some(Label::from("a")), Some(Label::from("b"))])
        );
        assert_eq!(axis.las, Some(1));

        let ab = AblineOptions::default().a(1.0).b([2, 3]).h(3.0);
        assert_eq!((ab.a, ab.b, ab.h), (vec![1.0], vec![2.0, 3.0], vec![3.0]));

        let plot = PlotOptions::default()
            .xlim(0.0, 10.0)
            .main(Label::math("x^2"))
            .axes(false);
        assert_eq!(plot.xlim, Some((0.0, 10.0)));
        assert_eq!(plot.main, Some(Label::math("x^2")));
        assert!(!plot.axes);

        let bars = BarplotOptions::default().names(["a", "b"]).col("grey");
        assert_eq!(bars.names, Some(vec![Label::from("a"), Label::from("b")]));
        assert_eq!(bars.col, vec![ColorSpec::try_from("grey").unwrap()]);

        assert_eq!(TextOptions::default().srt(45.0).srt, Some(45.0));
    }

    #[test]
    fn calls_have_the_same_setters_and_with() {
        use crate::backend::svg::SvgBackend;
        use crate::device::{AxisOptions, Device};
        use crate::graphics::Graphics;
        use crate::par::Par;

        let mut dev = Device::new(SvgBackend::new(4.0, 4.0), Par::default());
        let saved = AxisOptions::default().las(1).at([1.0, 2.0]).lwd(3.0);

        let call = dev.axis().side(2).las(2).tick(false);
        assert_eq!((call.opts.las, call.opts.tick), (Some(2), false));

        // `with` replaces what was set before it; later setters override
        // just this call, leaving `saved` alone.
        let call = dev.axis().side(2).las(2).with(&saved).lwd(0.5);
        assert_eq!(call.opts.las, Some(1));
        assert_eq!(call.opts.at, Some(vec![1.0, 2.0]));
        assert_eq!(call.opts.lwd, Some(0.5));
        assert_eq!(saved.lwd, Some(3.0));
    }

    #[test]
    fn font_setters_take_numbers_names_and_faces() {
        use crate::device::{AxisOptions, LegendOptions, TextOptions};
        use crate::par::ParOptions;
        assert_eq!(AxisOptions::default().font(2).font, Some(FontFace::Bold));
        assert_eq!(
            AxisOptions::default().font("bold.italic").font,
            Some(FontFace::BoldItalic)
        );
        assert_eq!(
            ParOptions::default().font_main(FontFace::Italic).font_main,
            Some(FontFace::Italic)
        );
        assert_eq!(
            TextOptions::default().font([1, 2]).font,
            vec![Some(FontFace::Plain), Some(FontFace::Bold)]
        );
        assert_eq!(
            LegendOptions::default()
                .text_font([Some("italic"), None])
                .text_font,
            vec![Some(FontFace::Italic), None]
        );
    }

    #[test]
    fn setters_take_r_strings() {
        use crate::backend::{LineEnd, LineJoin};
        use crate::device::{AxisOptions, LegendBty, LegendOptions, XyOptions};
        use crate::highlevel::{BarplotOptions, BoxplotOptions, OrderH, PlotOptions, PlotType};
        use crate::par::{AxisStyle, AxisType, ParOptions, Pty};

        let xy = XyOptions::default().type_("b").lty("dashed");
        assert_eq!(xy.type_, Some(PlotType::Both));
        assert_eq!(xy.lty, Some(LineType::DASHED));
        let p = PlotOptions::default().type_("h").xaxt("n");
        assert_eq!(
            (p.type_, p.xaxt),
            (Some(PlotType::Histogram), Some(AxisType::None))
        );
        assert_eq!(AxisOptions::default().axt("n").axt, Some(AxisType::None));
        assert_eq!(LegendOptions::default().bty("n").bty, Some(LegendBty::None));
        assert_eq!(
            BarplotOptions::default().order_h("decr").order_h,
            OrderH::Decr
        );
        let b = BoxplotOptions::default()
            .lty(["dashed", "dotted"])
            .boxlty("44");
        assert_eq!(b.lty, vec![Some(LineType::DASHED), Some(LineType::DOTTED)]);
        assert_eq!(b.boxlty, vec![Some(LineType::parse("44").unwrap())]);
        let par = ParOptions::default()
            .xaxs("i")
            .pty("s")
            .lty(String::from("dotdash"))
            .lend("butt")
            .ljoin("mitre")
            .yaxt("n");
        assert_eq!(par.xaxs, Some(AxisStyle::Exact));
        assert_eq!(par.pty, Some(Pty::Square));
        assert_eq!(par.lty, Some(LineType::DOTDASH));
        assert_eq!(par.lend, Some(LineEnd::Butt));
        assert_eq!(par.ljoin, Some(LineJoin::Mitre));
        assert_eq!(par.yaxt, Some(AxisType::None));

        use crate::device::BoxOptions;
        use crate::par::Bty;
        let par = ParOptions::default().bty("l").col_main("red");
        assert_eq!(par.bty, Some(Bty::L));
        assert_eq!(par.col_main, Some(Color::rgb(255, 0, 0)));
        assert_eq!(BoxOptions::default().bty("]").bty, Some(Bty::Bracket));
        assert_eq!(PlotOptions::default().bty("[").bty, Some(Bty::C));
        assert_eq!(Bty::parse("7"), Some(Bty::Seven));
        assert_eq!(Bty::parse("outline"), Some(Bty::O)); // first character, as R
        assert_eq!(Bty::parse("x"), None);
    }

    #[test]
    fn invalid_values_are_kept_not_panicked_on() {
        use crate::device::{AxisOptions, XyOptions};
        let xy = XyOptions::default().type_("z").lty("0x");
        // The first error is kept; the fields keep their defaults.
        let e = xy.deferred_error.clone().unwrap();
        assert!(
            e.message().starts_with("\"z\" is not a valid plot type"),
            "{e}"
        );
        assert_eq!((xy.type_, xy.lty), (None, None));
        let e = XyOptions::default().lty("0x").deferred_error.unwrap();
        assert!(
            e.message().starts_with("\"0x\" is not a valid line type"),
            "{e}"
        );
        let e = XyOptions::default()
            .col(["red", "notacolour"])
            .deferred_error
            .unwrap();
        assert_eq!(e.message(), "invalid color name 'notacolour'");
        let e = AxisOptions::default().font(9).deferred_error.unwrap();
        assert!(e.message().starts_with("invalid font 9"), "{e}");
        let e = crate::device::LegendOptions::default()
            .ncol(-1)
            .deferred_error
            .unwrap();
        assert_eq!(e.message(), "'ncol' must be a count, not -1");
        assert!(XyOptions::default().lty("dashed").deferred_error.is_none());
    }
}
