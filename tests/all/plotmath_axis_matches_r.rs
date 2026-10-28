// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks plotmath expression labels against a real R `pdf()`
//! device's uncompressed output: on `axis()` (all four sides, with
//! rotation, `cex.axis`, `hadj`/`padj` and `las`), in `title()`,
//! `mtext()`, `text()` and `legend()`, and as the titles of the high-level
//! plots on a 2x3 `mfrow` page. Compares every painted path and text item
//! in order (see `common::pdf::assert_same`); clips not where `mtext()` is
//! drawn (R clips it to the device), and the boxplot has no outliers (R's
//! are `par(cex)` times smaller on that page; we don't copy that). Skipped
//! if `Rscript` isn't available.
//!
//! Not compared: two or more digits in a name in a non-plain font. R's
//! `RenderSymbolStr` means to draw digits plain, but its font switch flips
//! back on every second consecutive digit (`bolditalic(x12)` draws "1"
//! plain and "2" bold italic); we draw them all plain.

use crate::common;
use crate::common::{pdf, r};

use rs_graphics::backend::pdf::PdfBackend;
use rs_graphics::device::{AxisOptions, Device};
use rs_graphics::par::Par;
use rs_graphics::plotmath::Label;
use rs_graphics::prelude::*;

const PAGE_W: f64 = 8.0;
const PAGE_H: f64 = 7.0;

struct Case {
    side: i32,
    exprs: &'static [&'static str],
    /// Extra R arguments to `axis()`.
    r_args: &'static str,
    opts: fn() -> AxisOptions,
}

const CASES: &[Case] = &[
    Case {
        side: 1,
        exprs: &[
            "10^5",
            "alpha[i]^2",
            "-log[10](italic(p))",
            "x[i]^2",
            "paste(\"P = \", 0.05)",
            "e^x^2",
        ],
        r_args: "",
        opts: AxisOptions::default,
    },
    Case {
        side: 2,
        exprs: &["beta", "10^-3", "a %+-% b", "x == y", "bold(x) ~ italic(y)"],
        r_args: "",
        opts: AxisOptions::default,
    },
    Case {
        side: 3,
        exprs: &[
            "a/b",
            "list(x, y, z)",
            "f(x, y)",
            "group(\"[\", x, \"]\")",
            "x * cdots * y",
        ],
        r_args: ", cex.axis = 1.5",
        opts: || AxisOptions {
            cex_axis: Some(1.5),
            ..AxisOptions::default()
        },
    },
    Case {
        side: 4,
        exprs: &[
            "Omega^2",
            "infinity",
            "2 %*% 10^4",
            "x %->% y",
            // One digit only: see `consecutive_digits_are_all_plain`.
            "bolditalic(abc1) <= plain(y)",
        ],
        r_args: ", las = 1, hadj = 0.3, padj = c(0, 1)",
        opts: || AxisOptions {
            las: Some(1),
            hadj: Some(0.3),
            padj: vec![0.0, 1.0],
            ..AxisOptions::default()
        },
    },
];

#[test]
fn plotmath_axis_labels_match_r() {
    if !common::r_available() {
        return;
    }
    let mut script =
        String::from("par(mar = c(5, 6, 5, 6)); plot.new(); plot.window(c(0, 6), c(0, 6))\n");
    for c in CASES {
        script.push_str(&format!(
            "axis({}, at = 1:{}, labels = expression({}){})\n",
            c.side,
            c.exprs.len(),
            c.exprs.join(", "),
            c.r_args
        ));
    }
    let r_pdf = r::run_r(
        "plotmath_axis",
        (PAGE_W, PAGE_H),
        "",
        &[("all", script.as_str())],
    )
    .pdfs
    .remove(0);

    let mut par = Par::default();
    par.mar = rs_graphics::par::Sides::new(5.0, 6.0, 5.0, 6.0);
    let mut dev = Device::new(PdfBackend::new(PAGE_W, PAGE_H).with_compression(false), par);
    dev.plot_new();
    dev.plot_window().xlim(0.0, 6.0).ylim(0.0, 6.0).draw();
    for c in CASES {
        let opts = AxisOptions {
            at: Some((1..=c.exprs.len()).map(|i| i as f64).collect()),
            labels: Some(c.exprs.iter().map(|e| Some(Label::math(e))).collect()),
            ..(c.opts)()
        };
        dev.axis().side(c.side).with(&opts).draw();
    }
    // Byte for byte: R writes Symbol codes above 127 as raw bytes.
    let ours_pdf = dev.into_backend().finish();
    pdf::assert_same_pdf(
        "plotmath_axis_labels_match_r",
        pdf::parse(&ours_pdf),
        pdf::parse(&r_pdf),
        &pdf::Compare::default(),
    );
}

