// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! R's errors as values: `try_draw()` returns each with R's message and the
//! R function's name, on a device and (for errors in the arguments) when a
//! `Figure` records the call; `.draw()` panics with the same text. The
//! messages are compared with R's `conditionMessage()`.

use crate::common;

use std::process::Command;

use rs_graphics::error::{Error, ErrorKind};
use rs_graphics::prelude::*;

/// A call on a device or a `Figure`, through `try_draw()`.
type Call<G> = fn(&mut G) -> Result<(), Error>;

/// Errors in a call's arguments, found on a device and when a `Figure`
/// records the call: R's code (after `plot(1:10)`), R's name for the
/// call, and the call.
fn argument_errors<G: Graphics>() -> Vec<(&'static str, &'static str, Call<G>)> {
    vec![
        ("text(1, 1, 'a', pos = 5)", "text", |g| {
            g.text().x(1).y(1).labels("a").pos(5).try_draw()
        }),
        ("pie(c(1, -1))", "pie", |g| g.pie().x([1, -1]).try_draw()),
        ("axis(1, labels = c('a', 'b'))", "axis", |g| {
            g.axis().side(1).labels(["a", "b"]).try_draw().map(drop)
        }),
        ("axis(1, at = 1:3, labels = c('a', 'b'))", "axis", |g| {
            g.axis()
                .side(1)
                .at([1, 2, 3])
                .labels(["a", "b"])
                .try_draw()
                .map(drop)
        }),
        ("abline(a = 1)", "abline", |g| g.abline().a(1).try_draw()),
        ("abline(a = NA, b = 1)", "abline", |g| {
            g.abline().a(f64::NAN).b(1).try_draw()
        }),
        ("legend('topleft', 'a', text.width = -1)", "legend", |g| {
            g.legend()
                .pos("topleft")
                .labels("a")
                .text_width(-1)
                .try_draw()
                .map(drop)
        }),
        ("legend('topleft', character(0))", "legend", |g| {
            g.legend()
                .pos("topleft")
                .labels([""; 0])
                .try_draw()
                .map(drop)
        }),
        ("mtext(character(0))", "mtext", |g| {
            g.mtext().text([""; 0]).try_draw()
        }),
        ("points(1, 1, lmitre = 0.5)", "points", |g| {
            g.points().x(1).y(1).lmitre(0.5).try_draw()
        }),
        ("par(lmitre = 0.5)", "par", |g| {
            g.par().lmitre(0.5).try_draw()
        }),
        ("par(usr = c(0, 0, 0, 1))", "par", |g| {
            g.par().usr([0, 0, 0, 1]).try_draw()
        }),
        ("barplot(1:3, names.arg = c('a', 'b'))", "barplot", |g| {
            g.barplot()
                .height([1, 2, 3])
                .names(["a", "b"])
                .try_draw()
                .map(drop)
        }),
        ("barplot(1:3, log = 'y', density = 10)", "barplot", |g| {
            g.barplot()
                .height([1, 2, 3])
                .ylog(true)
                .density(10)
                .try_draw()
                .map(drop)
        }),
        ("barplot(c(1, -1), log = 'y')", "barplot", |g| {
            g.barplot().height([1, -1]).ylog(true).try_draw().map(drop)
        }),
        ("hist(1:10, breaks = c(2, 5))", "hist", |g| {
            g.hist().x(1..=10).breaks([2, 5]).try_draw().map(drop)
        }),
        ("hist(1:10, breaks = 0)", "hist", |g| {
            g.hist().x(1..=10).breaks(0).try_draw().map(drop)
        }),
        ("boxplot(1:5, at = 1:2)", "boxplot", |g| {
            g.boxplot().x(1..=5).at([1, 2]).try_draw().map(drop)
        }),
        (
            "boxplot(list(1:5, 2:6), width = c(1, -1))",
            "boxplot",
            |g| {
                g.boxplot()
                    .x(&[vec![1, 2, 3, 4, 5], vec![2, 3, 4, 5, 6]])
                    .width([1, -1])
                    .try_draw()
                    .map(drop)
            },
        ),
    ]
}

