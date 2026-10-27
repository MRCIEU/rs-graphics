// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Traits for numeric arguments that take integers as well as `f64`: data
//! vectors ([`Numbers`]), groups of them ([`Groups`]) and pairs
//! ([`Pair`]).
//!
//! As in R, where all numbers are doubles, any number type works:
//! `dev.plot().x([1, 2, 3]).y(&y)`, or `dev.plot().x(1..=10).y(&y)` for
//! R's `plot(1:10, y)`. One number is a vector of one
//! (`dev.plot().x(1).y(1)` for R's `plot(1, 1)`). For `boxplot()`, one
//! vector is one group. Pairs are tuples such as `(5, 20)`.
//!
//! Data may be passed by reference or by value (`&x` or `x`, `&[1, 2]` or
//! `[1, 2]`), as may the recycled option vectors (`.h([1, 2])`,
//! `.col(&cols)`). `f64` data is borrowed as it is, or taken over when a
//! `Vec<f64>` is moved in; other numbers are converted to `f64`
//! once (`i64`/`u64`/`usize` values beyond 2^53 lose precision, as in R,
//! whose numbers are doubles).
//!
//! ```
//! # use rs_graphics::prelude::*;
//! # let mut dev = Device::null().width(4).height(4).open().unwrap();
//! let y = vec![2.0, 3.5, 1.0];
//! dev.plot().x(1..=3).y(&y).xlim(0, 4).draw();
//! ```

use std::borrow::Cow;
use std::ops::{Range, RangeInclusive};

mod sealed {
    pub trait Sealed {}
}

/// A number type that converts to `f64`: `f64`, `f32` and the integer
/// types.
///
/// Sealed: implemented only in this crate.
///
/// # Examples
///
/// ```
/// use rs_graphics::numbers::Num;
/// assert_eq!(3u8.to_f64(), 3.0);
/// ```
pub trait Num: Copy + sealed::Sealed {
    /// The value as an `f64` (rounded for 64-bit integers beyond 2^53).
    fn to_f64(self) -> f64;
}

macro_rules! num {
    ($($t:ty),*) => {
        $(
            impl sealed::Sealed for $t {}
            impl Num for $t {
                fn to_f64(self) -> f64 {
                    self as f64
                }
            }
        )*
    };
}
num!(f64, f32, i8, i16, i32, i64, u8, u16, u32, u64, usize, isize);

/// A data vector, such as the `x` and `y` of a plot.
///
/// Takes a slice, array or `Vec` of any [`Num`] type, by reference or by
/// value (`&x`, `x`, `&[1, 2, 3]`, `[1, 2, 3]`), an integer range
/// (`1..=10` is R's `1:10`), or one number (`1`, `2.5`, `&v`), R's
/// length-one vector. `f64` data passed by reference is borrowed, and a
/// `Vec<f64>` passed by value is taken over, both without copying; other
/// number types are converted once.
///
/// This is R's numeric vector.
///
/// # Examples
///
/// ```
/// # use rs_graphics::prelude::*;
/// assert_eq!(&*(1..=3_i32).into_numbers(), &[1.0, 2.0, 3.0]);
/// assert_eq!(&*[1_i32, 2].into_numbers(), &[1.0, 2.0]);
/// assert_eq!(&*2.5_f64.into_numbers(), &[2.5]);
/// ```
pub trait Numbers<'a> {
    /// The values as `f64`s.
    fn into_numbers(self) -> Cow<'a, [f64]>;
}

impl<'a> Numbers<'a> for Cow<'a, [f64]> {
    fn into_numbers(self) -> Cow<'a, [f64]> {
        self
    }
}

impl<'a> Numbers<'a> for &'a [f64] {
    fn into_numbers(self) -> Cow<'a, [f64]> {
        Cow::Borrowed(self)
    }
}

impl<'a> Numbers<'a> for &'a Vec<f64> {
    fn into_numbers(self) -> Cow<'a, [f64]> {
        Cow::Borrowed(self)
    }
}

/// A reference to a slice reference, e.g. from iterating over `&[&[f64]]`.
impl<'a, 'b: 'a> Numbers<'a> for &'a &'b [f64] {
    fn into_numbers(self) -> Cow<'a, [f64]> {
        Cow::Borrowed(*self)
    }
}

impl<'a, const N: usize> Numbers<'a> for &'a [f64; N] {
    fn into_numbers(self) -> Cow<'a, [f64]> {
        Cow::Borrowed(self)
    }
}

impl<'a> Numbers<'a> for Vec<f64> {
    fn into_numbers(self) -> Cow<'a, [f64]> {
        Cow::Owned(self)
    }
}

impl<'a, const N: usize> Numbers<'a> for [f64; N] {
    fn into_numbers(self) -> Cow<'a, [f64]> {
        Cow::Owned(self.to_vec())
    }
}

