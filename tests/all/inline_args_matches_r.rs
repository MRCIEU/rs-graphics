// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks inline arguments against a real R `pdf()` device's
//! uncompressed output, on a two-panel page where the plot region, the
//! figure and the device differ: `box(fg = , col = NA)`, inline `xpd` on
//! `title()` (`mtext()`'s, which R ignores, is checked on its own), inline
//! `xpd`/`lend` on `axis()`, `lwd` on `hist()`/`barplot()` axes with
//! shaded bars, `pie()`'s `border`/`lty`/`density`/`angle`, the `font` of
//! character symbols (and `par(font)` restored after), and `plot()`'s
//! inline `fg`/`adj`/`lend`/`xpd`/`font`; and `par(usr = )` (also on a log
//! axis), `par(xlog = )` (alone and with `usr`), `par(ylbias = )`, and
//! `NA` labels in `text()`, `mtext()` and `legend()`.
//! Compares every painted path and text item in order, with the clip
//! rectangle in effect for each (see `common::pdf::assert_same`). Skipped
//! if `Rscript` isn't available.

use crate::common;
use crate::common::{pdf, r};

use rs_graphics::prelude::*;

const PAGE_W: f64 = 8.0;
const PAGE_H: f64 = 5.0;

const X: [f64; 5] = [1.0, 2.0, 3.0, 4.0, 5.0];
const Y: [f64; 5] = [2.0, 4.0, 3.0, 5.0, 1.0];

type Draw = fn(&mut Device<PdfBackend>);

struct Case {
    name: &'static str,
    /// R code; `x` and `y` are the vectors above.
    r: &'static str,
    ours: Draw,
}

