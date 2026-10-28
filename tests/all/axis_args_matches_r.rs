// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks `axis()`'s arguments (`line`, `pos`, `outer`, `tck`/`tcl`,
//! `hadj`, `padj`, `gap.axis`, `tick`, `labels` (incl. `NA`), `lty`, `lwd`/`lwd.ticks`,
//! `col`/`col.ticks`/`col.axis`, `cex.axis`, `font`, `xaxt`/`yaxt`) and
//! its tick limits (reversed axes, `par(xpd = TRUE)`) against a real R `pdf()` device's
//! uncompressed output: every painted path and label in drawing order
//! (see `common::pdf::assert_same`). Skipped if `Rscript` isn't
//! available.

use crate::common;
use crate::common::{pdf, r};

use std::process::Command;

use rs_graphics::backend::pdf::PdfBackend;
use rs_graphics::color::{Color, ColorSpec};
use rs_graphics::device::{AxisOptions, Device};
use rs_graphics::lty::LineType;
use rs_graphics::plotmath::Label as AxisLabel;
use rs_graphics::par::{AxisType, Par, Sides};
use rs_graphics::prelude::*;

const PAGE_W: f64 = 7.0;
const PAGE_H: f64 = 6.0;

struct Case {
    name: &'static str,
    /// R code run after `plot.new()`/`plot.window()`.
    r: &'static str,
    /// `par()` settings for R, run before `plot.new()`.
    r_par: &'static str,
    xlim: (f64, f64),
    setup: fn(&mut Par),
    draw: fn(&mut Device<PdfBackend>),
}

fn col(name: &str) -> Option<ColorSpec> {
    Some(ColorSpec::Color(Color::parse(name).unwrap()))
}

/// `c(...)` of strings, `"NA"` being `NA`.
fn labels(l: &[&str]) -> Vec<Option<AxisLabel>> {
    l.iter().map(|s| (*s != "NA").then(|| AxisLabel::from(*s))).collect()
}

/// `c(rbind(NA, strrep(LETTERS[1:10], 3)), 'ZZZ')`.
fn alternating_na() -> Vec<Option<AxisLabel>> {
    let mut l: Vec<Option<AxisLabel>> = ('A'..='J')
        .flat_map(|c| [None, Some(AxisLabel::from(c.to_string().repeat(3)))])
        .collect();
    l.push(Some("ZZZ".into()));
    l
}