/// Converting impls for the number types other than `f64`.
macro_rules! converted_numbers {
    ($($t:ty),*) => {
        $(
            impl<'a> Numbers<'a> for &'a [$t] {
                fn into_numbers(self) -> Cow<'a, [f64]> {
                    Cow::Owned(self.iter().map(|v| v.to_f64()).collect())
                }
            }

            impl<'a, 'b: 'a> Numbers<'a> for &'a &'b [$t] {
                fn into_numbers(self) -> Cow<'a, [f64]> {
                    (*self).into_numbers()
                }
            }

            impl<'a> Numbers<'a> for &'a Vec<$t> {
                fn into_numbers(self) -> Cow<'a, [f64]> {
                    self.as_slice().into_numbers()
                }
            }

            impl<'a, const N: usize> Numbers<'a> for &'a [$t; N] {
                fn into_numbers(self) -> Cow<'a, [f64]> {
                    self.as_slice().into_numbers()
                }
            }

            impl<'a> Numbers<'a> for Vec<$t> {
                fn into_numbers(self) -> Cow<'a, [f64]> {
                    Cow::Owned(self.iter().map(|v| v.to_f64()).collect())
                }
            }

            impl<'a, const N: usize> Numbers<'a> for [$t; N] {
                fn into_numbers(self) -> Cow<'a, [f64]> {
                    Cow::Owned(self.iter().map(|v| v.to_f64()).collect())
                }
            }
        )*
    };
}
converted_numbers!(f32, i8, i16, i32, i64, u8, u16, u32, u64, usize, isize);

/// One number, as R's length-one vector (`plot(1, 1)` is
/// `.x(1).y(1)`), by value or by reference.
macro_rules! one_number {
    ($($t:ty),*) => {
        $(
            impl<'a> Numbers<'a> for $t {
                fn into_numbers(self) -> Cow<'a, [f64]> {
                    Cow::Owned(vec![self.to_f64()])
                }
            }

            impl<'a> Numbers<'a> for &'a $t {
                fn into_numbers(self) -> Cow<'a, [f64]> {
                    (*self).into_numbers()
                }
            }
        )*
    };
}
one_number!(f64, f32, i8, i16, i32, i64, u8, u16, u32, u64, usize, isize);

/// Integer ranges (`1..=10` is R's `1:10`).
macro_rules! range_numbers {
    ($($t:ty),*) => {
        $(
            impl<'a> Numbers<'a> for Range<$t> {
                fn into_numbers(self) -> Cow<'a, [f64]> {
                    Cow::Owned(self.map(|v| v.to_f64()).collect())
                }
            }

            impl<'a> Numbers<'a> for RangeInclusive<$t> {
                fn into_numbers(self) -> Cow<'a, [f64]> {
                    Cow::Owned(self.map(|v| v.to_f64()).collect())
                }
            }
        )*
    };
}
range_numbers!(i32, i64, u32, u64, usize);

/// Groups of data vectors, such as the groups of a `boxplot()`.
///
/// Takes a slice, array or `Vec` of `Vec`s of any [`Num`] type, by
/// reference or by value, or one data vector ([`Numbers`], one number
/// included), which is one group, as R's `boxplot(x)`. `f64` groups are
/// borrowed (by reference) or taken over (a `Vec` by value) without
/// copying.
///
/// This is R's list of groups.
///
/// # Examples
///
/// ```
/// # use rs_graphics::prelude::*;
/// let groups = [vec![1, 2], vec![3]].into_groups();
/// assert_eq!(&*groups, &[vec![1.0, 2.0], vec![3.0]]);
/// assert_eq!([1_i32, 2, 3].into_groups().len(), 1);
/// ```
pub trait Groups<'a> {
    /// The groups as `f64` vectors.
    fn into_groups(self) -> Cow<'a, [Vec<f64>]>;
}

impl<'a> Groups<'a> for &'a [Vec<f64>] {
    fn into_groups(self) -> Cow<'a, [Vec<f64>]> {
        Cow::Borrowed(self)
    }
}

impl<'a> Groups<'a> for &'a Vec<Vec<f64>> {
    fn into_groups(self) -> Cow<'a, [Vec<f64>]> {
        Cow::Borrowed(self)
    }
}

impl<'a, const N: usize> Groups<'a> for &'a [Vec<f64>; N] {
    fn into_groups(self) -> Cow<'a, [Vec<f64>]> {
        Cow::Borrowed(self)
    }
}

impl<'a> Groups<'a> for Vec<Vec<f64>> {
    fn into_groups(self) -> Cow<'a, [Vec<f64>]> {
        Cow::Owned(self)
    }
}

impl<'a, const N: usize> Groups<'a> for [Vec<f64>; N] {
    fn into_groups(self) -> Cow<'a, [Vec<f64>]> {
        Cow::Owned(self.into())
    }
}

/// One data vector (or number) is one group, as R's `boxplot(x)`.
impl<'a, N: Numbers<'a>> Groups<'a> for N {
    fn into_groups(self) -> Cow<'a, [Vec<f64>]> {
        Cow::Owned(vec![self.into_numbers().into_owned()])
    }
}

fn convert_groups<T: Num>(groups: &[Vec<T>]) -> Vec<Vec<f64>> {
    groups
        .iter()
        .map(|g| g.iter().map(|v| v.to_f64()).collect())
        .collect()
}

