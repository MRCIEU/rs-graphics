// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! R's recycling rule, `v[i %% length(v)]`, in the forms the drawing code
//! needs. An empty vector is R's `NULL`: the argument wasn't given.

/// Element `i` of `v` recycled, or `None` when `v` is empty (R's
/// `pcycle()`).
pub(crate) fn nth<T: Copy>(v: &[T], i: usize) -> Option<T> {
    (!v.is_empty()).then(|| v[i % v.len()])
}

/// Element `i` of `v` recycled, or `default` when `v` is empty.
pub(crate) fn pick<T: Copy>(v: &[T], i: usize, default: T) -> T {
    nth(v, i).unwrap_or(default)
}

/// Element `i` of a recycled [`NaValues`](crate::setters::NaValues)
/// vector: `Some(default)` when the vector is empty (R's default), `None`
/// for an `NA` element.
pub(crate) fn pick_na<T: Copy>(v: &[Option<T>], i: usize, default: T) -> Option<T> {
    pick(v, i, Some(default))
}

/// Element `i` of a recycled numeric setting whose `NA` is `None`:
/// `Some(Some(x))` for a number, `Some(None)` for `NA` (`None`, or a `NaN`
/// kept from before `None` was its spelling), `None` when `v` is empty.
pub(crate) fn nth_na(v: &[Option<f64>], i: usize) -> Option<Option<f64>> {
    nth(v, i).map(|x| x.filter(|x| !x.is_nan()))
}

/// `v` recycled to length `n` (R's `rep_len()`); empty stays empty.
pub(crate) fn rep_len<T: Copy>(v: &[T], n: usize) -> Vec<T> {
    if v.is_empty() {
        Vec::new()
    } else {
        (0..n).map(|i| v[i % v.len()]).collect()
    }
}

/// `v` recycled to length `n`, or `n` copies of `default` when `v` is
/// empty.
pub(crate) fn rep_or<T: Copy>(v: &[T], n: usize, default: T) -> Vec<T> {
    (0..n).map(|i| pick(v, i, default)).collect()
}

/// The number of elements after recycling vectors of lengths `lens`
/// together: the longest, or 0 when any is empty.
pub(crate) fn recycled_len(lens: &[usize]) -> usize {
    if lens.contains(&0) {
        0
    } else {
        lens.iter().copied().max().unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recycling_follows_r() {
        assert_eq!(nth(&[1, 2, 3], 4), Some(2));
        assert_eq!(nth::<i32>(&[], 4), None);
        assert_eq!(pick(&[], 4, 7), 7);
        // `None` survives recycling; an empty vector gives the default.
        let v = [Some(1.0), None];
        assert_eq!(
            (0..4).map(|i| pick_na(&v, i, 9.0)).collect::<Vec<_>>(),
            vec![Some(1.0), None, Some(1.0), None]
        );
        assert_eq!(pick_na(&[], 3, 9.0), Some(9.0));
        assert_eq!(rep_len(&[1, 2], 5), [1, 2, 1, 2, 1]);
        assert_eq!(rep_len::<i32>(&[], 5), []);
        assert_eq!(rep_or(&[], 2, 0), [0, 0]);
        assert_eq!(recycled_len(&[2, 5, 1]), 5);
        assert_eq!(recycled_len(&[2, 0]), 0);
    }
}