/// Errors only a device can see (they depend on its state, or are found
/// while drawing): R's code (after `plot(1:10)`), R's name for the call,
/// and the call.
type DeviceCall = fn(&mut Device<PdfBackend>) -> Result<(), Error>;

fn device_errors() -> Vec<(&'static str, &'static str, DeviceCall)> {
    vec![
        ("points(1, 1, col = -1)", "points", |d| {
            d.points().x(1).y(1).col(-1).try_draw()
        }),
        ("par(mfrow = c(0, 1))", "par", |d| {
            d.par().mfrow(0, 1).try_draw()
        }),
        ("par(mfcol = c(1, 0))", "par", |d| {
            d.par().mfcol(1, 0).try_draw()
        }),
        ("par(mfg = c(3, 1))", "par", |d| {
            d.par().mfg(3, 1).try_draw()
        }),
        ("par(mfg = c(1, 3))", "par", |d| {
            d.par().mfg(1, 3).try_draw()
        }),
        (
            "plot(1:100, log = 'x'); par(xaxp = c(1, 100, 7))",
            "par",
            |d| {
                d.plot().x(1..=100).xlog(true).draw();
                d.par().xaxp([1, 100, 7]).try_draw()
            },
        ),
        (
            "plot(1:100, log = 'x'); par(xaxp = c(1, 100, 0))",
            "par",
            |d| {
                d.plot().x(1..=100).xlog(true).draw();
                d.par().xaxp([1, 100, 0]).try_draw()
            },
        ),
    ]
}

fn device() -> Device<PdfBackend> {
    let mut dev = Device::new(PdfBackend::new(7, 7), Par::default());
    dev.plot().x(1..=10).draw();
    dev
}

/// R's message for each piece of code, run after `plot(1:10)` on a fresh
/// device.
fn r_messages(code: &[&str]) -> Vec<String> {
    let mut script = String::from(
        "e <- function(x) { pdf(NULL); plot(1:10); \
         m <- tryCatch({ x; 'ok' }, error = conditionMessage); \
         invisible(dev.off()); cat(m, '\\n', sep = '') }\n",
    );
    for c in code {
        script.push_str(&format!("e({{ {c} }})\n"));
    }
    let out = Command::new("Rscript")
        .args(["--vanilla", "-e", &script])
        .output()
        .expect("run Rscript");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let messages: Vec<String> = String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .map(str::to_string)
        .collect();
    assert_eq!(messages.len(), code.len());
    messages
}

fn check(what: &str, r: &str, name: &str, expected: &str, result: Result<(), Error>) {
    let e = result.expect_err(&format!("{what}: {r} should fail"));
    assert_eq!(
        (e.call(), e.kind(), e.message()),
        (name, ErrorKind::InvalidArgument, expected),
        "{what}: {r}"
    );
    assert_eq!(
        e.to_string(),
        format!("{name}(): {expected}"),
        "{what}: {r}"
    );
}

#[test]
fn errors_match_r() {
    if !common::r_available() {
        return;
    }
    let args = argument_errors::<Device<PdfBackend>>();
    let on_device = device_errors();
    let code: Vec<&str> = args
        .iter()
        .map(|c| c.0)
        .chain(on_device.iter().map(|c| c.0))
        .collect();
    let expected = r_messages(&code);
    let (expected_args, expected_device) = expected.split_at(args.len());

    for ((r, name, call), m) in args.iter().zip(expected_args) {
        check("device", r, name, m, call(&mut device()));
    }
    for ((r, name, call), m) in argument_errors::<Figure>().iter().zip(expected_args) {
        check("figure", r, name, m, call(&mut Figure::new()));
    }
    for ((r, name, call), m) in on_device.iter().zip(expected_device) {
        check("device", r, name, m, call(&mut device()));
    }
}

