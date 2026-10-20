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
//! (`.xlim(0.0, 10.0)`). As in R, numbers may be integers or floats
//! (`.cex(2)`, `.xlim(0, 10)`, `.mar([5, 4, 4, 2])`). Fields holding one of R's string-valued choices
//! also take R's string (`.lty("dashed")`, `.type_("b")`, `.bty("n")`,
//! `.order_h("decr")`); an invalid string panics, as R errors.

use crate::color::{Color, ColorSpec};
use crate::lty::LineType;

/// One value or several, for a field R recycles per element: `"red"`,
/// `["red", "blue"]`, `vec![1.0, 2.0]`, `&cex[..]`, `2`, `[1, 2]`, ...
/// (numeric fields take `f64`, `f32`, `i32` or `u32` values).
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
single_values!(f64: f64, f32, i32, u32);
single_values!(i32: i32);
single_values!(LineType: LineType, &str, String, &String);

/// A character `pch`, e.g. `'.'` or `'+'`.
impl Values<i32> for char {
    fn into_values(self) -> Vec<i32> {
        vec![self as i32]
    }
}

/// `setters!(Struct { field: kind Type, ... })` adds a chainable setter for
/// each field. Kinds: `value` (plain field), `option` (`Option<T>`, set to
/// `Some`), `values` (`Vec<T>` recycled by R), `option_values`
/// (`Option<Vec<T>>`), `list` (`Option<Vec<T>>` from any iterator of
/// `Into<T>`, e.g. strings for `Label`s), `labels`
/// (`Option<Vec<Option<Label>>>`), `pair` (`Option<(f64, f64)>` from two
/// numbers), `option_pair` (`Option<(f64, f64)>` from one
/// [`Pair`](crate::numbers::Pair) tuple) and `array` (`Option<[f64; N]>`
/// from an array of any numbers).
///
/// `setters!(Struct => [CallA, CallB] { ... })` also gives each listed
/// builder in [`crate::graphics`] the same setters (writing to its
/// `opts`), plus `with(&Struct)`.
macro_rules! setters {
    ($ty:ident $body:tt) => {
        $crate::setters::setters!(@opts $ty $body);
    };
    ($ty:ident => [$($call:ident),+ $(,)?] $body:tt) => {
        $crate::setters::setters!(@opts $ty $body);
        $( $crate::setters::setters!(@call $ty $call $body); )+
    };
    (@opts $ty:ident { $($field:ident : $kind:ident $t:ty),* $(,)? }) => {
        // R's argument names, such as `sub` (a subtitle) and `add`, win over
        // clippy's wish to keep `std::ops` method names free.
        #[allow(clippy::should_implement_trait)]
        impl $ty {
            $( $crate::setters::setters!(@one [] $ty $field $kind $t); )*
        }
    };
    (@call $ty:ident $call:ident { $($field:ident : $kind:ident $t:ty),* $(,)? }) => {
        #[allow(clippy::should_implement_trait)]
        impl<'a, G: $crate::graphics::Graphics> $crate::graphics::$call<'a, G> {
            #[doc = concat!("Uses a copy of `opts` (a reusable [`", stringify!($ty),
                "`]) for this call. Setters called afterwards change only this \
                call; setters called before are replaced.")]
            pub fn with(mut self, opts: &$ty) -> Self {
                self.opts = opts.clone();
                self
            }
            $( $crate::setters::setters!(@one [opts] $ty $field $kind $t); )*
        }
    };
    (@one [$($p:ident)?] $ty:ident $field:ident value $t:ty) => {
        #[doc = concat!("Sets [`", stringify!($ty), "::", stringify!($field), "`].")]
        pub fn $field(mut self, value: impl Into<$t>) -> Self {
            self$(.$p)?.$field = value.into();
            self
        }
    };
    (@one [$($p:ident)?] $ty:ident $field:ident option $t:ty) => {
        #[doc = concat!("Sets [`", stringify!($ty), "::", stringify!($field), "`].")]
        pub fn $field(mut self, value: impl Into<$t>) -> Self {
            self$(.$p)?.$field = Some(value.into());
            self
        }
    };
    (@one [$($p:ident)?] $ty:ident $field:ident values $t:ty) => {
        #[doc = concat!("Sets [`", stringify!($ty), "::", stringify!($field),
            "`] to one value or several.")]
        pub fn $field(mut self, values: impl $crate::setters::Values<$t>) -> Self {
            self$(.$p)?.$field = values.into_values();
            self
        }
    };
    (@one [$($p:ident)?] $ty:ident $field:ident option_values $t:ty) => {
        #[doc = concat!("Sets [`", stringify!($ty), "::", stringify!($field),
            "`] to one value or several.")]
        pub fn $field(mut self, values: impl $crate::setters::Values<$t>) -> Self {
            self$(.$p)?.$field = Some(values.into_values());
            self
        }
    };
    (@one [$($p:ident)?] $ty:ident $field:ident list $t:ty) => {
        #[doc = concat!("Sets [`", stringify!($ty), "::", stringify!($field), "`].")]
        pub fn $field<S: Into<$t>>(mut self, values: impl IntoIterator<Item = S>) -> Self {
            self$(.$p)?.$field = Some(values.into_iter().map(Into::into).collect());
            self
        }
    };
    (@one [$($p:ident)?] $ty:ident $field:ident labels $t:ty) => {
        #[doc = concat!("Sets [`", stringify!($ty), "::", stringify!($field),
            "`], all present (set the field directly for `NA` labels).")]
        pub fn $field<L: Into<$crate::plotmath::Label>>(
            mut self,
            values: impl IntoIterator<Item = L>,
        ) -> Self {
            self$(.$p)?.$field = Some(values.into_iter().map(|l| Some(l.into())).collect());
            self
        }
    };
    (@one [$($p:ident)?] $ty:ident $field:ident pair $t:ty) => {
        #[doc = concat!("Sets [`", stringify!($ty), "::", stringify!($field), "`].")]
        pub fn $field(mut self, a: impl Into<f64>, b: impl Into<f64>) -> Self {
            self$(.$p)?.$field = Some((a.into(), b.into()));
            self
        }
    };
    (@one [$($p:ident)?] $ty:ident $field:ident option_pair $t:ty) => {
        #[doc = concat!("Sets [`", stringify!($ty), "::", stringify!($field), "`].")]
        pub fn $field(mut self, value: impl $crate::numbers::Pair) -> Self {
            self$(.$p)?.$field = Some(value.into_pair());
            self
        }
    };
    (@one [$($p:ident)?] $ty:ident $field:ident array $t:ty) => {
        #[doc = concat!("Sets [`", stringify!($ty), "::", stringify!($field), "`].")]
        pub fn $field<U: Into<f64>, const N: usize>(mut self, values: [U; N]) -> Self
        where
            [f64; N]: Into<$t>,
        {
            let values: [f64; N] = values.map(Into::into);
            self$(.$p)?.$field = Some(values.into());
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
        assert_eq!(bars.names, Some(vec![Label::from("a"), Label::from("b")]));
        assert_eq!(bars.col, vec![ColorSpec::from("grey")]);

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

        let call = dev.axis(2).las(2).tick(false);
        assert_eq!((call.opts.las, call.opts.tick), (Some(2), false));

        // `with` replaces what was set before it; later setters override
        // just this call, leaving `saved` alone.
        let call = dev.axis(2).las(2).with(&saved).lwd(0.5);
        assert_eq!(call.opts.las, Some(1));
        assert_eq!(call.opts.at, Some(vec![1.0, 2.0]));
        assert_eq!(call.opts.lwd, Some(0.5));
        assert_eq!(saved.lwd, Some(3.0));
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
        assert_eq!(b.lty, vec![LineType::DASHED, LineType::DOTTED]);
        assert_eq!(b.boxlty, vec![LineType::parse("44").unwrap()]);
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
    }

    #[test]
    #[should_panic(expected = "\"z\" is not a valid plot type")]
    fn invalid_r_string_panics() {
        let _ = crate::device::XyOptions::default().type_("z");
    }

    #[test]
    #[should_panic(expected = "\"0x\" is not a valid line type")]
    fn invalid_lty_string_panics() {
        let _ = crate::device::XyOptions::default().lty("0x");
    }
}