/// Two panels, the first one current, with `plot.window(c(0, 10), c(0, 10))`.
fn panels(d: &mut Device<PdfBackend>) {
    d.par().mfrow(1, 2).draw();
    d.plot_new();
    d.plot_window().xlim(0, 10).ylim(0, 10).draw();
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            // `col` wins unless `NA`; then `fg`; then `par(col)`.
            name: "box_fg",
            r: "par(mfrow = c(1, 2)); plot.new(); plot.window(c(0, 10), c(0, 10))\n\
                par(col = 'grey40')\n\
                box(fg = 'red', lwd = 3)\n\
                box(which = 'figure', col = NA)\n\
                box(which = 'inner', col = 'blue', fg = 'green')\n\
                box(which = 'outer', col = NA, fg = 'orange', lty = 2)",
            ours: |d| {
                panels(d);
                d.par().col("grey40").draw();
                d.box_().fg("red").lwd(3).draw();
                d.box_().which(BoxWhich::Figure).col(ColorSpec::None).draw();
                d.box_()
                    .which(BoxWhich::Inner)
                    .col("blue")
                    .fg("green")
                    .draw();
                d.box_()
                    .which(BoxWhich::Outer)
                    .col(ColorSpec::None)
                    .fg("orange")
                    .lty(2)
                    .draw();
            },
        },
        // Titles clip to at least the figure region; `NA` to the device.
        // One call per device: R caches the clip across calls with inline
        // `xpd` (`GClip()`'s `oldxpd`, saved and restored around each
        // call), so a later call can keep a stale clip, an R bug. (R's
        // `mtext()` always clips to the device, another bug; see
        // `mtext_clips_to_the_figure_unless_xpd_is_na`.)
        Case {
            name: "title_xpd_na",
            r: "par(mfrow = c(1, 2)); plot.new(); plot.window(c(0, 10), c(0, 10))\ntitle(main = 'mmm', xpd = NA)",
            ours: |d| {
                panels(d);
                d.title().main("mmm").xpd(Xpd::Device).draw();
            },
        },
        Case {
            name: "title_xpd_true",
            r: "par(mfrow = c(1, 2)); plot.new(); plot.window(c(0, 10), c(0, 10))\ntitle(sub = 'sss', xpd = TRUE)",
            ours: |d| {
                panels(d);
                d.title().sub("sss").xpd(true).draw();
            },
        },
        Case {
            name: "title_xpd_false",
            r: "par(mfrow = c(1, 2)); plot.new(); plot.window(c(0, 10), c(0, 10))\npar(xpd = NA); title(xlab = 'xxx', xpd = FALSE)",
            ours: |d| {
                panels(d);
                d.par().xpd(Xpd::Device).draw();
                d.title().xlab("xxx").xpd(false).draw();
            },
        },
        Case {
            // Inline `xpd` sets how far ticks reach; `lend` the line ends.
            name: "axis_xpd_lend",
            r: "par(mfrow = c(1, 2)); plot.new(); plot.window(c(0, 10), c(0, 10))\n\
                axis(1, at = c(-3, 0, 5, 10, 13), labels = c('a', 'b', 'c', 'd', 'e'), xpd = TRUE)\n\
                axis(3, at = c(-30, 5, 40), labels = c('p', 'q', 'r'), xpd = NA)\n\
                axis(2, lend = 'butt', lwd = 6)\n\
                par(xpd = TRUE); axis(4, at = c(-1, 5, 11), labels = c('u', 'v', 'w'), lend = 2, lwd = 6, xpd = FALSE)",
            ours: |d| {
                panels(d);
                d.axis()
                    .side(1)
                    .at([-3, 0, 5, 10, 13])
                    .labels(["a", "b", "c", "d", "e"])
                    .xpd(true)
                    .draw();
                d.axis()
                    .side(3)
                    .at([-30, 5, 40])
                    .labels(["p", "q", "r"])
                    .xpd(Xpd::Device)
                    .draw();
                d.axis().side(2).lend("butt").lwd(6).draw();
                d.par().xpd(true).draw();
                d.axis()
                    .side(4)
                    .at([-1, 5, 11])
                    .labels(["u", "v", "w"])
                    .lend(LineEnd::Square)
                    .lwd(6)
                    .xpd(false)
                    .draw();
            },
        },
        Case {
            // `lwd` reaches the axes only; shading in the bars.
            name: "hist_barplot",
            r: "par(mfrow = c(1, 2))\n\
                hist(c(1, 2, 2, 3, 3, 3, 4), lwd = 3, density = c(10, 20), angle = c(45, 135), \
                     col = 'red', ann = FALSE)\n\
                barplot(c(3, 1, 2), lwd = 2.5, axis.lty = 1, names.arg = c('a', 'b', 'c'))",
            ours: |d| {
                d.par().mfrow(1, 2).draw();
                d.hist()
                    .x([1, 2, 2, 3, 3, 3, 4])
                    .lwd(3)
                    .density([10, 20])
                    .angle([45, 135])
                    .col("red")
                    .ann(false)
                    .draw();
                d.barplot()
                    .height([3, 1, 2])
                    .lwd(2.5)
                    .axis_lty(1)
                    .names(["a", "b", "c"])
                    .draw();
            },
        },
        Case {
            name: "pie",
            r: "pie(c(1, 2, 3, 4), labels = c('a', 'b', 'c', 'd'), border = c('red', 'blue'), \
                lty = c(1, 2, 3), density = c(NA, 10, 20, -1), angle = c(0, 30))",
            ours: |d| {
                d.pie()
                    .x([1, 2, 3, 4])
                    .labels(["a", "b", "c", "d"])
                    .border(["red", "blue"])
                    .lty([1, 2, 3])
                    .density([None, Some(10), Some(20), Some(-1)])
                    .angle([0, 30])
                    .draw();
            },
        },
        Case {
            // `font` for character symbols, and `par(font)` after.
            name: "pch_font",
            r: "par(mfrow = c(1, 2)); plot.new(); plot.window(c(0, 10), c(0, 10))\n\
                points(c(2, 4, 6), c(2, 4, 6), pch = c('a', 'b', 'g'), font = 2, cex = 2)\n\
                points(c(2, 4), c(8, 8), pch = 'x', font = 3)\n\
                points(8, 8, pch = 'q')\n\
                par(font = 4); points(5, 1, pch = 'z')",
            ours: |d| {
                panels(d);
                d.points()
                    .x([2, 4, 6])
                    .y([2, 4, 6])
                    .pch(['a', 'b', 'g'])
                    .font(2)
                    .cex(2)
                    .draw();
                d.points().x([2, 4]).y([8, 8]).pch('x').font(3).draw();
                d.points().x([8]).y([8]).pch('q').draw();
                d.par().font(4).draw();
                d.points().x([5]).y([1]).pch('z').draw();
            },
        },
        Case {
            // `fg` colours the axes and frame, `adj` moves the titles,
            // `lend` the line ends, `xpd` the clip of points, ticks and
            // titles, `font` the symbols and axis labels.
            name: "plot_inline",
            r: "par(mfrow = c(1, 2))\n\
                plot(x, y, type = 'b', pch = 'o', ylim = c(2, 4), fg = 'red', adj = 0, \
                     lend = 'butt', lwd = 4, xpd = NA, font = 2, main = 'mmm', xlab = 'xxx', \
                     ylab = 'yyy')\n\
                plot(x, y, fg = 'blue', adj = 1, font = 3, xlab = 'xxx', ylab = 'yyy')",
            ours: |d| {
                d.par().mfrow(1, 2).draw();
                d.plot()
                    .x(X)
                    .y(Y)
                    .type_("b")
                    .pch('o')
                    .ylim(2, 4)
                    .fg("red")
                    .adj(0)
                    .lend("butt")
                    .lwd(4)
                    .xpd(Xpd::Device)
                    .font(2)
                    .main("mmm")
                    .xlab("xxx")
                    .ylab("yyy")
                    .draw();
                d.plot()
                    .x(X)
                    .y(Y)
                    .fg("blue")
                    .adj(1)
                    .font(3)
                    .xlab("xxx")
                    .ylab("yyy")
                    .draw();
            },
        },
        Case {
            // New coordinates, and the axis ticks worked out from them.
            name: "par_usr",
            r: "plot.new(); plot.window(c(0, 10), c(0, 10)); points(5, 5)\n\
                par(usr = c(0, 20, -5, 5)); points(5, 0); axis(1)\n\
                axis(2, labels = FALSE); abline(h = 0)",
            ours: |d| {
                d.plot_new();
                d.plot_window().xlim(0, 10).ylim(0, 10).draw();
                d.points().x([5]).y([5]).draw();
                d.par().usr([0, 20, -5, 5]).draw();
                d.points().x([5]).y([0]).draw();
                d.axis().side(1).draw();
                d.axis().side(2).draw_labels(false).draw();
                d.abline().h([0]).draw();
            },
        },
        Case {
            // On a log axis `usr` is in log10 units. (No axis: R works out
            // linear tick settings there, which its `axis()` rejects; see
            // `par_usr_on_a_log_axis_sets_log_ticks`.)
            name: "par_usr_log",
            r: "plot.new(); plot.window(c(1, 100), c(0, 1), log = 'x')\n\
                par(usr = c(0, 3, 0, 1)); points(c(10, 100), c(0.5, 0.5))",
            ours: |d| {
                d.plot_new();
                d.plot_window().xlim(1, 100).ylim(0, 1).xlog(true).draw();
                d.par().usr([0, 3, 0, 1]).draw();
                d.points().x([10, 100]).y([0.5, 0.5]).draw();
            },
        },
        Case {
            // `xlog` alone changes only the mapping; with `usr` in the same
            // call, `usr` is in log units.
            name: "par_xlog",
            r: "par(mfrow = c(1, 2))\n\
                plot.new(); plot.window(c(0, 3), c(0, 1)); par(xlog = TRUE)\n\
                points(c(1, 10, 100), c(0.5, 0.5, 0.5)); axis(2)\n\
                plot.new(); plot.window(c(0, 1), c(0, 1)); par(xlog = TRUE, usr = c(0, 2, 0, 1))\n\
                points(c(1, 10, 100), c(0.2, 0.4, 0.6))",
            ours: |d| {
                d.par().mfrow(1, 2).draw();
                d.plot_new();
                d.plot_window().xlim(0, 3).ylim(0, 1).draw();
                d.par().xlog(true).draw();
                d.points().x([1, 10, 100]).y([0.5, 0.5, 0.5]).draw();
                d.axis().side(2).draw();
                d.plot_new();
                d.plot_window().xlim(0, 1).ylim(0, 1).draw();
                d.par().xlog(true).usr([0, 2, 0, 1]).draw();
                d.points().x([1, 10, 100]).y([0.2, 0.4, 0.6]).draw();
            },
        },
        Case {
            name: "par_ylbias",
            r: "par(mfrow = c(1, 2), ylbias = 0.5)\n\
                plot.new(); plot.window(c(0, 10), c(0, 10))\n\
                title(xlab = 'xx', ylab = 'yy'); axis(1); axis(4); axis(3)\n\
                par(ylbias = -0.1); plot.new(); plot.window(c(0, 10), c(0, 10))\n\
                title(xlab = 'xx', ylab = 'yy'); axis(1); axis(3)",
            ours: |d| {
                d.par().mfrow(1, 2).ylbias(0.5).draw();
                d.plot_new();
                d.plot_window().xlim(0, 10).ylim(0, 10).draw();
                d.title().xlab("xx").ylab("yy").draw();
                d.axis().side(1).draw();
                d.axis().side(4).draw();
                d.axis().side(3).draw();
                d.par().ylbias(-0.1).draw();
                d.plot_new();
                d.plot_window().xlim(0, 10).ylim(0, 10).draw();
                d.title().xlab("xx").ylab("yy").draw();
                d.axis().side(1).draw();
                d.axis().side(3).draw();
            },
        },
        Case {
            // `NA` labels: skipped by `text()` (the others keep their index
            // for recycled styles) and `mtext()`; in `legend()` the key is
            // drawn and the label takes no room.
            name: "na_labels",
            r: "plot.new(); plot.window(c(0, 10), c(0, 10))\n\
                text(c(2, 4, 6, 8), 8, c('a', NA, 'c', NA), col = 1:4)\n\
                mtext(c('m', NA, 'o'), side = 1, outer = TRUE, line = -1, at = c(0.2, 0.5, 0.8))\n\
                legend('bottomleft', legend = c('a', NA, 'c'), pch = 1:3)",
            ours: |d| {
                d.plot_new();
                d.plot_window().xlim(0, 10).ylim(0, 10).draw();
                d.text()
                    .x([2, 4, 6, 8])
                    .y([8])
                    .labels([Some("a"), None, Some("c"), None])
                    .col([1, 2, 3, 4])
                    .draw();
                d.mtext()
                    .text([Some("m"), None, Some("o")])
                    .side(1)
                    .outer(true)
                    .line(-1)
                    .at([0.2, 0.5, 0.8])
                    .draw();
                d.legend()
                    .pos("bottomleft")
                    .labels([Some("a"), None, Some("c")])
                    .pch([1, 2, 3])
                    .draw();
            },
        },
    ]
}

