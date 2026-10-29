// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Options that R's functions don't have: shortcuts for results that take
//! several calls in R.
//!
//! Each is an extra setter on an R call's builder, documented "**Not in
//! R.**" with the R code it replaces, and draws what that R code draws. The
//! R-faithful setters are unchanged: a call without extras draws exactly
//! what R draws.
//!
//! | Setter | On | R code it replaces |
//! |---|---|---|
//! | `label_srt(deg)` | [`axis()`](crate::graphics::Graphics::axis); the x axis of [`plot()`](crate::graphics::Graphics::plot) and [`hist()`](crate::graphics::Graphics::hist), the names axis of [`barplot()`](crate::graphics::Graphics::barplot) and [`boxplot()`](crate::graphics::Graphics::boxplot) | `axis(labels = FALSE)`, then `text(srt = deg, adj = c(1, 0.5), xpd = NA)` at the labels' line |
//! | `panel_bg(col)` | `plot()`, `hist()`, `barplot()`, `boxplot()` | `panel.first = rect(<par("usr")>, col = col, border = NA)` |
//!
//! ```
//! use rs_graphics::prelude::*;
//! # let mut dev = Device::null().open().unwrap();
//! dev.par().mar([7, 4, 2, 1]).draw();
//! dev.barplot()
//!     .height([3, 5, 2])
//!     .names(["first group", "second group", "third group"])
//!     .label_srt(45)
//!     .panel_bg("grey92")
//!     .draw();
//! ```
//
// For maintainers: every extra lives in this file. Its value is stored in
// the hidden `extra` field of the call's options struct (so `.with(&opts)`
// and `Figure`s carry it), its setters are declared with `extras!` below
// (which `tests/all/generated_docs.rs` reads for the R reference), and the
// R function's `*_impl` calls one function here per place it acts. With the
// extra unset, that function changes nothing.

use crate::backend::Backend;
use crate::color::ColorSpec;
use crate::device::{AxisLabelStyle, AxisOptions, Device, RectOptions};
use crate::error::Error;
use crate::highlevel::{BarplotOptions, BoxplotOptions, HistOptions, PlotOptions};