fn seq(from: f64, to: f64, by: f64) -> Vec<f64> {
    let n = ((to - from) / by).round() as usize;
    (0..=n).map(|i| from + i as f64 * by).collect()
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "line",
            r_par: "",
            r: "axis(1, line = 1); axis(2, line = -1); \
                axis(3, line = 2, las = 2); axis(4, line = 0.5, las = 1)",
            xlim: (0.0, 10.0),
            setup: |_| {},
            draw: |d| {
                d.axis()
                    .side(1)
                    .with(&AxisOptions::default().line(1.0))
                    .draw();
                d.axis()
                    .side(2)
                    .with(&AxisOptions::default().line(-1.0))
                    .draw();
                let o = AxisOptions::default().line(2.0).las(2);
                d.axis().side(3).with(&o).draw();
                let o = AxisOptions::default().line(0.5).las(1);
                d.axis().side(4).with(&o).draw();
            },
        },
        Case {
            name: "pos",
            r_par: "",
            r: "axis(1, pos = 2); axis(2, pos = 3, las = 1); \
                axis(3, pos = 8, line = 3); axis(4, pos = 12)",
            xlim: (0.0, 10.0),
            setup: |_| {},
            draw: |d| {
                d.axis()
                    .side(1)
                    .with(&AxisOptions::default().pos(2.0))
                    .draw();
                let o = AxisOptions::default().pos(3.0).las(1);
                d.axis().side(2).with(&o).draw();
                let o = AxisOptions::default().pos(8.0).line(3.0);
                d.axis().side(3).with(&o).draw();
                d.axis()
                    .side(4)
                    .with(&AxisOptions::default().pos(12.0))
                    .draw();
            },
        },
        Case {
            name: "outer",
            r_par: "par(oma = c(2, 3, 2, 1))",
            r: "axis(1, outer = TRUE); axis(2, outer = TRUE, line = 1); \
                axis(3, outer = TRUE, line = -1); axis(4, outer = TRUE, las = 2)",
            xlim: (0.0, 10.0),
            setup: |p| p.oma = Sides::new(2.0, 3.0, 2.0, 1.0),
            draw: |d| {
                d.axis()
                    .side(1)
                    .with(&AxisOptions::default().outer(true))
                    .draw();
                let o = AxisOptions::default().outer(true).line(1.0);
                d.axis().side(2).with(&o).draw();
                let o = AxisOptions::default().outer(true).line(-1.0);
                d.axis().side(3).with(&o).draw();
                let o = AxisOptions::default().outer(true).las(2);
                d.axis().side(4).with(&o).draw();
            },
        },
        Case {
            name: "tck",
            r_par: "",
            r: "axis(1, tck = 0.02); axis(2, tck = -0.03); \
                axis(3, tck = 1, col.ticks = 'grey'); axis(4, tcl = 0.4)",
            xlim: (0.0, 10.0),
            setup: |_| {},
            draw: |d| {
                d.axis()
                    .side(1)
                    .with(&AxisOptions::default().tck(0.02))
                    .draw();
                d.axis()
                    .side(2)
                    .with(&AxisOptions::default().tck(-0.03))
                    .draw();
                let o = AxisOptions::default().tck(1.0).col_ticks(col("grey"));
                d.axis().side(3).with(&o).draw();
                d.axis()
                    .side(4)
                    .with(&AxisOptions::default().tcl(0.4))
                    .draw();
            },
        },
        Case {
            name: "adj",
            r_par: "",
            r: "axis(1, hadj = 0, padj = c(0, 1)); \
                axis(2, las = 1, hadj = 0.5, padj = -0.5); \
                axis(3, las = 2, hadj = 0.2); axis(4, padj = c(1, NA, 0.5))",
            xlim: (0.0, 10.0),
            setup: |_| {},
            draw: |d| {
                let o = AxisOptions::default().hadj(0.0).padj(vec![0.0, 1.0]);
                d.axis().side(1).with(&o).draw();
                let o = AxisOptions::default().las(1).hadj(0.5).padj(vec![-0.5]);
                d.axis().side(2).with(&o).draw();
                let o = AxisOptions::default().las(2).hadj(0.2);
                d.axis().side(3).with(&o).draw();
                let o = AxisOptions::default().padj(vec![1.0, f64::NAN, 0.5]);
                d.axis().side(4).with(&o).draw();
            },
        },
        Case {
            name: "gap",
            r_par: "",
            r: "axis(1, at = seq(0, 10, 0.5), gap.axis = -0.5); \
                axis(3, at = seq(0, 10, 0.5), gap.axis = 2); \
                axis(2, at = seq(0, 10, 0.25), las = 1, gap.axis = 0); \
                axis(4, at = seq(0, 10, 0.25), las = 1, gap.axis = 3)",
            xlim: (0.0, 10.0),
            setup: |_| {},
            draw: |d| {
                let at = Some(seq(0.0, 10.0, 0.5));
                let o = AxisOptions::default().at(at.clone()).gap_axis(-0.5);
                d.axis().side(1).with(&o).draw();
                let o = AxisOptions::default().at(at).gap_axis(2.0);
                d.axis().side(3).with(&o).draw();
                let at = Some(seq(0.0, 10.0, 0.25));
                let o = AxisOptions::default().at(at.clone()).las(1).gap_axis(0.0);
                d.axis().side(2).with(&o).draw();
                let o = AxisOptions::default().at(at).las(1).gap_axis(3.0);
                d.axis().side(4).with(&o).draw();
            },
        },
        Case {
            name: "styles",
            r_par: "par(lwd = 3)",
            r: "axis(1, lwd = 2, lwd.ticks = 0.5, col = 'red', col.ticks = 'blue', \
                     col.axis = 'darkgreen', cex.axis = 0.8, font = 2); \
                axis(2, lty = 'dashed', labels = FALSE); \
                axis(3, tick = FALSE, at = c(2, 5, 7), labels = c('a', 'bb', 'ccc')); \
                axis(4, lwd = 0, lwd.ticks = 2)",
            xlim: (0.0, 10.0),
            setup: |p| p.lwd = 3.0,
            draw: |d| {
                let o = AxisOptions::default()
                    .lwd(2.0)
                    .lwd_ticks(0.5)
                    .col(col("red"))
                    .col_ticks(col("blue"))
                    .col_axis(col("darkgreen"))
                    .cex_axis(0.8)
                    .font(FontFace::Bold);
                d.axis().side(1).with(&o).draw();
                let o = AxisOptions::default()
                    .lty(LineType::from_code(2).unwrap())
                    .draw_labels(false);
                d.axis().side(2).with(&o).draw();
                let o = AxisOptions::default()
                    .tick(false)
                    .at(vec![2.0, 5.0, 7.0])
                    .labels(labels(&["a", "bb", "ccc"]));
                d.axis().side(3).with(&o).draw();
                let o = AxisOptions::default().lwd(0.0).lwd_ticks(2.0);
                d.axis().side(4).with(&o).draw();
            },
        },
        Case {
            name: "axt",
            r_par: "par(xaxt = 'n')",
            r: "axis(1); axis(3, xaxt = 's'); axis(2, yaxt = 'n'); axis(4)",
            xlim: (0.0, 10.0),
            setup: |p| p.xaxt = AxisType::None,
            draw: |d| {
                d.axis().side(1).draw();
                let o = AxisOptions::default().axt(AxisType::Standard);
                d.axis().side(3).with(&o).draw();
                let o = AxisOptions::default().axt(AxisType::None);
                d.axis().side(2).with(&o).draw();
                d.axis().side(4).draw();
            },
        },
        Case {
            name: "reversed_xpd",
            r_par: "",
            r: "axis(3); par(xpd = TRUE); axis(1, at = c(-1, 0, 5, 10, 11), labels = c('a', 'b', 'c', 'd', 'e')); \
                axis(2, at = c(-2, 5, 12), labels = c('lo', 'mid', 'hi'), las = 1); \
                axis(4, at = c(8, 2), labels = c('eight', 'two'))",
            xlim: (10.0, 0.0),
            setup: |_| {},
            draw: |d| {
                d.axis().side(3).draw();
                d.par().xpd(Xpd::Figure).draw();
                // Letters, not negative numbers: R's `pdf()` draws `-` as
                // the wider `/minus` glyph, which we deliberately don't.
                let o = AxisOptions::default()
                    .at(vec![-1.0, 0.0, 5.0, 10.0, 11.0])
                    .labels(labels(&["a", "b", "c", "d", "e"]));
                d.axis().side(1).with(&o).draw();
                let o = AxisOptions::default()
                    .at(vec![-2.0, 5.0, 12.0])
                    .labels(labels(&["lo", "mid", "hi"]))
                    .las(1);
                d.axis().side(2).with(&o).draw();
                let o = AxisOptions::default()
                    .at(vec![8.0, 2.0])
                    .labels(labels(&["eight", "two"]));
                d.axis().side(4).with(&o).draw();
            },
        },
        Case {
            name: "na_labels",
            r_par: "",
            r: "axis(1, at = 0:4 * 2.5, labels = c('a', NA, 'c', NA, 'e')); \
                axis(3, at = seq(0, 10, 0.5), labels = c(rbind(NA, strrep(LETTERS[1:10], 3)), 'ZZZ')); \
                axis(2, at = c(1, 5, 9), labels = c(NA, NA, NA))",
            xlim: (0.0, 10.0),
            setup: |_| {},
            draw: |d| {
                let o = AxisOptions::default()
                    .at(seq(0.0, 10.0, 2.5))
                    .labels(labels(&["a", "NA", "c", "NA", "e"]));
                d.axis().side(1).with(&o).draw();
                // NA between labels spaced so that `gap.axis` would drop
                // every one of them if the NAs took part.
                let o = AxisOptions::default()
                    .at(seq(0.0, 10.0, 0.5))
                    .labels(alternating_na());
                d.axis().side(3).with(&o).draw();
                let o = AxisOptions::default()
                    .at(vec![1.0, 5.0, 9.0])
                    .labels(labels(&["NA", "NA", "NA"]));
                d.axis().side(2).with(&o).draw();
            },
        },
        Case {
            name: "mgp",
            r_par: "par(mgp = c(2, 0.7, 0.2))",
            r: "axis(1, mgp = c(3, 0.3, 0)); axis(2); \
                axis(3, mgp = c(3, 1.5, 0.5), line = 1); axis(4, mgp = c(3, 0.2, -0.5), pos = 9, las = 1)",
            xlim: (0.0, 10.0),
            setup: |p| p.mgp = [2.0, 0.7, 0.2],
            draw: |d| {
                d.axis().side(1).mgp([3.0, 0.3, 0.0]).draw();
                // `par(mgp)` when not given inline.
                d.axis().side(2).draw();
                d.axis().side(3).mgp([3.0, 1.5, 0.5]).line(1).draw();
                d.axis().side(4).mgp([3.0, 0.2, -0.5]).pos(9).las(1).draw();
            },
        },
        Case {
            name: "mex_2",
            r_par: "par(mex = 2, mgp = c(2.5, 0.8, 0))",
            r: "axis(1); axis(2); axis(3, las = 2); axis(4, las = 1); \
                title(main = 'M', sub = 'S', xlab = 'X', ylab = 'Y'); \
                mtext('A', 3, line = 1); mtext('B', 4, line = 1)",
            xlim: (0.0, 10.0),
            setup: |p| {
                p.mex = 2.0;
                p.mgp = [2.5, 0.8, 0.0];
            },
            draw: |d| {
                d.axis().side(1).draw();
                d.axis().side(2).draw();
                d.axis().side(3).las(2).draw();
                d.axis().side(4).las(1).draw();
                d.title().main("M").sub("S").xlab("X").ylab("Y").draw();
                d.mtext().text("A").side(3).line(1).draw();
                d.mtext().text("B").side(4).line(1).draw();
            },
        },
        Case {
            name: "mex_0_7",
            r_par: "par(mex = 0.7)",
            r: "axis(1); axis(2); axis(3, las = 2); axis(4, las = 1); \
                title(main = 'M', sub = 'S', xlab = 'X', ylab = 'Y'); \
                mtext('A', 3, line = 1); mtext('B', 4, line = 1)",
            xlim: (0.0, 10.0),
            setup: |p| p.mex = 0.7,
            draw: |d| {
                d.axis().side(1).draw();
                d.axis().side(2).draw();
                d.axis().side(3).las(2).draw();
                d.axis().side(4).las(1).draw();
                d.title().main("M").sub("S").xlab("X").ylab("Y").draw();
                d.mtext().text("A").side(3).line(1).draw();
                d.mtext().text("B").side(4).line(1).draw();
            },
        },
        Case {
            name: "na_labels_reversed",
            r_par: "",
            r: "axis(1, at = seq(0, 10, 0.5), labels = c(rbind(NA, strrep(LETTERS[1:10], 3)), 'ZZZ'))",
            xlim: (10.0, 0.0),
            setup: |_| {},
            draw: |d| {
                let o = AxisOptions::default()
                    .at(seq(0.0, 10.0, 0.5))
                    .labels(alternating_na());
                d.axis().side(1).with(&o).draw();
            },
        },
    ]
}

