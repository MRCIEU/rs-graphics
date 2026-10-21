// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! A required argument (one R gives no default) left out of a call panics
//! at `.draw()` with R's message, on a device and when a `Figure` records
//! the call. (No R needed.)

use std::panic::{AssertUnwindSafe, catch_unwind};

use rs_graphics::prelude::*;

fn device() -> Device<SvgBackend> {
    let mut dev = Device::new(SvgBackend::new(5.0, 5.0), Par::default());
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

/// A call on a device or a `Figure`.
type Call<G> = fn(&mut G);

/// Each call with one required argument left out, and that argument.
fn calls<G: Graphics>() -> Vec<(&'static str, Call<G>)> {
    vec![
        ("x", |g| g.plot().y(&[1.0, 2.0]).draw()),
        ("x", |g| {
            g.hist().draw();
        }),
        ("height", |g| {
            g.barplot().draw();
        }),
        ("x", |g| {
            g.boxplot().draw();
        }),
        ("x", |g| g.pie().draw()),
        ("xlim", |g| g.plot_window().ylim(0, 1).draw()),
        ("ylim", |g| g.plot_window().xlim(0, 1).draw()),
        ("x", |g| g.points().y(&[1.0]).draw()),
        ("x", |g| g.lines().draw()),
        ("x", |g| g.polygon().draw()),
        ("x0", |g| g.segments().y0(&[1.0]).draw()),
        ("y0", |g| g.segments().x0(&[1.0]).draw()),
        ("x0", |g| g.arrows().y0(&[1.0]).draw()),
        ("ytop", |g| {
            g.rect().xleft(&[0.0]).ybottom(&[0.0]).xright(&[1.0]).draw()
        }),
        ("x", |g| g.text().labels("a").draw()),
        ("side", |g| {
            g.axis().draw();
        }),
        ("pos", |g| {
            g.legend().labels("a").draw();
        }),
        ("labels", |g| {
            g.legend().pos("topleft").draw();
        }),
        ("text", |g| g.mtext().side(1).draw()),
    ]
}

#[test]
fn missing_required_arguments_panic_like_r() {
    for (arg, call) in calls::<Device<SvgBackend>>() {
        let msg = panic_message(|| call(&mut device()));
        assert_eq!(
            msg.as_deref(),
            Some(format!("argument \"{arg}\" is missing, with no default").as_str()),
            "device, `{arg}`"
        );
    }
    for (arg, call) in calls::<Figure>() {
        let msg = panic_message(|| call(&mut Figure::new()));
        assert_eq!(
            msg.as_deref(),
            Some(format!("argument \"{arg}\" is missing, with no default").as_str()),
            "figure, `{arg}`"
        );
    }
}

#[test]
fn arguments_with_r_defaults_can_be_left_out() {
    // `y` (one-vector form), `x1`/`y1`, `labels` and `mtext`'s `side`.
    let mut dev = device();
    dev.plot_window().xlim(0, 5).ylim(0, 5).draw();
    dev.points().x(&[1.0, 2.0]).draw();
    dev.segments().x0(&[1.0]).y0(&[1.0]).draw();
    dev.arrows().x0(&[1.0]).y0(&[1.0]).y1(&[3.0]).draw();
    dev.text().x(&[1.0, 2.0]).y(&[1.0, 2.0]).draw();
    dev.mtext().text("top").draw();
    dev.box_().draw();
    dev.abline().h(&[1.0][..]).draw();
    dev.title().main("t").draw();
}