/// Declares an extras struct and its setters, on the options structs and
/// builders listed: `struct X for [Options, ..] => [Call, ..] { field: Type
/// => kind, .. }`, kinds `number` (any number or an `Option`) and `color`
/// (anything that converts to a colour; a failed conversion is the call's
/// error).
macro_rules! extras {
    (
        $(#[$sattr:meta])*
        struct $ty:ident for [$($opts:ident),+] => [$($call:ident),+] $body:tt
    ) => {
        extras!(@struct [$(#[$sattr])*] $ty $body);
        $( extras!(@opts $opts $body); )+
        $( extras!(@call $call $body); )+
    };
    (@struct [$($sattr:tt)*] $ty:ident {
        $( $(#[$fattr:meta])* $field:ident : $fty:ty => $kind:ident ),* $(,)?
    }) => {
        $($sattr)*
        #[derive(Clone, Debug, Default, PartialEq)]
        pub(crate) struct $ty {
            $( pub(crate) $field: $fty, )*
        }
    };
    (@opts $opts:ident {
        $( $(#[$fattr:meta])* $field:ident : $fty:ty => $kind:ident ),* $(,)?
    }) => {
        /// **Not in R.** Setters for options R's function doesn't have; see
        /// [`extra`](crate::extra).
        impl $opts {
            $( extras!(@setter [extra] [] [$(#[$fattr])*] $field $kind); )*
        }
    };
    (@call $call:ident {
        $( $(#[$fattr:meta])* $field:ident : $fty:ty => $kind:ident ),* $(,)?
    }) => {
        /// **Not in R.** Setters for options R's function doesn't have; see
        /// [`extra`](crate::extra).
        impl<G: crate::graphics::Graphics> crate::graphics::$call<'_, G> {
            $( extras!(@setter [opts extra] [opts] [$(#[$fattr])*] $field $kind); )*
        }
    };
    (@setter [$($p:ident)*] [$($d:ident)*] [$($a:tt)*] $field:ident number) => {
        $($a)*
        #[doc = ""]
        #[doc = "Takes any number, or an `Option` of one; `None` leaves it unset (see [`OptionNum`](crate::setters::OptionNum))."]
        pub fn $field<M>(mut self, value: impl crate::setters::OptionNum<M>) -> Self {
            self$(.$p)*.$field = value.into_option_num();
            self
        }
    };
    (@setter [$($p:ident)*] [$($d:ident)*] [$($a:tt)*] $field:ident color) => {
        $($a)*
        pub fn $field(mut self, value: impl crate::setters::IntoArg<ColorSpec>) -> Self {
            match value.into_arg() {
                Ok(v) => self$(.$p)*.$field = Some(v),
                Err(e) => self$(.$d)*.defer(e),
            }
            self
        }
    };
}

extras! {
    /// `axis()`'s extras.
    struct AxisExtras for [AxisOptions] => [AxisCall] {
        /// **Not in R.** Rotates the tick labels by this many degrees
        /// (anticlockwise, as `text()`'s `srt`), each ending at its tick:
        /// placed `mgp[1]` lines beyond the axis line, as `las = 2` labels
        /// are, right-justified on sides 1 and 2 (left on 3 and 4) unless
        /// `hadj` says otherwise, centred across on the "M" height unless
        /// `padj` does, and none dropped for overlapping. So 90 on side 1 is
        /// `las = 2`. R code: `axis(labels = FALSE)`, then `text(at, <the
        /// labels' line>, labels, srt = deg, adj = c(1, 0.5), xpd = NA)`.
        label_srt: Option<f64> => number,
    }
}

extras! {
    /// The high-level plots' extras.
    struct PlotExtras for [PlotOptions, HistOptions, BarplotOptions, BoxplotOptions]
        => [PlotCall, HistCall, BarplotCall, BoxplotCall] {
        /// **Not in R.** Rotates the x axis's tick labels (`barplot()`,
        /// `boxplot()`: the names axis) by this many degrees; see
        /// `axis()`'s `label_srt`.
        label_srt: Option<f64> => number,
        /// **Not in R.** Fills the plot region with this colour before
        /// anything is drawn in it, so the data, grid lines and axes sit on
        /// top. R code: `panel.first = rect(<par("usr")>, col = col, border
        /// = NA)` (in data units on a log axis).
        panel_bg: Option<ColorSpec> => color,
    }
}

/// `axis()`'s labels with `label_srt`: rotated by it, placed and aligned as
/// perpendicular labels, none dropped for overlapping.
pub(crate) fn axis_labels(extra: &AxisExtras, style: &mut AxisLabelStyle) {
    if let Some(srt) = extra.label_srt.filter(|s| s.is_finite()) {
        style.angle = srt;
        style.parallel = false;
        style.padj = 0.5;
        style.drop_overlaps = false;
    }
}

/// `opts` for the axis that a high-level plot's `label_srt` rotates.
pub(crate) fn rotated_axis(extra: &PlotExtras, mut opts: AxisOptions) -> AxisOptions {
    if extra.label_srt.is_some() {
        opts.extra.label_srt = extra.label_srt;
    }
    opts
}

/// Fills the plot region with `panel_bg`, as R's `rect()` over the user
/// coordinates' limits with no border would (clipped as `xpd` says).
pub(crate) fn panel_bg<B: Backend>(dev: &mut Device<B>, extra: &PlotExtras) -> Result<(), Error> {
    let Some(col) = extra.panel_bg else {
        return Ok(());
    };
    let [x0, x1, y0, y1] = dev.usr();
    let data = |v: f64, log: bool| if log { 10f64.powf(v) } else { v };
    let (xlog, ylog) = (dev.xlog(), dev.ylog());
    let style = RectOptions {
        col: vec![col],
        border: vec![ColorSpec::None],
        ..RectOptions::default()
    };
    dev.rect_impl(
        &[data(x0, xlog)],
        &[data(y0, ylog)],
        &[data(x1, xlog)],
        &[data(y1, ylog)],
        &style,
    )
}