#[test]
fn consecutive_digits_are_all_plain() {
    let mut dev = Device::new(
        PdfBackend::new(PAGE_W, PAGE_H).with_compression(false),
        Par::default(),
    );
    dev.plot_new();
    let opts = AxisOptions {
        at: Some(vec![0.5]),
        labels: Some(vec![Some(Label::math("bolditalic(x12)"))]),
        ..AxisOptions::default()
    };
    dev.axis().side(1).with(&opts).draw();
    let fonts: Vec<(String, Vec<u8>)> = pdf::parse(&dev.into_backend().finish())
        .page()
        .texts
        .into_iter()
        .map(|t| (t.font, t.string.chars().map(|c| c as u8).collect()))
        .collect();
    let want = [
        ("Helvetica-BoldOblique", "x"),
        ("Helvetica", "1"),
        ("Helvetica", "2"),
    ];
    let want: Vec<(String, Vec<u8>)> = want
        .iter()
        .map(|(f, t)| (f.to_string(), t.as_bytes().to_vec()))
        .collect();
    assert_eq!(fonts, want);
}

/// Expressions in `title()`, `mtext()` (inner, outer, perpendicular),
/// `text()` (default and two-element `adj`, `srt`, `cex`, `col`) and
/// `legend()` (labels, including a tall one, and title).
#[test]
fn plotmath_title_mtext_text_legend_match_r() {
    if !common::r_available() {
        return;
    }
    let script = String::from(
        "par(mar = c(5, 6, 5, 6), oma = c(2, 0, 2, 0))\n\
         plot.new(); plot.window(c(0, 10), c(0, 10))\n\
         title(main = expression(Main ~ alpha^2), sub = expression(sub[i]),\n\
               xlab = expression(x[1] + x[2]), ylab = expression(-log[10](italic(p))))\n\
         mtext(expression(beta^2), side = 3, line = 0.5, adj = 0)\n\
         mtext(expression(Omega ~ infinity), side = 4, line = 1)\n\
         mtext(expression(10^-5), side = 1, outer = TRUE, line = 0.5)\n\
         par(las = 1); mtext(expression(gamma[k]), side = 2, line = 3, at = 8); par(las = 0)\n\
         text(2, 8, expression(x^2 + y^2 == r^2))\n\
         text(7, 3, expression(alpha %+-% beta), adj = c(0, 1), cex = 1.5, srt = 30)\n\
         text(5, 2, expression(paste(\"P = \", 0.05)), col = 'red')\n\
         legend('topright', legend = expression(alpha, beta^2, x[i]^(y^2)), title = expression(Sigma[j]), lty = 1:3)",
    );
    let r_pdf = r::run_r(
        "plotmath_other",
        (PAGE_W, PAGE_H),
        "",
        &[("all", script.as_str())],
    )
    .pdfs
    .remove(0);

    use rs_graphics::color::{Color, ColorSpec};
    use rs_graphics::device::{
        LegendKeyword, LegendOptions, LegendPos, MtextOptions, TitleOptions,
    };
    use rs_graphics::lty::LineType;
    use rs_graphics::par::Sides;
    let par = Par {
        mar: Sides::new(5.0, 6.0, 5.0, 6.0),
        oma: Sides::new(2.0, 0.0, 2.0, 0.0),
        ..Par::default()
    };
    let mut d = Device::new(PdfBackend::new(PAGE_W, PAGE_H).with_compression(false), par);
    d.plot_new();
    d.plot_window().xlim(0.0, 10.0).ylim(0.0, 10.0).draw();
    d.title()
        .with(&TitleOptions {
            main: Some(Label::math("Main ~ alpha^2")),
            sub: Some(Label::math("sub[i]")),
            xlab: Some(Label::math("x[1] + x[2]")),
            ylab: Some(Label::math("-log[10](italic(p))")),
            ..TitleOptions::default()
        })
        .draw();
    let mt = |line: f64| MtextOptions {
        line: vec![line],
        ..MtextOptions::default()
    };
    d.mtext()
        .text(Label::math("beta^2"))
        .side(3)
        .with(&MtextOptions {
            adj: vec![0.0],
            ..mt(0.5)
        })
        .draw();
    d.mtext()
        .text(Label::math("Omega ~ infinity"))
        .side(4)
        .with(&mt(1.0))
        .draw();
    d.mtext()
        .text(Label::math("10^-5"))
        .side(1)
        .with(&MtextOptions {
            outer: true,
            ..mt(0.5)
        })
        .draw();
    d.par().las(1).draw();
    d.mtext()
        .text(Label::math("gamma[k]"))
        .side(2)
        .with(&MtextOptions {
            at: vec![8.0],
            ..mt(3.0)
        })
        .draw();
    d.par().las(0).draw();
    let black = ColorSpec::Index(1);
    d.text()
        .x([2.0])
        .y([8.0])
        .labels(&[Label::math("x^2 + y^2 == r^2")])
        .with(&TextOptions {
            adj: Some(0.5),
            col: vec![black],
            cex: vec![Some(1.0)],
            font: vec![Some(FontFace::Plain)],
            ..TextOptions::default()
        })
        .draw();
    d.text()
        .x([7.0])
        .y([3.0])
        .labels(&[Label::math("alpha %+-% beta")])
        .with(&TextOptions {
            adj: Some(0.0),
            vadj: Some(1.0),
            srt: Some(30.0),
            col: vec![black],
            cex: vec![Some(1.5)],
            font: vec![Some(FontFace::Plain)],
            ..TextOptions::default()
        })
        .draw();
    let red = ColorSpec::Color(Color::parse("red").unwrap());
    d.text()
        .x([5.0])
        .y([2.0])
        .labels(&[Label::math("paste(\"P = \", 0.05)")])
        .with(&TextOptions {
            adj: Some(0.5),
            col: vec![red],
            cex: vec![Some(1.0)],
            font: vec![Some(FontFace::Plain)],
            ..TextOptions::default()
        })
        .draw();
    let labels = ["alpha", "beta^2", "x[i]^(y^2)"].map(Label::math);
    let opts = LegendOptions {
        col: vec![black],
        lty: (1..=3)
            .map(|c| Some(LineType::from_code(c).unwrap()))
            .collect(),
        lwd: vec![Some(1.0)],
        title: Some(Label::math("Sigma[j]")),
        ..LegendOptions::default()
    };
    d.legend()
        .pos(LegendPos::Keyword(LegendKeyword::TopRight))
        .labels(labels)
        .with(&opts)
        .draw();

    let ours_pdf = d.into_backend().finish();
    // R clips `mtext()` to the device even for inner margins (its
    // `C_mtext` tests `outer` on the SEXP); we clip to the figure.
    let how = pdf::Compare {
        clip: false,
        ..Default::default()
    };
    pdf::assert_same_pdf(
        "plotmath_title_mtext_text_legend_match_r",
        pdf::parse(&ours_pdf),
        pdf::parse(&r_pdf),
        &how,
    );
}

