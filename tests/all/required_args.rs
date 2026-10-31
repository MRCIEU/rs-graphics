// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! A required argument (one R gives no default) left out of a call is an
//! error with R's message (`try_draw()`; `.draw()` panics with it), on a
//! device and when a `Figure` records the call. (No R needed.)

use std::panic::{AssertUnwindSafe, catch_unwind};

use rs_graphics::Error;
use rs_graphics::prelude::*;

fn device() -> Device<AnyBackend> {
    let mut dev = Device::null().width(5).height(5).open().unwrap();
    dev.plot_new();
    dev
}

/// The panic message of `f`, or `None` if it didn't panic.
fn panic_message(f: impl FnOnce()) -> Option<String> {
    let err = catch_unwind(AssertUnwindSafe(f)).err()?;
    err.downcast_ref::<String>()
        .cloned()
        .or_else(|| err.downcast_ref::<&str>().map(|s| s.to_string()))
}

/// A call on a device or a `Figure`, through `try_draw()`.
type Call<G> = fn(&mut G) -> Result<(), Error>;

/// Each call with one required argument left out: R's name for the call,
/// the argument, and the call.
fn calls<G: Graphics>() -> Vec<(&'static str, &'static str, Call<G>)> {
    vec![
        ("plot", "x", |g| g.plot().y([1.0, 2.0]).try_draw()),
        ("hist", "x", |g| g.hist().try_draw().map(drop)),
        ("barplot", "height", |g| g.barplot().try_draw().map(drop)),
        ("boxplot", "x", |g| g.boxplot().try_draw().map(drop)),
        ("pie", "x", |g| g.pie().try_draw()),
        ("plot.window", "xlim", |g| {
            g.plot_window().ylim([0, 1]).try_draw()
        }),
        ("plot.window", "ylim", |g| {
            g.plot_window().xlim([0, 1]).try_draw()
        }),
        ("points", "x", |g| g.points().y([1.0]).try_draw()),
        ("lines", "x", |g| g.lines().try_draw()),
        ("polygon", "x", |g| g.polygon().try_draw()),
        ("segments", "x0", |g| g.segments().y0([1.0]).try_draw()),
        ("segments", "y0", |g| g.segments().x0([1.0]).try_draw()),
        ("arrows", "x0", |g| g.arrows().y0([1.0]).try_draw()),
        ("rect", "ytop", |g| {
            g.rect()
                .xleft([0.0])
                .ybottom([0.0])
                .xright([1.0])
                .try_draw()
        }),
        ("text", "x", |g| g.text().labels("a").try_draw()),
        ("axis", "side", |g| g.axis().try_draw().map(drop)),
        ("legend", "pos", |g| {
            g.legend().labels("a").try_draw().map(drop)
        }),
        ("legend", "labels", |g| {
            g.legend().pos("topleft").try_draw().map(drop)
        }),
        ("mtext", "text", |g| g.mtext().side(1).try_draw()),
    ]
}

/// Checks `calls()` on a fresh `G` from `new`.
fn check<G: Graphics>(what: &str, new: impl Fn() -> G) {
    for (name, arg, call) in calls::<G>() {
        let e = call(&mut new()).unwrap_err();
        let message = format!("argument \"{arg}\" is missing, with no default");
        assert_eq!(
            (e.call(), e.kind(), e.message()),
            (name, ErrorKind::MissingArgument, message.as_str()),
            "{what}, `{arg}`"
        );
        assert_eq!(e.to_string(), format!("{name}(): {message}"));
    }
}

#[test]
fn missing_required_arguments_are_errors_like_r() {
    check("device", device);
    check("figure", Figure::new);
}

#[test]
fn draw_panics_with_the_error() {
    let msg = panic_message(|| {
        device().plot().y([1.0, 2.0]).draw();
    });
    assert_eq!(
        msg.as_deref(),
        Some("plot(): argument \"x\" is missing, with no default")
    );
    let msg = panic_message(|| {
        Figure::new().legend().pos("topleft").draw();
    });
    assert_eq!(
        msg.as_deref(),
        Some("legend(): argument \"labels\" is missing, with no default")
    );
}

#[test]
fn arguments_with_r_defaults_can_be_left_out() {
    // `y` (one-vector form), `x1`/`y1`, `labels` and `mtext`'s `side`.
    let mut dev = device();
    dev.plot_window().xlim([0, 5]).ylim([0, 5]).draw();
    dev.points().x([1.0, 2.0]).draw();
    dev.segments().x0([1.0]).y0([1.0]).draw();
    dev.arrows().x0([1.0]).y0([1.0]).y1([3.0]).draw();
    dev.text().x([1.0, 2.0]).y([1.0, 2.0]).draw();
    dev.mtext().text("top").draw();
    dev.box_().draw();
    dev.abline().h(&[1.0][..]).draw();
    dev.title().main("t").draw();
}