/// `.draw()` panics with the error's text; `.try_draw()` returns the
/// call's value when it succeeds, and draws the same output.
#[test]
fn draw_and_try_draw_agree() {
    let mut dev = device();
    let msg = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            dev.pie().x([1, -1]).draw();
        }));
        std::panic::set_hook(hook);
        r
    }))
    .unwrap()
    .unwrap_err();
    assert_eq!(
        msg.downcast_ref::<String>().map(String::as_str),
        Some("pie(): 'x' values must be positive.")
    );

    let draw = |try_: bool| {
        let mut dev = device();
        let at = if try_ {
            dev.axis().side(3).try_draw().unwrap()
        } else {
            dev.axis().side(3).draw()
        };
        (at, dev.finish())
    };
    let (at, out) = draw(true);
    assert_eq!(at, [2.0, 4.0, 6.0, 8.0, 10.0]);
    assert_eq!((at, out), draw(false));
}

/// An error found while drawing is returned by that call only: the next
/// call draws normally, and one inside another (`panel_first`) is returned
/// by the outer call, naming the inner one.
#[test]
fn errors_while_drawing_end_with_their_call() {
    let mut dev = device();
    assert!(dev.points().x(1).y(1).col(-1).try_draw().is_err());
    assert!(dev.points().x(1).y(1).col(2).try_draw().is_ok());

    let mut panel = Figure::new();
    panel.abline().h(5).col(-2).draw();
    let mut dev = device();
    let e = dev
        .plot()
        .x(1..=10)
        .panel_first(panel)
        .try_draw()
        .unwrap_err();
    assert_eq!(
        e.to_string(),
        "abline(): numerical color values must be >= 0, found -2"
    );
    assert!(dev.points().x(1).y(1).try_draw().is_ok());
}

/// The error type works with `?` in a function returning a boxed error.
#[test]
fn errors_work_with_the_question_mark() {
    fn plot_it(dev: &mut Device<PdfBackend>) -> Result<(), Box<dyn std::error::Error>> {
        dev.plot().y([1, 2]).try_draw()?;
        Ok(())
    }
    let e = plot_it(&mut device()).unwrap_err();
    assert_eq!(
        e.to_string(),
        "plot(): argument \"x\" is missing, with no default"
    );
}

/// R's errors for invalid `group()`/`bgroup()` delimiters (found when
/// drawing in R) are parse errors here, so `Label::math` panics and
/// `Expression::parse` returns them.
#[test]
fn plotmath_delimiters_are_checked_when_parsed() {
    assert!(Expression::parse("group(lfloor, x, rfloor)").is_ok());
    assert!(Expression::parse("bgroup('(', frac(a, b), ']')").is_ok());
    let message = |s: &str| Expression::parse(s).unwrap_err().to_string();
    assert!(message("group('<', x, '>')").contains("invalid group delimiter"));
    assert!(message("bgroup(lfloor, x, rfloor)").contains("group is incomplete"));
}

/// On a log axis, an `axp` code that `CreateAtVector()` can't use is
/// `axis()`'s error (R: "INVALID xaxp[3]"), not a panic.
#[test]
fn log_axis_with_an_invalid_axp_code_is_an_error() {
    let mut dev = device();
    dev.plot().x(1..=100).xlog(true).axes(false).draw();
    let e = dev.axis().side(1).axp([1, 100, 7]).try_draw().unwrap_err();
    assert_eq!(
        e.to_string(),
        "axis(): log - axis(), 'at' creation: INVALID xaxp[3] = 7"
    );
    assert!(dev.axis().side(1).axp([1, 100, 3]).try_draw().is_ok());
    assert!(dev.axis().side(1).axp([1, 100, -4]).try_draw().is_ok());
}

/// A negative palette number as a device's `bg` fails at `open()`, before
/// the file is created.
#[test]
fn negative_bg_fails_when_opening() {
    let path = std::env::temp_dir().join("rs_graphics_errors_matches_r_bg.png");
    let _ = std::fs::remove_file(&path);
    let e = Device::png(path.to_str().unwrap())
        .bg(-1)
        .open()
        .err()
        .unwrap();
    assert_eq!(e.kind(), std::io::ErrorKind::InvalidInput);
    assert!(!path.exists());
}