#[test]
fn plotmath_high_level_titles_match_r() {
    if !common::r_available() {
        return;
    }
    let script = String::from(
        "par(mfrow = c(2, 3))\n\
         plot(1:10, (1:10)^2, main = expression(y == x^2), sub = expression(n[obs]),\n\
              xlab = expression(x[i]), ylab = expression(-log[10](italic(p))))\n\
         x <- c(1, 2, 2, 3, 3, 3, 4, 4, 5)\n\
         hist(x, main = expression(Histogram ~ of ~ X[1]), ylab = expression(count[i]))\n\
         barplot(c(3, 5, 2), names.arg = c('a', 'b', 'c'), main = expression(bold(Bars)),\n\
                 xlab = expression(group[k]))\n\
         boxplot(list(c(1, 2, 3, 4, 9), c(2, 3, 4)), main = expression(sigma^2),\n\
                 ylab = expression(mu %+-% sigma), outline = FALSE)\n\
         pie(c(1, 2, 3), labels = c('a', 'b', 'c'), main = expression(pi * r^2))",
    );
    let r_pdf = r::run_r(
        "plotmath_highlevel",
        (PAGE_W, PAGE_H),
        "",
        &[("all", script.as_str())],
    )
    .pdfs
    .remove(0);

    use rs_graphics::highlevel::{
        BarplotOptions, BoxplotOptions, HistOptions, PieOptions, PlotOptions,
    };
    let math = |s: &str| Some(Label::math(s));
    let abc = || Some(vec!["a".into(), "b".into(), "c".into()]);
    let mut d = Device::new(
        PdfBackend::new(PAGE_W, PAGE_H).with_compression(false),
        Par::default(),
    );
    d.par().mfrow(2, 3).draw();
    let x: Vec<f64> = (1..=10).map(f64::from).collect();
    let y: Vec<f64> = x.iter().map(|v| v * v).collect();
    d.plot()
        .x(&x)
        .y(&y)
        .with(&PlotOptions {
            main: math("y == x^2"),
            sub: math("n[obs]"),
            xlab: math("x[i]"),
            ylab: math("-log[10](italic(p))"),
            ..PlotOptions::default()
        })
        .draw();
    d.hist()
        .x([1.0, 2.0, 2.0, 3.0, 3.0, 3.0, 4.0, 4.0, 5.0])
        .with(&HistOptions {
            main: math("Histogram ~ of ~ X[1]"),
            ylab: math("count[i]"),
            ..HistOptions::default()
        })
        .draw();
    d.barplot()
        .height([3.0, 5.0, 2.0])
        .with(&BarplotOptions {
            names: abc(),
            main: math("bold(Bars)"),
            xlab: math("group[k]"),
            ..BarplotOptions::default()
        })
        .draw();
    d.boxplot()
        .x(&[vec![1.0, 2.0, 3.0, 4.0, 9.0], vec![2.0, 3.0, 4.0]])
        .with(&BoxplotOptions {
            main: math("sigma^2"),
            ylab: math("mu %+-% sigma"),
            // R's outliers are `par(cex)` times smaller on this page (an R
            // quirk we don't copy), and only the titles matter here.
            outline: false,
            ..BoxplotOptions::default()
        })
        .draw();
    d.pie()
        .x([1.0, 2.0, 3.0])
        .with(&PieOptions {
            labels: abc(),
            main: math("pi * r^2"),
            ..PieOptions::default()
        })
        .draw();

    let ours_pdf = d.into_backend().finish();
    pdf::assert_same_pdf(
        "plotmath_high_level_titles_match_r",
        pdf::parse(&ours_pdf),
        pdf::parse(&r_pdf),
        &pdf::Compare::default(),
    );
}