// -------------------------------------------------------------------
// A minimal PDF content-stream interpreter: painted paths, and text with
// font, size and fill colour.
// -------------------------------------------------------------------

#[test]
fn inline_args_match_r() {
    if !common::r_available() {
        return;
    }
    let cases = cases();
    let page = (PAGE_W, PAGE_H);
    let r_cases: Vec<_> = cases.iter().map(|c| (c.name, c.r)).collect();
    let prelude = format!("x <- {}\ny <- {}", r::r_vec(&X), r::r_vec(&Y));
    let r = r::run_r("inline_args", page, &prelude, &r_cases);
    for (c, r_pdf) in cases.iter().zip(&r.pdfs) {
        let (ours_pdf, ()) = r::ours(page, c.ours);
        let ours = pdf::parse(&ours_pdf);
        let theirs = pdf::parse(r_pdf);
        pdf::assert_same_pdf(c.name, ours, theirs, &pdf::Compare::default());
    }
}

/// `mtext()` clips to the figure region, or to the device with `xpd = NA`
/// (inline or `par()`), and with `outer = TRUE`. R means to do this, but
/// its `C_mtext` tests `if (outer)` on the argument vector rather than its
/// value, so it always clips to the device (an R bug, not copied).
#[test]
fn mtext_clips_to_the_figure_unless_xpd_is_na() {
    let clip_of = |draw: Draw| {
        let mut dev = Device::new(
            PdfBackend::new(PAGE_W, PAGE_H).with_compression(false),
            Par::default(),
        );
        panels(&mut dev);
        draw(&mut dev);
        let texts = pdf::parse(&dev.into_backend().finish()).page().texts;
        assert_eq!(texts.len(), 1);
        texts[0].clip
    };
    let figure = [0.0, 0.0, PAGE_W * 72.0 / 2.0, PAGE_H * 72.0];
    let device = [0.0, 0.0, PAGE_W * 72.0, PAGE_H * 72.0];
    let cases: [(Draw, [f64; 4]); 5] = [
        (|d| d.mtext().text("a").side(4).draw(), figure),
        (|d| d.mtext().text("a").side(4).xpd(false).draw(), figure),
        (
            |d| d.mtext().text("a").side(4).xpd(Xpd::Device).draw(),
            device,
        ),
        (
            |d| {
                d.par().xpd(Xpd::Device).draw();
                d.mtext().text("a").side(4).draw();
            },
            device,
        ),
        (|d| d.mtext().text("a").side(1).outer(true).draw(), device),
    ];
    for (k, (draw, want)) in cases.into_iter().enumerate() {
        assert!(pdf::close(&clip_of(draw), &want, 0.01), "case {k}");
    }
}