fn r_code(c: &Case) -> String {
    format!(
        "{par}\n\
         plot.new(); plot.window(xlim = c({x0}, {x1}), ylim = c(0, 10))\n\
         {body}",
        par = c.r_par,
        x0 = c.xlim.0,
        x1 = c.xlim.1,
        body = c.r,
    )
}

fn draw_ours(c: &Case) -> Vec<u8> {
    let mut par = Par::default();
    (c.setup)(&mut par);
    let (pdf, ()) = r::ours_with((PAGE_W, PAGE_H), par, |dev| {
        dev.plot_new();
        dev.plot_window()
            .xlim(c.xlim.0, c.xlim.1)
            .ylim(0.0, 10.0)
            .draw();
        (c.draw)(dev);
    });
    pdf
}

// -------------------------------------------------------------------
// A minimal PDF content-stream interpreter (see `boxplot_draw_matches_r.rs`)
// -------------------------------------------------------------------

#[test]
fn axis_arguments_match_r() {
    if !common::r_available() {
        return;
    }
    let cases = cases();
    let codes: Vec<String> = cases.iter().map(r_code).collect();
    let r_cases: Vec<_> = cases
        .iter()
        .zip(&codes)
        .map(|(c, r)| (c.name, r.as_str()))
        .collect();
    let r = r::run_r("axis_args", (PAGE_W, PAGE_H), "", &r_cases);
    // R writes widths to 2 decimals.
    let how = pdf::Compare {
        lwd: 0.0051,
        ..Default::default()
    };
    for (c, r_pdf) in cases.iter().zip(&r.pdfs) {
        let ours_pdf = draw_ours(c);
        pdf::assert_same_pdf(c.name, pdf::parse(&ours_pdf), pdf::parse(r_pdf), &how);
    }
}

