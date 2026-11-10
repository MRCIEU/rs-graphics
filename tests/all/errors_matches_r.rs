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

use rs_graphics::Error;
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
        // `plot.window()`'s finite limits, from the data's range or the
        // given limits, whichever call made the window.
        ("plot(numeric(0))", "plot.window", |g| {
            g.plot().x([0.0; 0]).try_draw()
        }),
        ("plot(c(NA_real_, NA_real_))", "plot.window", |g| {
            g.plot().x([f64::NAN, f64::NAN]).try_draw()
        }),
        ("plot(1:2, xlim = c(NA, 1))", "plot.window", |g| {
            g.plot().x([1, 2]).xlim([f64::NAN, 1.0]).try_draw()
        }),
        ("plot(1:2, ylim = c(0, Inf))", "plot.window", |g| {
            g.plot().x([1, 2]).ylim([0.0, f64::INFINITY]).try_draw()
        }),
        ("plot.window(c(NA, 1), c(0, 1))", "plot.window", |g| {
            g.plot_window()
                .xlim([f64::NAN, 1.0])
                .ylim([0, 1])
                .try_draw()
        }),
        ("plot.window(c(0, 1), c(0, NA))", "plot.window", |g| {
            g.plot_window()
                .xlim([0, 1])
                .ylim([0.0, f64::NAN])
                .try_draw()
        }),
        ("hist(numeric(0))", "hist", |g| {
            g.hist().x([0.0; 0]).try_draw().map(drop)
        }),
        ("hist(c(NA_real_, NA_real_))", "hist", |g| {
            g.hist().x([f64::NAN, f64::NAN]).try_draw().map(drop)
        }),
        ("hist(1:3, ylim = c(0, NA))", "plot.window", |g| {
            g.hist()
                .x([1, 2, 3])
                .ylim([0.0, f64::NAN])
                .try_draw()
                .map(drop)
        }),
        ("barplot(c(NA_real_, NA_real_))", "plot.window", |g| {
            g.barplot()
                .height([f64::NAN, f64::NAN])
                .try_draw()
                .map(drop)
        }),
        ("barplot(c(1, Inf))", "plot.window", |g| {
            g.barplot()
                .height([1.0, f64::INFINITY])
                .try_draw()
                .map(drop)
        }),
        ("barplot(1:2, space = NA)", "plot.window", |g| {
            g.barplot()
                .height([1, 2])
                .space(f64::NAN)
                .try_draw()
                .map(drop)
        }),
        (
            "barplot(c(1, 2), horiz = TRUE, xlim = c(0, NA))",
            "plot.window",
            |g| {
                g.barplot()
                    .height([1, 2])
                    .horiz(true)
                    .xlim([0.0, f64::NAN])
                    .try_draw()
                    .map(drop)
            },
        ),
        ("boxplot(numeric(0))", "plot.window", |g| {
            g.boxplot().x([0.0; 0]).try_draw().map(drop)
        }),
        ("boxplot(c(NA_real_, NA_real_))", "plot.window", |g| {
            g.boxplot().x([f64::NAN, f64::NAN]).try_draw().map(drop)
        }),
        (
            "boxplot(1:3, horizontal = TRUE, ylim = c(NA, 1))",
            "plot.window",
            |g| {
                g.boxplot()
                    .x([1, 2, 3])
                    .horizontal(true)
                    .ylim([f64::NAN, 1.0])
                    .try_draw()
                    .map(drop)
            },
        ),
        // `par()`'s value checks (R's `par-common.c` and `par.c`).
        ("par(adj = 1.5)", "par", |g| g.par().adj(1.5).try_draw()),
        ("par(adj = NA)", "par", |g| g.par().adj(f64::NAN).try_draw()),
        ("par(cex = 0)", "par", |g| g.par().cex(0).try_draw()),
        ("par(cex = Inf)", "par", |g| {
            g.par().cex(f64::INFINITY).try_draw()
        }),
        ("par(cex.axis = -1)", "par", |g| {
            g.par().cex_axis(-1).try_draw()
        }),
        ("par(cex.lab = NA)", "par", |g| {
            g.par().cex_lab(f64::NAN).try_draw()
        }),
        ("par(cex.main = 0)", "par", |g| {
            g.par().cex_main(0).try_draw()
        }),
        ("par(cex.sub = 0)", "par", |g| g.par().cex_sub(0).try_draw()),
        ("par(lwd = 0)", "par", |g| g.par().lwd(0).try_draw()),
        ("par(mex = -1)", "par", |g| g.par().mex(-1).try_draw()),
        ("par(lheight = 0)", "par", |g| g.par().lheight(0).try_draw()),
        ("par(mar = c(-1, 1, 1, 1))", "par", |g| {
            g.par().mar([-1, 1, 1, 1]).try_draw()
        }),
        ("par(mai = c(1, NA, 1, 1))", "par", |g| {
            g.par().mai([1.0, f64::NAN, 1.0, 1.0]).try_draw()
        }),
        ("par(oma = c(1, 1, Inf, 1))", "par", |g| {
            g.par().oma([1.0, 1.0, f64::INFINITY, 1.0]).try_draw()
        }),
        ("par(omi = c(1, 1, 1, -1))", "par", |g| {
            g.par().omi([1, 1, 1, -1]).try_draw()
        }),
        ("par(plt = c(-0.1, 1, 0, 1))", "par", |g| {
            g.par().plt([-0.1, 1.0, 0.0, 1.0]).try_draw()
        }),
        ("par(pin = c(NA, 1))", "par", |g| {
            g.par().pin([f64::NAN, 1.0]).try_draw()
        }),
        ("par(omd = c(0, 1.5, 0, 1))", "par", |g| {
            g.par().omd([0.0, 1.5, 0.0, 1.0]).try_draw()
        }),
        ("par(mgp = c(3, NA, 0))", "par", |g| {
            g.par().mgp([3.0, f64::NAN, 0.0]).try_draw()
        }),
        ("par(srt = Inf)", "par", |g| {
            g.par().srt(f64::INFINITY).try_draw()
        }),
        ("par(las = 4)", "par", |g| g.par().las(4).try_draw()),
        ("par(las = -1)", "par", |g| g.par().las(-1).try_draw()),
        ("par(ps = -1)", "par", |g| g.par().ps(-1).try_draw()),
        ("par(xaxp = c(NA, 2, 5))", "par", |g| {
            g.par().xaxp([f64::NAN, 2.0, 5.0]).try_draw()
        }),
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
        // `log`'s check is `plot.window()`'s, whichever call it came from.
        ("plot(1:10, log = 'z')", "plot.window", |g| {
            g.plot().x(1..=10).log("z").try_draw()
        }),
        ("hist(c(1, 2, 2), log = 'X')", "plot.window", |g| {
            g.hist().x([1, 2, 2]).log("X").try_draw().map(drop)
        }),
        ("barplot(1:3, log = 'yxq')", "plot.window", |g| {
            g.barplot().height(1..=3).log("yxq").try_draw().map(drop)
        }),
        ("boxplot(1:3, log = ' ')", "plot.window", |g| {
            g.boxplot().x(1..=3).log(" ").try_draw().map(drop)
        }),
        (
            "plot.window(c(1, 10), c(1, 10), log = 'xyz')",
            "plot.window",
            |g| {
                g.plot_window()
                    .xlim([1, 10])
                    .ylim([1, 10])
                    .log("xyz")
                    .try_draw()
            },
        ),
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
        ("mtext('a', side = integer(0))", "mtext", |g| {
            g.mtext().text("a").side([0i32; 0]).try_draw()
        }),
        ("points(1, 1, lmitre = 0.5)", "points", |g| {
            g.points().x(1).y(1).lmitre(0.5).try_draw()
        }),
        ("par(lmitre = 0.5)", "par", |g| {
            g.par().lmitre(0.5).try_draw()
        }),
        ("par(pch = c(1, 2))", "par", |g| {
            g.par().pch([1, 2]).try_draw()
        }),
        ("par(pch = NA)", "par", |g| {
            g.par().pch(None::<i32>).try_draw()
        }),
        ("par(pch = '')", "par", |g| g.par().pch("").try_draw()),
        ("palette('red')", "par", |g| {
            g.par().palette("red").try_draw()
        }),
        ("palette(c('red', 'nonsense'))", "par", |g| {
            g.par().palette(["red", "nonsense"]).try_draw()
        }),
        ("palette(rep('red', 1025))", "par", |g| {
            g.par().palette(vec!["red"; 1025]).try_draw()
        }),
        ("par(usr = c(0, 0, 0, 1))", "par", |g| {
            g.par().usr([0, 0, 0, 1]).try_draw()
        }),
        ("barplot(1:3, names.arg = c('a', 'b'))", "barplot", |g| {
            g.barplot()
                .height([1, 2, 3])
                .names_arg(["a", "b"])
                .try_draw()
                .map(drop)
        }),
        ("barplot(1:3, log = 'y', density = 10)", "barplot", |g| {
            g.barplot()
                .height([1, 2, 3])
                .log("y")
                .density(10)
                .try_draw()
                .map(drop)
        }),
        ("barplot(c(1, -1), log = 'y')", "barplot", |g| {
            g.barplot().height([1, -1]).log("y").try_draw().map(drop)
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
        // Setters' values that don't convert: kept, returned by try_draw().
        ("points(1, 1, col = 'notacolour')", "points", |g| {
            g.points().x(1).y(1).col("notacolour").try_draw()
        }),
        ("axis(5)", "axis", |g| g.axis().side(5).try_draw().map(drop)),
        ("lines(1:2, 1:2, lty = -1)", "lines", |g| {
            g.lines().x([1, 2]).y([1, 2]).lty(-1).try_draw()
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
type DeviceCall = fn(&mut Device<AnyBackend>) -> Result<(), Error>;

fn device_errors() -> Vec<(&'static str, &'static str, DeviceCall)> {
    vec![
        ("points(1, 1, col = -1)", "points", |d| {
            d.points().x(1).y(1).col(-1).try_draw()
        }),
        ("par(mfrow = c(0, 1))", "par", |d| {
            d.par().mfrow([0, 1]).try_draw()
        }),
        ("par(mfcol = c(1, 0))", "par", |d| {
            d.par().mfcol([1, 0]).try_draw()
        }),
        ("par(mfrow = c(-1, 2))", "par", |d| {
            d.par().mfrow([-1, 2]).try_draw()
        }),
        ("par(mfg = c(0, 1))", "par", |d| {
            d.par().mfg([0, 1]).try_draw()
        }),
        ("par(mfg = c(3, 1))", "par", |d| {
            d.par().mfg([3, 1]).try_draw()
        }),
        ("par(mfg = c(1, 3))", "par", |d| {
            d.par().mfg([1, 3]).try_draw()
        }),
        // Beyond R's integer range: `as.integer()` gives `NA` (a warning),
        // which `par()` rejects. Our count would overflow instead.
        ("par(mfrow = c(1e300, 2))", "par", |d| {
            d.par().mfrow([1e300, 2.0]).try_draw()
        }),
        ("par(mfg = c(1e300, 1))", "par", |d| {
            d.par().mfg([1e300, 1.0]).try_draw()
        }),
        ("par(lab = c(0, 5, 7))", "par", |d| {
            d.par().lab([0, 5, 7]).try_draw()
        }),
        ("par(lab = c(5, 0.5, 7))", "par", |d| {
            d.par().lab([5.0, 0.5, 7.0]).try_draw()
        }),
        ("par(lab = c(5, 5, -1))", "par", |d| {
            d.par().lab([5, 5, -1]).try_draw()
        }),
        ("par(lab = c(5, 5, NA))", "par", |d| {
            d.par().lab([5.0, 5.0, f64::NAN]).try_draw()
        }),
        ("par(lab = c(1e300, 5, 7))", "par", |d| {
            d.par().lab([1e300, 5.0, 7.0]).try_draw()
        }),
        // The third value's check depends on the axis's log scale, which
        // only the device knows: a positive count on a linear axis.
        ("par(yaxp = c(1, 2, 0))", "par", |d| {
            d.par().yaxp([1, 2, 0]).try_draw()
        }),
        ("par(xaxp = c(1, 2, -1))", "par", |d| {
            d.par().xaxp([1, 2, -1]).try_draw()
        }),
        (
            "plot(1:100, log = 'x'); par(xaxp = c(1, 100, 7))",
            "par",
            |d| {
                d.plot().x(1..=100).log("x").draw();
                d.par().xaxp([1, 100, 7]).try_draw()
            },
        ),
        (
            "plot(1:100, log = 'x'); par(xaxp = c(1, 100, 0))",
            "par",
            |d| {
                d.plot().x(1..=100).log("x").draw();
                d.par().xaxp([1, 100, 0]).try_draw()
            },
        ),
        // `axis()` checks both directions' inline values, whichever side.
        (
            "plot(1:100, log = 'y'); axis(1, yaxp = c(1, 100, 7))",
            "axis",
            |d| {
                d.plot().x(1..=100).log("y").draw();
                d.axis().side(1).yaxp([1, 100, 7]).try_draw().map(|_| ())
            },
        ),
    ]
}

fn device() -> Device<AnyBackend> {
    let mut dev = Device::memory("pdf").width(7).height(7).open().unwrap();
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
    let args = argument_errors::<Device<AnyBackend>>();
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
    fn plot_it(dev: &mut Device<AnyBackend>) -> Result<(), Box<dyn std::error::Error>> {
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

/// On a log axis, an inline `xaxp` code above 4 is the graphical-parameter
/// error, and code 4, which `CreateAtVector()` can't use, is `axis()`'s
/// (R: "INVALID {xy}axp[3]", unformatted; ours names the axis); not
/// panics.
#[test]
fn log_axis_with_an_invalid_axp_code_is_an_error() {
    let mut dev = device();
    dev.plot().x(1..=100).log("x").axes(false).draw();
    let e = dev.axis().side(1).xaxp([1, 100, 7]).try_draw().unwrap_err();
    assert_eq!(
        e.to_string(),
        "axis(): invalid value specified for graphical parameter \"xaxp\""
    );
    let e = dev.axis().side(1).xaxp([1, 100, 4]).try_draw().unwrap_err();
    assert_eq!(
        e.to_string(),
        "axis(): log - axis(), 'at' creation: INVALID xaxp[3] = 4"
    );
    assert!(dev.axis().side(1).xaxp([1, 100, 3]).try_draw().is_ok());
    assert!(dev.axis().side(1).xaxp([1, 100, -4]).try_draw().is_ok());
}

/// A setter's value that doesn't convert is returned by `try_draw()`, also
/// through `.with(&opts)` and on the other builders; only the first is
/// kept, and the call isn't drawn.
#[test]
fn setter_errors_are_deferred_to_the_call() {
    let mut dev = device();
    let e = dev
        .lines()
        .x([1, 2])
        .y([1, 2])
        .type_("z")
        .col("notacolour")
        .try_draw()
        .unwrap_err();
    assert!(
        e.message().starts_with("\"z\" is not a valid plot type"),
        "{e}"
    );

    let opts = AxisOptions::default().font("heavy");
    let e = dev.axis().side(1).with(&opts).try_draw().unwrap_err();
    assert_eq!(e.call(), "axis");
    // `with` replaces the options, and with them errors set before it.
    let call = dev.axis().lwd(2).font(9).with(&AxisOptions::default());
    assert!(call.side(1).try_draw().is_ok());

    let e = dev
        .legend()
        .pos("middle")
        .labels("a")
        .try_draw()
        .unwrap_err();
    assert_eq!(e.call(), "legend");
    let e = dev.par().lty("wavy").try_draw().unwrap_err();
    assert_eq!(e.call(), "par");
    let e = dev.strwidth().s("a").units("feet").try_get().unwrap_err();
    assert_eq!(e.call(), "strwidth");

    // In a figure, the error is the recording call's.
    let mut fig = Figure::new();
    let e = fig.mtext().text("a").side(7).try_draw().unwrap_err();
    assert_eq!(e.to_string(), "mtext(): invalid axis number 7");
}

/// A setter's error means nothing is drawn: the output is as without the
/// call.
#[test]
fn a_call_with_a_setter_error_draws_nothing() {
    let mut with = device();
    assert!(
        with.points()
            .x(5)
            .y(5)
            .pch('x')
            .col("nocolour")
            .try_draw()
            .is_err()
    );
    assert_eq!(with.finish(), device().finish());
}

/// `ax_ticks()` is a query, not a call, so a bad side panics.
#[test]
#[should_panic(expected = "axTicks(): invalid axis number 5")]
fn ax_ticks_panics_on_a_bad_side() {
    let _ = device().ax_ticks(5);
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
    assert_eq!(e.kind(), ErrorKind::InvalidArgument);
    assert_eq!(e.call(), "png");
    assert!(!path.exists());
}
