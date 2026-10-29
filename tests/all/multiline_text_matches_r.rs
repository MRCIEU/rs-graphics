// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks text with newlines against a real R `pdf()` device's
//! uncompressed output: R's `GEText()` draws each line on its own, one
//! line height apart, and `strwidth()`/`strheight()` measure every line,
//! which moves legend rows and boxes and `axis()`'s overlap check. Every
//! painted path and text item in order (see `common::pdf::assert_same`)
//! for `text()` (`adj`, `srt`, `cex`, `par(lheight)`), `mtext()`,
//! `title()`, `legend()` labels and title, `axis()` labels and
//! `barplot()` names. `mtext()` cases don't compare clips (R's `C_mtext`
//! clips to the device, a bug not copied). Skipped if `Rscript` isn't
//! available.

use crate::common;
use crate::common::{pdf, r};

use rs_graphics::prelude::*;

const PAGE: (f64, f64) = (7.0, 7.0);

struct Case {
    name: &'static str,
    r: &'static str,
    ours: fn(&mut Device<AnyBackend>),
    clip: bool,
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "text",
            r: "plot.new()\n\
                text(0.5, 0.5, 'ab\\nlonger line')\n\
                text(0.2, 0.8, 'one\\ntwo\\nthree', adj = c(0, 1))\n\
                text(0.8, 0.3, 'rot\\nated', srt = 30, cex = 1.5)\n\
                text(0.3, 0.2, 'x\\ny', adj = 1)\n\
                text(0.6, 0.9, 'first\\n\\nthird', font = 2)",
            ours: |d| {
                d.plot_new();
                d.text().x(0.5).y(0.5).labels("ab\nlonger line").draw();
                d.text()
                    .x(0.2)
                    .y(0.8)
                    .labels("one\ntwo\nthree")
                    .adj(0)
                    .vadj(1)
                    .draw();
                d.text()
                    .x(0.8)
                    .y(0.3)
                    .labels("rot\nated")
                    .srt(30)
                    .cex(1.5)
                    .draw();
                d.text().x(0.3).y(0.2).labels("x\ny").adj(1).draw();
                d.text()
                    .x(0.6)
                    .y(0.9)
                    .labels("first\n\nthird")
                    .font(2)
                    .draw();
            },
            clip: true,
        },
        Case {
            name: "lheight",
            r: "plot.new(); par(lheight = 1.5, cex = 0.8)\n\
                text(0.5, 0.5, 'one\\ntwo')",
            ours: |d| {
                d.plot_new();
                d.par().lheight(1.5).cex(0.8).draw();
                d.text().x(0.5).y(0.5).labels("one\ntwo").draw();
            },
            clip: true,
        },
        Case {
            name: "mtext",
            r: "plot.new()\n\
                mtext('top\\nsecond', 3)\n\
                mtext('left\\nside', 2, las = 1)\n\
                mtext('right\\nside', 4)",
            ours: |d| {
                d.plot_new();
                d.mtext().text("top\nsecond").side(3).draw();
                d.mtext().text("left\nside").side(2).las(1).draw();
                d.mtext().text("right\nside").side(4).draw();
            },
            clip: false,
        },
        Case {
            name: "title",
            r: "plot.new(); title(main = 'Main\\nTitle', xlab = 'x\\nlabel', ylab = 'y\\nlabel')",
            ours: |d| {
                d.plot_new();
                d.title()
                    .main("Main\nTitle")
                    .xlab("x\nlabel")
                    .ylab("y\nlabel")
                    .draw();
            },
            clip: true,
        },
        Case {
            name: "legend",
            r: "plot.new()\n\
                legend('topleft', c('one', 'two\\nlines', 'three'), pch = 1)\n\
                legend('bottomright', c('a', 'b'), lty = 1, title = 'A\\nlong title here')",
            ours: |d| {
                d.plot_new();
                d.legend()
                    .pos("topleft")
                    .labels(["one", "two\nlines", "three"])
                    .pch(1)
                    .draw();
                d.legend()
                    .pos("bottomright")
                    .labels(["a", "b"])
                    .lty(1)
                    .title("A\nlong title here")
                    .draw();
            },
            clip: true,
        },
        Case {
            // `gap.axis` sees every line: side 2's "s" is dropped because of
            // the three-line label's height (perpendicular labels), and side
            // 3's "x" is kept because the width is the widest line's, not
            // the whole string's.
            name: "axis",
            r: "plot.new()\n\
                axis(1, at = c(0.2, 0.8), labels = c('a\\nb', 'c'))\n\
                axis(2, at = c(0.2, 0.25, 0.8), labels = c('p\\nq\\nr', 's', 't'), las = 1)\n\
                axis(3, at = c(0.3, 0.38), labels = c('wide\\nwide', 'x'))",
            ours: |d| {
                d.plot_new();
                d.axis().side(1).at([0.2, 0.8]).labels(["a\nb", "c"]).draw();
                d.axis()
                    .side(2)
                    .at([0.2, 0.25, 0.8])
                    .labels(["p\nq\nr", "s", "t"])
                    .las(1)
                    .draw();
                d.axis()
                    .side(3)
                    .at([0.3, 0.38])
                    .labels(["wide\nwide", "x"])
                    .draw();
            },
            clip: true,
        },
        Case {
            name: "barplot",
            r: "barplot(1:3, names.arg = c('a', 'b\\nc', 'd'))",
            ours: |d| {
                d.barplot()
                    .height([1, 2, 3])
                    .names(["a", "b\nc", "d"])
                    .draw();
            },
            clip: true,
        },
    ]
}

#[test]
fn multiline_text_matches_r() {
    if !common::r_available() {
        return;
    }
    let cases = cases();
    let r_cases: Vec<_> = cases.iter().map(|c| (c.name, c.r)).collect();
    let r = r::run_r("multiline", PAGE, "", &r_cases);
    for (c, r_pdf) in cases.iter().zip(&r.pdfs) {
        let (ours, ()) = r::ours(PAGE, c.ours);
        let how = pdf::Compare {
            clip: c.clip,
            ..pdf::Compare::default()
        };
        pdf::assert_same_pdf(c.name, pdf::parse(&ours), pdf::parse(r_pdf), &how);
    }
}
