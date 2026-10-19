// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Chainable setters for the options structs, so that
//!
//! ```
//! # use rs_graphics::prelude::*;
//! let opts = XyOptions::default().pch(19).col(["red", "blue"]).cex(1.5);
//! # let _ = opts;
//! ```
//!
//! is the same as filling in the fields. Each setter is named after its
//! field and documented there. `Option` fields take the value itself
//! (`.las(1)`, `.main("Title")`); fields R recycles per element take one
//! value or several (see [`Values`]); limits take two numbers
//! (`.xlim(0.0, 10.0)`).

use crate::color::{Color, ColorSpec};
use crate::lty::LineType;

/// One value or several, for a field R recycles per element: `"red"`,
/// `["red", "blue"]`, `vec![1.0, 2.0]`, `&cex[..]`, ...
pub trait Values<T> {
    fn into_values(self) -> Vec<T>;
}

impl<T, U: Into<T>> Values<T> for Vec<U> {
    fn into_values(self) -> Vec<T> {
        self.into_iter().map(Into::into).collect()
    }
}

impl<T, U: Into<T>, const N: usize> Values<T> for [U; N] {
    fn into_values(self) -> Vec<T> {
        self.into_iter().map(Into::into).collect()
    }
}

impl<T, U: Into<T> + Clone> Values<T> for &[U] {
    fn into_values(self) -> Vec<T> {
        self.iter().cloned().map(Into::into).collect()
    }
}

impl<T, U: Into<T> + Clone> Values<T> for &Vec<U> {
    fn into_values(self) -> Vec<T> {
        self.iter().cloned().map(Into::into).collect()
    }
}

/// `impl Values<$t> for $single` (one value) for each listed type.
macro_rules! single_values {
    ($t:ty: $($single:ty),*) => {
        $(
            impl Values<$t> for $single {
                fn into_values(self) -> Vec<$t> {
                    vec![self.into()]
                }
            }
        )*
    };
}

single_values!(ColorSpec: ColorSpec, Color, &str, String, &String, i32);
single_values!(f64: f64);
single_values!(i32: i32);
single_values!(LineType: LineType);

/// A character `pch`, e.g. `'.'` or `'+'`.
impl Values<i32> for char {
    fn into_values(self) -> Vec<i32> {
        vec![self as i32]
    }
}

/// `setters!(Struct { field: kind Type, ... })` adds a chainable setter for
/// each field. Kinds: `value` (plain field), `option` (`Option<T>`, set to
/// `Some`), `values` (`Vec<T>` recycled by R), `option_values`
/// (`Option<Vec<T>>`), `strings` (`Option<Vec<String>>`), `labels`
/// (`Option<Vec<Option<Label>>>`) and `pair` (`Option<(f64, f64)>`).
macro_rules! setters {
    ($ty:ident { $($field:ident : $kind:ident $t:ty),* $(,)? }) => {
        // R's argument names, such as `sub` (a subtitle) and `add`, win over
        // clippy's wish to keep `std::ops` method names free.
        #[allow(clippy::should_implement_trait)]
        impl $ty {
            $( $crate::setters::setters!(@one $ty $field $kind $t); )*
        }
    };
    (@one $ty:ident $field:ident value $t:ty) => {
        #[doc = concat!("Sets [`", stringify!($ty), "::", stringify!($field), "`].")]
        pub fn $field(mut self, value: impl Into<$t>) -> Self {
            self.$field = value.into();
            self
        }
    };
    (@one $ty:ident $field:ident option $t:ty) => {
        #[doc = concat!("Sets [`", stringify!($ty), "::", stringify!($field), "`].")]
        pub fn $field(mut self, value: impl Into<$t>) -> Self {
            self.$field = Some(value.into());
            self
        }
    };
    (@one $ty:ident $field:ident values $t:ty) => {
        #[doc = concat!("Sets [`", stringify!($ty), "::", stringify!($field),
            "`] to one value or several.")]
        pub fn $field(mut self, values: impl $crate::setters::Values<$t>) -> Self {
            self.$field = values.into_values();
            self
        }
    };
    (@one $ty:ident $field:ident option_values $t:ty) => {
        #[doc = concat!("Sets [`", stringify!($ty), "::", stringify!($field),
            "`] to one value or several.")]
        pub fn $field(mut self, values: impl $crate::setters::Values<$t>) -> Self {
            self.$field = Some(values.into_values());
            self
        }
    };
    (@one $ty:ident $field:ident strings $t:ty) => {
        #[doc = concat!("Sets [`", stringify!($ty), "::", stringify!($field), "`].")]
        pub fn $field<S: Into<String>>(mut self, values: impl IntoIterator<Item = S>) -> Self {
            self.$field = Some(values.into_iter().map(Into::into).collect());
            self
        }
    };
    (@one $ty:ident $field:ident labels $t:ty) => {
        #[doc = concat!("Sets [`", stringify!($ty), "::", stringify!($field),
            "`], all present (set the field directly for `NA` labels).")]
        pub fn $field<L: Into<$crate::plotmath::Label>>(
            mut self,
            values: impl IntoIterator<Item = L>,
        ) -> Self {
            self.$field = Some(values.into_iter().map(|l| Some(l.into())).collect());
            self
        }
    };
    (@one $ty:ident $field:ident pair $t:ty) => {
        #[doc = concat!("Sets [`", stringify!($ty), "::", stringify!($field), "`].")]
        pub fn $field(mut self, a: f64, b: f64) -> Self {
            self.$field = Some((a, b));
            self
        }
    };
}
pub(crate) use setters;

#[cfg(test)]
mod tests {
    use super::*;

    fn values<T>(v: impl Values<T>) -> Vec<T> {
        v.into_values()
    }

    #[test]
    fn one_value_or_several() {
        let red = ColorSpec::from("red");
        assert_eq!(values::<ColorSpec>("red"), vec![red]);
        assert_eq!(values::<ColorSpec>(2), vec![ColorSpec::Index(2)]);
        assert_eq!(values::<ColorSpec>(["red", "blue"]).len(), 2);
        assert_eq!(values::<ColorSpec>(vec![red, red]), vec![red, red]);
        assert_eq!(values::<f64>(1.5), vec![1.5]);
        assert_eq!(values::<f64>(&[1.0, 2.0][..]), vec![1.0, 2.0]);
        assert_eq!(values::<i32>('.'), vec![46]);
        assert_eq!(values::<LineType>([LineType::SOLID, 2.into()]).len(), 2);
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
        assert_eq!(xy.pch, vec![46]);
        assert_eq!(
            xy.col,
            vec![ColorSpec::from("red"), ColorSpec::from("blue")]
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

        let ab = AblineOptions::default().coef(1.0, 2.0).h(3.0);
        assert_eq!((ab.coef, ab.h), (Some((1.0, 2.0)), vec![3.0]));

        let plot = PlotOptions::default()
            .xlim(0.0, 10.0)
            .main(Label::math("x^2"))
            .axes(false);
        assert_eq!(plot.xlim, Some((0.0, 10.0)));
        assert_eq!(plot.main, Some(Label::math("x^2")));
        assert!(!plot.axes);

        let bars = BarplotOptions::default().names(["a", "b"]).col("grey");
        assert_eq!(bars.names, Some(vec!["a".to_string(), "b".to_string()]));
        assert_eq!(bars.col, Some(vec![ColorSpec::from("grey")]));

        assert_eq!(TextOptions::default().srt(45.0).srt, 45.0);
    }
}
