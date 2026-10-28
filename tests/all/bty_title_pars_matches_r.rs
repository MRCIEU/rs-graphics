// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks `bty` (every value through `plot(bty = )`, `par(bty)`,
//! `box(bty = , which = )` and `boxplot()`'s frame) and the inline title
//! parameters (`cex.main`/`col.main`/`font.main` and the `sub` and `lab`
//! ones) of `plot()`, `title()`, `hist()`, `barplot()`, `boxplot()`
//! (which, as R's `bxp()`, passes only the `cex` and `col` ones) and
//! `pie()` against a real R `pdf()` device's uncompressed output. Compares
//! every painted path and text item in order (see
//! `common::pdf::assert_same`). Skipped if `Rscript` isn't available.

use crate::common;
use crate::common::{pdf, r};

use rs_graphics::prelude::*;

const PAGE_W: f64 = 8.0;
const PAGE_H: f64 = 6.0;

const X: [f64; 5] = [1.0, 2.0, 3.0, 4.0, 5.0];
const Y: [f64; 5] = [2.0, 4.0, 3.0, 5.0, 1.0];
const H: [f64; 12] = [1.0, 2.0, 2.0, 3.0, 3.0, 3.0, 4.0, 4.0, 5.0, 6.0, 6.5, 7.0];

type Draw = fn(&mut Device<PdfBackend>);

struct Case {
    name: &'static str,
    /// R code; `x`, `y` and `h` are the vectors above.
    r: &'static str,
    ours: Draw,
}