/// `axis()`'s return value: R's `at` (or default ticks) sorted with `NA`
/// last, `NULL` (empty here) for an empty `at` or `xaxt = "n"`, in data
/// units on a log axis; and the same values from `Device::add_figure`.
#[test]
fn axis_returns_tick_positions_like_r() {
    if !common::r_available() {
        return;
    }
    let r = Command::new("Rscript")
        .args([
            "--vanilla",
            "-e",
            "pdf(NULL); p <- function(v) cat(paste(v, collapse = ','), '\n', sep = '')\n\
             plot.new(); plot.window(c(0, 10), c(0, 10))\n\
             p(axis(1)); p(axis(2, at = c(Inf, 3, NA, -Inf, 1))); p(axis(3, at = c(12, 5, -3)))\n\
             p(axis(4, at = numeric(0))); p(axis(1, xaxt = 'n')); p(axis(2, labels = FALSE, tick = FALSE))\n\
             plot.new(); plot.window(c(1, 1000), c(1, 10), log = 'x')\n\
             p(axis(1)); p(axis(3, at = c(100, 5, 2000)))",
        ])
        .output()
        .expect("run Rscript");
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    let expected = String::from_utf8(r.stdout).unwrap();

    fn scene<G: Graphics>(g: &mut G) -> Vec<G::AxisResult> {
        g.plot_new();
        g.plot_window().xlim(0, 10).ylim(0, 10).draw();
        let mut out = vec![
            g.axis().side(1).draw(),
            g.axis()
                .side(2)
                // A negative NaN too (as `0.0 / 0.0` gives on x86), which a
                // plain `total_cmp` sorts first.
                .at([f64::INFINITY, -f64::NAN, 3.0, f64::NEG_INFINITY, 1.0])
                .draw(),
            g.axis().side(3).at([12, 5, -3]).draw(),
            g.axis().side(4).at(Vec::<f64>::new()).draw(),
            g.axis().side(1).axt("n").draw(),
            g.axis().side(2).draw_labels(false).tick(false).draw(),
        ];
        g.plot_new();
        g.plot_window().xlim(1, 1000).ylim(1, 10).xlog(true).draw();
        out.push(g.axis().side(1).draw());
        out.push(g.axis().side(3).at([100, 5, 2000]).draw());
        out
    }
    let show = |v: &[f64]| {
        v.iter()
            .map(|x| match x {
                x if x.is_nan() => "NA".to_string(),
                x if x.is_infinite() => if *x > 0.0 { "Inf" } else { "-Inf" }.to_string(),
                x => format!("{x}"),
            })
            .collect::<Vec<_>>()
            .join(",")
    };
    let device = || Device::new(PdfBackend::new(PAGE_W, PAGE_H), Par::default());
    let mut dev = device();
    let ours: String = scene(&mut dev).iter().map(|v| show(v) + "\n").collect();
    assert_eq!(ours, expected);

    let mut fig = Figure::new();
    scene(&mut fig);
    let replayed = device().add_figure(&fig);
    let from_figure: String = replayed.axes.iter().map(|v| show(v) + "\n").collect();
    assert_eq!(from_figure, expected);
}