/// `par(usr = )` with an empty range or `NA` fails with R's message, on a
/// device and when a `Figure` records the call.
#[test]
fn par_usr_is_checked() {
    for usr in [
        [1.0, 1.0, 0.0, 1.0],
        [0.0, 1.0, 2.0, 2.0],
        [f64::NAN, 1.0, 0.0, 1.0],
    ] {
        for figure in [false, true] {
            let r = std::panic::catch_unwind(|| {
                if figure {
                    Figure::new().par().usr(usr).draw();
                } else {
                    let mut dev = Device::new(PdfBackend::new(5, 5), Par::default());
                    dev.par().usr(usr).draw();
                }
            });
            let e = r.unwrap_err();
            let msg = e
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_default();
            assert!(
                msg.contains("graphical parameter \"usr\""),
                "{usr:?}: {msg}"
            );
        }
    }
}

/// `par(usr = )` on a log axis sets the tick settings a log axis gets from
/// `plot.window()` over the same range. (R's `GSetupAxis()` works out
/// linear ones there, which its own `axis()` then rejects: an R bug, not
/// copied.)
#[test]
fn par_usr_on_a_log_axis_sets_log_ticks() {
    let mut dev = Device::new(PdfBackend::new(5, 5), Par::default());
    dev.plot_new();
    dev.plot_window().xlim(1, 100).ylim(0, 1).xlog(true).draw();
    dev.par().usr([0, 3, 0, 1]).draw();
    let got = dev.current_par().xaxp;
    let mut want = Device::new(PdfBackend::new(5, 5), Par::default());
    want.par().xaxs("i").draw();
    want.plot_new();
    want.plot_window()
        .xlim(1, 1000)
        .ylim(0, 1)
        .xlog(true)
        .draw();
    assert_eq!(got, want.current_par().xaxp);
    assert_eq!(dev.usr(), (0.0, 3.0, 0.0, 1.0));
    // And so the same ticks.
    assert_eq!(dev.axis().side(1).draw(), want.axis().side(1).draw());
}