fn groups() -> Vec<Vec<f64>> {
    vec![vec![1.0, 2.0, 3.0, 4.0, 9.0], vec![2.0, 3.0, 5.0, 6.0]]
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "bty_every_value",
            r: "par(mfrow = c(2, 4)); for (b in c('o', 'l', '7', 'c', 'u', ']', 'n', '[')) \
                plot(x, y, ann = FALSE, bty = b)",
            ours: |d| {
                d.par().mfrow(2, 4).draw();
                for b in ["o", "l", "7", "c", "u", "]", "n", "["] {
                    d.plot().x(X).y(Y).bty(b).draw();
                }
            },
        },
        Case {
            name: "par_bty",
            r: "par(bty = 'L'); plot(x, y, ann = FALSE)",
            ours: |d| {
                d.par().bty("L").draw();
                d.plot().x(X).y(Y).draw();
            },
        },
        Case {
            name: "box_bty_which",
            r: "plot(x, y, ann = FALSE, axes = FALSE)\n\
                box(bty = 'u', lwd = 2, col = 'red')\n\
                box(which = 'figure', bty = 'l', col = 'blue')",
            ours: |d| {
                d.plot().x(X).y(Y).axes(false).draw();
                d.box_().bty("u").lwd(2).col("red").draw();
                d.box_().which(BoxWhich::Figure).bty("l").col("blue").draw();
            },
        },
        Case {
            name: "boxplot_par_bty",
            r: "par(bty = '7'); boxplot(list(c(1, 2, 3, 4, 9), c(2, 3, 5, 6)))",
            ours: |d| {
                d.par().bty("7").draw();
                d.boxplot().x(groups()).draw();
            },
        },
        Case {
            name: "plot_title_pars",
            r: "plot(x, y, main = 'title', sub = 'sub', xlab = 'xlab', ylab = 'ylab', \
                cex.main = 2, col.main = 'red', font.main = 3, \
                cex.sub = 0.8, col.sub = 'blue', font.sub = 2, \
                cex.lab = 1.5, col.lab = 'darkgreen', font.lab = 4)",
            ours: |d| {
                d.plot()
                    .x(X)
                    .y(Y)
                    .main("title")
                    .sub("sub")
                    .xlab("xlab")
                    .ylab("ylab")
                    .cex_main(2)
                    .col_main("red")
                    .font_main(3)
                    .cex_sub(0.8)
                    .col_sub("blue")
                    .font_sub(2)
                    .cex_lab(1.5)
                    .col_lab("darkgreen")
                    .font_lab(4)
                    .draw();
            },
        },
        Case {
            name: "title_pars_and_par_defaults",
            r: "par(cex.main = 1.5, col.lab = 'orange')\n\
                plot(x, y, ann = FALSE)\n\
                title(main = 'title', xlab = 'xlab', ylab = 'ylab', col.main = 'purple', \
                font.main = 1, font.lab = 3)",
            ours: |d| {
                d.par().cex_main(1.5).col_lab("orange").draw();
                d.plot().x(X).y(Y).draw();
                d.title()
                    .main("title")
                    .xlab("xlab")
                    .ylab("ylab")
                    .col_main("purple")
                    .font_main(1)
                    .font_lab(3)
                    .draw();
            },
        },
        Case {
            name: "hist_and_barplot_title_pars",
            r: "par(mfrow = c(1, 2))\n\
                hist(h, main = 'title', xlab = 'xlab', ylab = 'n', cex.main = 0.9, \
                col.main = 'red', font.lab = 3, cex.lab = 1.2)\n\
                barplot(c(3, 5, 2), main = 'title', sub = 'sub', xlab = 'xlab', ylab = 'ylab', \
                font.main = 1, col.sub = 'blue', cex.sub = 1.3, col.lab = 'grey40')",
            ours: |d| {
                d.par().mfrow(1, 2).draw();
                d.hist()
                    .x(H)
                    .main("title")
                    .xlab("xlab")
                    .ylab("n")
                    .cex_main(0.9)
                    .col_main("red")
                    .font_lab(3)
                    .cex_lab(1.2)
                    .draw();
                d.barplot()
                    .height([3, 5, 2])
                    .main("title")
                    .sub("sub")
                    .xlab("xlab")
                    .ylab("ylab")
                    .font_main(1)
                    .col_sub("blue")
                    .cex_sub(1.3)
                    .col_lab("grey40")
                    .draw();
            },
        },
        Case {
            // R's bxp() passes only cex.* and col.* to title(): font.main
            // has no effect, so it isn't an option here.
            name: "boxplot_title_pars",
            r: "boxplot(list(c(1, 2, 3, 4, 9), c(2, 3, 5, 6)), main = 'title', sub = 'sub', \
                xlab = 'xlab', ylab = 'ylab', cex.main = 2, col.main = 'red', font.main = 3, \
                cex.sub = 0.7, col.sub = 'blue', col.lab = 'darkgreen', cex.lab = 1.4)",
            ours: |d| {
                d.boxplot()
                    .x(groups())
                    .main("title")
                    .sub("sub")
                    .xlab("xlab")
                    .ylab("ylab")
                    .cex_main(2)
                    .col_main("red")
                    .cex_sub(0.7)
                    .col_sub("blue")
                    .col_lab("darkgreen")
                    .cex_lab(1.4)
                    .draw();
            },
        },
        Case {
            name: "pie_title_pars",
            r: "pie(c(1, 2, 3), labels = c('a', 'b', 'c'), main = 'title', cex.main = 0.8, \
                col.main = 'red', font.main = 4)",
            ours: |d| {
                d.pie()
                    .x([1, 2, 3])
                    .labels(["a", "b", "c"])
                    .main("title")
                    .cex_main(0.8)
                    .col_main("red")
                    .font_main(4)
                    .draw();
            },
        },
        Case {
            name: "title_adj",
            r: "plot.new(); plot.window(c(0, 10), c(0, 10))\n\
                par(adj = 0); title(main = 'M', sub = 'S', xlab = 'X', ylab = 'Y')\n\
                title(main = 'N', xlab = 'W', ylab = 'Z', adj = 1)\n\
                par(adj = 0.5); title(sub = 'T', adj = 0.25)\n\
                title(main = expression(alpha^2), adj = 0.75)",
            ours: |d| {
                d.plot_new();
                d.plot_window().xlim(0, 10).ylim(0, 10).draw();
                d.par().adj(0).draw();
                d.title().main("M").sub("S").xlab("X").ylab("Y").draw();
                d.title().main("N").xlab("W").ylab("Z").adj(1).draw();
                d.par().adj(0.5).draw();
                d.title().sub("T").adj(0.25).draw();
                d.title().main(Label::math("alpha^2")).adj(0.75).draw();
            },
        },
        Case {
            name: "title_line_outer",
            r: "par(oma = c(2, 3, 3, 1)); plot.new(); plot.window(c(0, 10), c(0, 10))\n\
                title(main = 'M', sub = 'S', xlab = 'X', ylab = 'Y', line = 1)\n\
                title(main = 'N', outer = TRUE)\n\
                title(main = 'O', sub = 'T', xlab = 'W', ylab = 'Z', outer = TRUE, line = 0.5, adj = 0)\n\
                title(main = expression(beta^2), line = 2.5)",
            ours: |d| {
                d.par().oma([2, 3, 3, 1]).draw();
                d.plot_new();
                d.plot_window().xlim(0, 10).ylim(0, 10).draw();
                d.title()
                    .main("M")
                    .sub("S")
                    .xlab("X")
                    .ylab("Y")
                    .line(1)
                    .draw();
                d.title().main("N").outer(true).draw();
                d.title()
                    .main("O")
                    .sub("T")
                    .xlab("W")
                    .ylab("Z")
                    .outer(true)
                    .line(0.5)
                    .adj(0)
                    .draw();
                d.title().main(Label::math("beta^2")).line(2.5).draw();
            },
        },
        Case {
            name: "plot_window_pars_and_panels",
            r: "par(mfrow = c(2, 2))\n\
                plot(x, y, asp = 1, ann = FALSE)\n\
                plot(x, y, xaxs = 'i', yaxs = 'i', lab = c(10, 3, 7), ann = FALSE)\n\
                plot(x, y, ann = FALSE, panel.first = abline(h = 1:5, col = 'grey'),\n\
                     panel.last = points(3, 3, pch = 19, col = 'red'))\n\
                plot.new(); plot.window(c(0, 10), c(0, 10), xaxs = 'i', lab = c(3, 8, 7))\n\
                axis(1); axis(2)\n\
                plot.window(c(0, 10), c(0, 10)); axis(3)",
            ours: |d| {
                d.par().mfrow(2, 2).draw();
                d.plot().x(X).y(Y).asp(1).draw();
                d.plot()
                    .x(X)
                    .y(Y)
                    .xaxs("i")
                    .yaxs("i")
                    .lab([10, 3, 7])
                    .draw();
                let mut grid = Figure::new();
                grid.abline().h([1, 2, 3, 4, 5]).col("grey").draw();
                let mut mark = Figure::new();
                mark.points().x([3]).y([3]).pch(19).col("red").draw();
                d.plot().x(X).y(Y).panel_first(grid).panel_last(mark).draw();
                d.plot_new();
                d.plot_window()
                    .xlim(0, 10)
                    .ylim(0, 10)
                    .xaxs("i")
                    .lab([3, 8, 7])
                    .draw();
                d.axis().side(1).draw();
                d.axis().side(2).draw();
                // The inline xaxs and lab didn't last.
                d.plot_window().xlim(0, 10).ylim(0, 10).draw();
                d.axis().side(3).draw();
            },
        },
        Case {
            name: "mtext_recycling",
            r: "plot.new(); plot.window(c(0, 10), c(0, 5))\n\
                mtext(c('a', 'b', 'c'), side = 1, at = c(2, 5, 8), line = c(0, 1),\n\
                      col = c('red', 'blue'), cex = c(1, 1.5, 2), font = c(1, 2))\n\
                mtext('x', side = 3, at = c(1, 4, 9), adj = c(0, 1))\n\
                mtext(c('p', 'q'), side = 2, line = c(1, 2.5))\n\
                mtext(expression(alpha, beta), side = 4, at = c(1, 4))",
            ours: |d| {
                d.plot_new();
                d.plot_window().xlim(0, 10).ylim(0, 5).draw();
                d.mtext()
                    .text(["a", "b", "c"])
                    .side(1)
                    .at([2, 5, 8])
                    .line([0, 1])
                    .col(["red", "blue"])
                    .cex([1.0, 1.5, 2.0])
                    .font([1, 2])
                    .draw();
                d.mtext().text("x").side(3).at([1, 4, 9]).adj([0, 1]).draw();
                d.mtext().text(["p", "q"]).side(2).line([1.0, 2.5]).draw();
                d.mtext()
                    .text([Label::math("alpha"), Label::math("beta")])
                    .side(4)
                    .at([1, 4])
                    .draw();
            },
        },
        Case {
            name: "pie_inline_cex_font",
            r: "pie(c(1, 2, 3), labels = c('a', 'b', 'c'), main = 'M', cex = 1.8, font = 3)",
            ours: |d| {
                d.pie()
                    .x([1, 2, 3])
                    .labels(["a", "b", "c"])
                    .main("M")
                    .cex(1.8)
                    .font(3)
                    .draw();
            },
        },
        Case {
            name: "hist_labels_col_border",
            r: "par(mfrow = c(1, 3))\n\
                hist(h, labels = TRUE, col = c('red', 'blue'), border = c('black', 'green'),\n\
                     lty = c(1, 2), ann = FALSE)\n\
                hist(h, breaks = c(0, 3, 7), labels = TRUE, ann = FALSE)\n\
                hist(h, breaks = c(0, 4, 7), labels = c('p', 'q'), ann = FALSE)",
            ours: |d| {
                d.par().mfrow(1, 3).draw();
                d.hist()
                    .x(H)
                    .labels(true)
                    .col(["red", "blue"])
                    .border(["black", "green"])
                    .lty(["solid", "dashed"])
                    .ann(false)
                    .draw();
                d.hist()
                    .x(H)
                    .breaks([0, 3, 7])
                    .labels(true)
                    .ann(false)
                    .draw();
                d.hist()
                    .x(H)
                    .breaks([0, 4, 7])
                    .labels(["p", "q"])
                    .ann(false)
                    .draw();
            },
        },
        Case {
            name: "frame_names_xpd_plot_false",
            r: "par(mfrow = c(2, 3))\n\
                plot(x, y, frame.plot = FALSE, ann = FALSE)\n\
                plot(x, y, axes = FALSE, frame.plot = TRUE, ann = FALSE)\n\
                boxplot(list(a = c(1, 2, 3, 9)), show.names = TRUE)\n\
                boxplot(list(a = c(1, 2, 3, 9), b = c(2, 3, 4)), show.names = FALSE, frame.plot = FALSE)\n\
                barplot(c(3, 5, 2), ylim = c(0, 4), xpd = FALSE)\n\
                invisible(hist(h, plot = FALSE)); invisible(barplot(c(1, 2), plot = FALSE))",
            ours: |d| {
                d.par().mfrow(2, 3).draw();
                d.plot().x(X).y(Y).frame_plot(false).draw();
                d.plot().x(X).y(Y).axes(false).frame_plot(true).draw();
                d.boxplot()
                    .x(&[vec![1, 2, 3, 9]])
                    .names(["a"])
                    .show_names(true)
                    .draw();
                d.boxplot()
                    .x(&[vec![1, 2, 3, 9], vec![2, 3, 4]])
                    .names(["a", "b"])
                    .show_names(false)
                    .frame_plot(false)
                    .draw();
                d.barplot().height([3, 5, 2]).ylim(0, 4).xpd(false).draw();
                d.hist().x(H).plot(false).draw();
                d.barplot().height([1, 2]).plot(false).draw();
            },
        },
        Case {
            name: "plot_index",
            r: "par(mfrow = c(1, 2))\n\
                plot(y, ylab = '')\n\
                plot(c(3, 1, 4, 1, 5, 9, 2), type = 'b', xlab = 'i', ylab = '')",
            ours: |d| {
                d.par().mfrow(1, 2).draw();
                d.plot().x(Y).draw();
                d.plot()
                    .x([3, 1, 4, 1, 5, 9, 2])
                    .type_("b")
                    .xlab("i")
                    .draw();
            },
        },
        Case {
            // R's one-vector forms (`x` alone, against its index) and the
            // argument defaults: `segments`/`arrows` `x1 = x0`, `y1 = y0`,
            // `mtext` `side = 3`.
            name: "one_vector_and_defaults",
            r: "plot(c(0, 8), c(0, 6), type = 'n', xlab = '', ylab = '')\n\
                points(c(1, 3, 2, 5))\n\
                lines(c(2, 4, 3, 5, 1))\n\
                text(c(5, 4, 3), labels = c('a', 'b', 'c'))\n\
                polygon(c(1, 2, 1.5), col = 'grey')\n\
                segments(c(1, 2), c(5, 5.5), 6)\n\
                arrows(1, 0.5, 3)\n\
                arrows(7, 0.5, y1 = 2)\n\
                mtext('top')",
            ours: |d| {
                d.plot()
                    .x([0, 8])
                    .y([0, 6])
                    .type_("n")
                    .xlab("")
                    .ylab("")
                    .draw();
                d.points().x([1, 3, 2, 5]).draw();
                d.lines().x([2, 4, 3, 5, 1]).draw();
                d.text().x([5, 4, 3]).labels(["a", "b", "c"]).draw();
                d.polygon().x([1.0, 2.0, 1.5]).col("grey").draw();
                d.segments().x0([1, 2]).y0([5.0, 5.5]).x1([6]).draw();
                d.arrows().x0([1]).y0([0.5]).x1([3]).draw();
                d.arrows().x0([7]).y0([0.5]).y1([2]).draw();
                d.mtext().text("top").draw();
            },
        },
        Case {
            name: "plot_inline_axis_pars",
            r: "par(mfrow = c(1, 2))\n\
                plot(x, y, las = 1, cex.axis = 0.8, col.axis = 'blue', font.axis = 2,\n\
                     mgp = c(2, 0.6, 0), tck = 0.02, xlab = 'X', ylab = 'Y')\n\
                plot(x, y, tcl = -0.2, las = 2, ann = FALSE)",
            ours: |d| {
                d.par().mfrow(1, 2).draw();
                d.plot()
                    .x(X)
                    .y(Y)
                    .las(1)
                    .cex_axis(0.8)
                    .col_axis("blue")
                    .font_axis(2)
                    .mgp([2.0, 0.6, 0.0])
                    .tck(0.02)
                    .xlab("X")
                    .ylab("Y")
                    .draw();
                d.plot().x(X).y(Y).tcl(-0.2).las(2).draw();
            },
        },
        Case {
            name: "hist_barplot_inline_axis_pars",
            r: "par(mfrow = c(1, 2))\n\
                hist(h, las = 1, cex.axis = 1.2, mgp = c(2.5, 0.8, 0), tcl = 0.3, gap.axis = 3,\n\
                     main = 'M', xlab = 'X', ylab = 'Y')\n\
                barplot(c(3, 5, 2), names.arg = c('a', 'b', 'c'), las = 2, cex.axis = 0.7,\n\
                        col.axis = 'red', mgp = c(2, 0.5, 0), ylab = 'Y')",
            ours: |d| {
                d.par().mfrow(1, 2).draw();
                d.hist()
                    .x(H)
                    .las(1)
                    .cex_axis(1.2)
                    .mgp([2.5, 0.8, 0.0])
                    .tcl(0.3)
                    .gap_axis(3)
                    .main("M")
                    .xlab("X")
                    .ylab("Y")
                    .draw();
                d.barplot()
                    .height([3, 5, 2])
                    .names(["a", "b", "c"])
                    .las(2)
                    .cex_axis(0.7)
                    .col_axis("red")
                    .mgp([2.0, 0.5, 0.0])
                    .ylab("Y")
                    .draw();
            },
        },
        Case {
            name: "barplot_names_axis",
            r: "par(mfrow = c(1, 2))\n\
                barplot(c(3, 5, 2), names.arg = c('a', 'b', 'c'), cex.names = 1.5, axis.lty = 1,\n\
                        cex.axis = 0.7)\n\
                barplot(c(3, 5, 2), names.arg = c('a', 'b', 'c'), axisnames = FALSE, horiz = TRUE)",
            ours: |d| {
                d.par().mfrow(1, 2).draw();
                d.barplot()
                    .height([3, 5, 2])
                    .names(["a", "b", "c"])
                    .cex_names(1.5)
                    .axis_lty("solid")
                    .cex_axis(0.7)
                    .draw();
                d.barplot()
                    .height([3, 5, 2])
                    .names(["a", "b", "c"])
                    .axisnames(false)
                    .horiz(true)
                    .draw();
            },
        },
        Case {
            name: "boxplot_inline_axis_pars",
            r: "par(mfrow = c(1, 2))\n\
                boxplot(list(a = c(1, 2, 3, 9), b = c(2, 3, 4)), las = 2, cex.axis = 1.3,\n\
                        col.axis = 'darkgreen', yaxt = 'n')\n\
                boxplot(list(a = c(1, 2, 3, 9), b = c(2, 3, 4)), horizontal = TRUE, las = 1,\n\
                        xaxp = c(0, 10, 2), gap.axis = 0)",
            ours: |d| {
                d.par().mfrow(1, 2).draw();
                let groups = [vec![1.0, 2.0, 3.0, 9.0], vec![2.0, 3.0, 4.0]];
                d.boxplot()
                    .x(&groups)
                    .names(["a", "b"])
                    .las(2)
                    .cex_axis(1.3)
                    .col_axis("darkgreen")
                    .yaxt("n")
                    .draw();
                d.boxplot()
                    .x(&groups)
                    .names(["a", "b"])
                    .horizontal(true)
                    .las(1)
                    .xaxp([0, 10, 2])
                    .gap_axis(0)
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
fn bty_and_title_pars_match_r() {
    if !common::r_available() {
        return;
    }
    let cases = cases();
    let page = (PAGE_W, PAGE_H);
    let r_cases: Vec<_> = cases.iter().map(|c| (c.name, c.r)).collect();
    let prelude = format!(
        "x <- {}\ny <- {}\nh <- {}",
        r::r_vec(&X),
        r::r_vec(&Y),
        r::r_vec(&H)
    );
    let r = r::run_r("bty_title", page, &prelude, &r_cases);
    for (c, r_pdf) in cases.iter().zip(&r.pdfs) {
        let (ours_pdf, ()) = r::ours(page, c.ours);
        let ours = pdf::parse(&ours_pdf);
        let theirs = pdf::parse(r_pdf);
        pdf::assert_same_pdf(c.name, ours, theirs, &pdf::Compare::default());
    }
}

/// An `NA` (`NaN`) `mtext()` line leaves that label out. R writes it with a
/// `nan` coordinate, which makes the PDF invalid (an R bug, not copied).
#[test]
fn mtext_nan_line_skips_the_label() {
    let mut dev = Device::new(
        PdfBackend::new(PAGE_W, PAGE_H).with_compression(false),
        Par::default(),
    );
    dev.plot_new();
    dev.mtext()
        .text(["p", "q"])
        .side(2)
        .line([Some(1.0), None])
        .draw();
    let texts = pdf::parse(&dev.into_backend().finish()).page().texts;
    let strings: Vec<&str> = texts.iter().map(|t| t.string.as_str()).collect();
    assert_eq!(strings, ["p"]);
    for t in &texts {
        assert!(t.origin().0.is_finite() && t.origin().1.is_finite());
    }
}

/// `plot = FALSE` returns what drawing returns, and draws nothing.
#[test]
fn plot_false_returns_the_result_without_drawing() {
    let blank = || {
        Device::new(
            PdfBackend::new(PAGE_W, PAGE_H).with_compression(false),
            Par::default(),
        )
    };
    let (mut a, mut b) = (blank(), blank());
    assert_eq!(
        a.barplot().height([3, 5, 2]).plot(false).draw(),
        b.barplot().height([3, 5, 2]).draw()
    );
    let h = a.hist().x(H).plot(false).draw();
    let drawn = b.hist().x(H).draw();
    assert_eq!((h.breaks, h.counts), (drawn.breaks, drawn.counts));
    let drawn = pdf::parse(&a.into_backend().finish());
    for page in drawn.pages {
        let page = page.without_background();
        assert!(page.paths.is_empty() && page.texts.is_empty());
    }
}
