// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks plotmath's fractions (`frac`, `over`, `atop`), radicals
//! (`sqrt`), accents (`hat`, `tilde`, `dot`, `ring`, `bar`, `widehat`,
//! `widetilde`), `abs`, `underline`, `bgroup` delimiters, big operators
//! (`sum`, `prod`, `integral`, `union`, `intersect`, `lim`, `min`, `max`,
//! `inf`, `sup`, ...) and `{}` against a real R `pdf()` device's
//! uncompressed output, in display and script styles, rotated and scaled:
//! every text item and painted path (fraction bars, radicals, accents,
//! `abs` bars) in order (see `common::pdf::assert_same`). Skipped if
//! `Rscript` isn't available.

use crate::common;
use crate::common::{pdf, r};

use rs_graphics::prelude::*;

const PAGE_W: f64 = 8.0;
const PAGE_H: f64 = 9.0;

/// `(x, y, expression, cex, srt)`, each drawn with `text()`.
const ITEMS: &[(f64, f64, &str, f64, f64)] = &[
    (1.0, 9.6, "frac(1, 2)", 1.0, 0.0),
    (3.0, 9.6, "frac(x^2 + 1, alpha[i])", 1.0, 0.0),
    (5.0, 9.6, "over(a, b + c)", 1.0, 0.0),
    (7.0, 9.6, "atop(n, k)", 1.0, 0.0),
    (9.0, 9.6, "e^{frac(-x^2, 2)}", 1.0, 0.0),
    (1.0, 8.5, "sqrt(x)", 1.0, 0.0),
    (3.0, 8.5, "sqrt(x^2 + y^2, 3)", 1.0, 0.0),
    (5.0, 8.5, "sqrt(frac(a, b))", 1.0, 0.0),
    (7.0, 8.5, "root(italic(f), n + 1)", 1.0, 0.0),
    (9.0, 8.5, "x^sqrt(y)", 1.5, 0.0),
    (1.0, 7.4, "hat(x) + tilde(y) + dot(z) + ring(A)", 1.0, 0.0),
    (3.6, 7.4, "bar(x) == widehat(xyz)", 1.0, 0.0),
    (6.2, 7.4, "widetilde(abc) * hat(italic(f))", 1.0, 0.0),
    (8.8, 7.4, "bar(italic(W))[i]", 1.0, 0.0),
    (1.0, 6.3, "abs(x - y)", 1.0, 0.0),
    (3.0, 6.3, "underline(bold(x)) + underline(g)", 1.0, 0.0),
    (5.0, 6.3, "abs(frac(1, n))", 1.0, 0.0),
    (7.0, 6.3, "{}^14*C", 1.0, 0.0),
    (9.0, 6.3, "{a + b}^2", 1.0, 0.0),
    (1.0, 5.2, "bgroup(\"(\", frac(a, b), \")\")", 1.0, 0.0),
    (3.0, 5.2, "bgroup(\"[\", x, \"]\")", 1.0, 0.0),
    (5.0, 5.2, "bgroup(\"{\", atop(a, b), \"}\")", 1.0, 0.0),
    (7.0, 5.2, "bgroup(\"|\", frac(x, y), \"|\")", 1.0, 0.0),
    (
        9.0,
        5.2,
        "bgroup(\".\", frac(1, 2), \"|\") * group(lceil, x, rfloor)",
        1.0,
        0.0,
    ),
    (1.0, 3.9, "sum(x[i], i == 1, n)", 1.0, 0.0),
    (3.0, 3.9, "prod(p[k], k == 1, K)", 1.0, 0.0),
    (5.0, 3.9, "integral(f(x) * dx, a, b)", 1.0, 0.0),
    (
        7.0,
        3.9,
        "union(A[i], i == 1, n) * intersect(B[j], j)",
        1.0,
        0.0,
    ),
    (9.0, 3.9, "lim(f(x), x %->% 0)", 1.0, 0.0),
    (1.0, 2.6, "max(x[i], i) - min(y)", 1.0, 0.0),
    (3.0, 2.6, "inf(S) + sup(S, k)", 1.0, 0.0),
    (5.0, 2.6, "liminf(a[n], n %->% infinity)", 1.0, 0.0),
    (
        7.0,
        2.6,
        "textstyle(sum(x[i], i == 1, n) + integral(g, 0, 1))",
        1.0,
        0.0,
    ),
    (9.0, 2.6, "e^{sum(k, k == 1, n)}", 1.0, 0.0),
    (1.5, 1.2, "frac(sqrt(x), 2)", 1.3, 30.0),
    (4.0, 1.2, "bgroup(\"(\", sum(x[i], i), \")\")", 1.0, 90.0),
    (
        6.5,
        1.2,
        "paste(\"Mean \", bar(x), \" = \", frac(1, n) * sum(x[i], i == 1, n))",
        0.8,
        0.0,
    ),
    (9.0, 1.2, "integral(f, a, b)", 2.0, -20.0),
    (1.0, 0.3, "frac(atop(a, b), atop(c, d))", 1.0, 0.0),
    (3.0, 0.3, "frac(1, x^2) + frac(1, y[i]^2)", 1.0, 0.0),
    (
        5.0,
        0.3,
        "bgroup(\"{\", x, \"}\") + bgroup(\"(\", x, \")\")",
        1.0,
        0.0,
    ),
    (7.0, 0.3, "sqrt(x, n)", 1.0, 0.0),
];

#[test]
fn plotmath_extras_match_r() {
    if !common::r_available() {
        return;
    }
    let mut script = String::from(
        "par(mar = c(1, 1, 1, 1))\n\
         plot.new(); plot.window(c(0, 10), c(0, 10))\n",
    );
    for &(x, y, e, cex, srt) in ITEMS {
        script.push_str(&format!(
            "text({x}, {y}, expression({e}), cex = {cex}, srt = {srt})\n"
        ));
    }
    let r_pdf = r::run_r(
        "plotmath_extras",
        (PAGE_W, PAGE_H),
        "",
        &[("all", script.as_str())],
    )
    .pdfs
    .remove(0);

    let mut d = Device::memory("pdf")
        .width(PAGE_W)
        .height(PAGE_H)
        .compress(false)
        .open()
        .unwrap();
    d.par().mar([1, 1, 1, 1]).draw();
    d.plot_new();
    d.plot_window().xlim([0, 10]).ylim([0, 10]).draw();
    for &(x, y, e, cex, srt) in ITEMS {
        d.text()
            .x([x])
            .y([y])
            .labels([Label::math(e)])
            .cex(cex)
            .srt(srt)
            .draw();
    }

    let ours_pdf = d.finish();
    pdf::assert_same_pdf(
        "plotmath_extras_match_r",
        pdf::parse(&ours_pdf),
        pdf::parse(&r_pdf),
        &pdf::Compare::default(),
    );
}
