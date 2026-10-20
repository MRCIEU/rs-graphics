// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Numeric arguments that take integers as well as `f64`, as R's numbers
//! do: data vectors ([`Numbers`]: `dev.plot(&[1, 2, 3], &y)` or
//! `dev.plot(1..=10, &y)`, R's `plot(1:10, y)`), groups of them
//! ([`Groups`], for `boxplot()`) and pairs ([`Pair`]: `(5, 20)`).
//!
//! `f64` data is borrowed as it is; other numbers are converted to `f64`
//! once (`i64`/`u64`/`usize` values beyond 2^53 lose precision, as in R,
//! whose numbers are doubles).

use std::borrow::Cow;
use std::ops::{Range, RangeInclusive};

mod sealed {
    pub trait Sealed {}
}

/// A number type that converts to `f64`: `f64`, `f32` and the integer
/// types. Sealed: implemented only here.
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

/// A data vector (R's numeric vector `x`, `y`, ...): a reference to a
/// slice, array or `Vec` of any [`Num`] type (`&x`, `&[1, 2, 3]`), or an
/// integer range (`1..=10` is R's `1:10`). `f64` data is borrowed without
/// copying.
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
        )*
    };
}
converted_numbers!(f32, i8, i16, i32, i64, u8, u16, u32, u64, usize, isize);

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

/// Groups of data vectors (`boxplot()`'s R list of groups): a reference
/// to a slice, array or `Vec` of `Vec`s of any [`Num`] type. `f64` groups
/// are borrowed without copying.
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
        )*
    };
}
converted_groups!(f32, i8, i16, i32, i64, u8, u16, u32, u64, usize, isize);

/// Two numbers, such as limits `(0, 10)` or a point `(5.0, 20.0)`: a
/// tuple of `f64`s and/or `i32`s.
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
}