macro_rules! converted_groups {
    ($($t:ty),*) => {
        $(
            impl<'a> Groups<'a> for &'a [Vec<$t>] {
                fn into_groups(self) -> Cow<'a, [Vec<f64>]> {
                    Cow::Owned(convert_groups(self))
                }
            }

            impl<'a> Groups<'a> for &'a Vec<Vec<$t>> {
                fn into_groups(self) -> Cow<'a, [Vec<f64>]> {
                    Cow::Owned(convert_groups(self))
                }
            }

            impl<'a, const N: usize> Groups<'a> for &'a [Vec<$t>; N] {
                fn into_groups(self) -> Cow<'a, [Vec<f64>]> {
                    Cow::Owned(convert_groups(self))
                }
            }

            impl<'a> Groups<'a> for Vec<Vec<$t>> {
                fn into_groups(self) -> Cow<'a, [Vec<f64>]> {
                    Cow::Owned(convert_groups(&self))
                }
            }

            impl<'a, const N: usize> Groups<'a> for [Vec<$t>; N] {
                fn into_groups(self) -> Cow<'a, [Vec<f64>]> {
                    Cow::Owned(convert_groups(&self))
                }
            }
        )*
    };
}
converted_groups!(f32, i8, i16, i32, i64, u8, u16, u32, u64, usize, isize);

/// Two numbers, such as limits `(0, 10)` or a point `(5.0, 20.0)`.
///
/// Implemented for tuples of `f64`s and/or `i32`s.
///
/// # Examples
///
/// ```
/// # use rs_graphics::prelude::*;
/// assert_eq!((5, 20).into_pair(), (5.0, 20.0));
/// assert_eq!((0.5, 2).into_pair(), (0.5, 2.0));
/// ```
pub trait Pair {
    /// The two values as `f64`s.
    fn into_pair(self) -> (f64, f64);
}

macro_rules! pair {
    ($(($a:ty, $b:ty)),*) => {
        $(
            impl Pair for ($a, $b) {
                fn into_pair(self) -> (f64, f64) {
                    (self.0.to_f64(), self.1.to_f64())
                }
            }
        )*
    };
}
pair!((f64, f64), (f64, i32), (i32, f64), (i32, i32));

#[cfg(test)]
// The tests use the reference forms on purpose; clippy would drop the `&`.
#[allow(clippy::needless_borrows_for_generic_args)]
mod tests {
    use super::*;

    fn n<'a>(v: impl Numbers<'a>) -> Cow<'a, [f64]> {
        v.into_numbers()
    }

    #[test]
    fn f64_data_is_borrowed_and_other_numbers_converted() {
        let x = vec![1.5, 2.5];
        assert!(matches!(n(&x), Cow::Borrowed(_)));
        assert!(matches!(n(&x[..]), Cow::Borrowed(_)));
        assert!(matches!(n(&[1.0, 2.0]), Cow::Borrowed(_)));
        // A slice reference behind another (e.g. from `for s in &slices`).
        let s: &[f64] = &x;
        let ss: &&[f64] = &s;
        assert!(matches!(n(ss), Cow::Borrowed(_)));
        assert_eq!(&*n(&[1, 2, 3]), &[1.0, 2.0, 3.0]);
        assert_eq!(&*n(&vec![4u64, 5]), &[4.0, 5.0]);
        assert_eq!(&*n(&[0.5f32]), &[0.5]);
        assert_eq!(&*n(&vec![7usize]), &[7.0]);
        assert_eq!(&*n(1..=3), &[1.0, 2.0, 3.0]);
        assert_eq!(&*n(0..2), &[0.0, 1.0]);
        let g = vec![vec![1.0], vec![2.0, 3.0]];
        assert!(matches!(g.as_slice().into_groups(), Cow::Borrowed(_)));
        assert_eq!(
            &*(&vec![vec![1, 2], vec![3]]).into_groups(),
            &[vec![1.0, 2.0], vec![3.0]]
        );
        assert_eq!((5, 20).into_pair(), (5.0, 20.0));
        assert_eq!((0.5, 2).into_pair(), (0.5, 2.0));
    }

    #[test]
    fn data_by_value_equals_by_reference_without_copying() {
        let x = vec![1.5, 2.5];
        assert_eq!(n(&x), n(x.clone()));
        assert_eq!(n(&[1.0, 2.0]), n([1.0, 2.0]));
        assert_eq!(n(&[1, 2]), n([1, 2]));
        assert_eq!(n(&vec![3u8]), n(vec![3u8]));
        // A `Vec<f64>` passed by value is taken over, not copied.
        let ptr = x.as_ptr();
        match n(x) {
            Cow::Owned(v) => assert_eq!(v.as_ptr(), ptr),
            Cow::Borrowed(_) => panic!("expected the Vec itself"),
        }
        let g = vec![vec![1.0], vec![2.0, 3.0]];
        assert_eq!(g.as_slice().into_groups(), g.clone().into_groups());
        assert_eq!(
            [vec![1, 2], vec![3]].into_groups(),
            (&[vec![1, 2], vec![3]]).into_groups()
        );
    }
}
