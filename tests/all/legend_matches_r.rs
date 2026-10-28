// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks `legend()` against a real R `pdf()` device's uncompressed
//! output (every painted path and text item in order, see
//! `common::pdf::assert_same`), for keyword positions, `inset`,
//! multi-column / horizontal layouts, titles, filled boxes,
//! `cex`/`intersp`, `par(cex)` and corner-pair positioning, and
//! `legend()`'s return value (also on log axes). Skipped if `Rscript`
//! isn't available.

use crate::common;
use crate::common::{pdf, r};

use std::process::Command;

use rs_graphics::prelude::*;

struct Case {
    name: &'static str,
    r: &'static str,
    run: fn(&mut Device<PdfBackend>) -> LegendLayout,
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "topright_title_merge",
            r: r#"legend("topright", legend=c("alpha","beta gamma"), col=c(1,2), pch=c(19,1), lty=c(1,2), lwd=c(1,2), title="Key")"#,
            run: |d| {
                d.legend()
                    .pos("topright")
                    .labels(["alpha", "beta gamma"])
                    .col([1, 2])
                    .pch([19, 1])
                    .lty([1, 2])
                    .lwd([1, 2])
                    .title("Key")
                    .draw()
            },
        },
        Case {
            name: "bottomleft_inset",
            r: r#"legend("bottomleft", legend=c("a","bb","ccc"), pch=c(1,2,3), inset=0.05)"#,
            run: |d| {
                d.legend()
                    .pos("bottomleft")
                    .labels(["a", "bb", "ccc"])
                    .pch([1, 2, 3])
                    .inset((0.05, 0.05))
                    .draw()
            },
        },
        Case {
            name: "center_ncol2_nobox",
            r: r#"legend("center", legend=c("one","two","three"), lty=1:3, col=2:4, ncol=2, bty="n")"#,
            run: |d| {
                d.legend()
                    .pos("center")
                    .labels(["one", "two", "three"])
                    .lty([1, 2, 3])
                    .col([2, 3, 4])
                    .ncol(2)
                    .bty("n")
                    .draw()
            },
        },
        Case {
            name: "top_horiz_fill",
            r: r#"legend("top", legend=c("x","yy"), fill=c("red","blue"), horiz=TRUE)"#,
            run: |d| {
                d.legend()
                    .pos("top")
                    .labels(["x", "yy"])
                    .fill(["red", "blue"])
                    .horiz(true)
                    .draw()
            },
        },
        Case {
            name: "xy_cex_intersp_title",
            r: r#"legend(5, 20, legend=c("p","qq","rrr"), pch=1:3, cex=0.8, y.intersp=1.5, x.intersp=2, title="T")"#,
            run: |d| {
                d.legend()
                    .pos((5, 20))
                    .labels(["p", "qq", "rrr"])
                    .pch([1, 2, 3])
                    .cex(0.8)
                    .y_intersp(1.5)
                    .x_intersp(2)
                    .title("T")
                    .draw()
            },
        },
        Case {
            name: "corners",
            r: r#"legend(c(5,15), c(10,20), legend=c("a","b"), pch=1:2)"#,
            run: |d| {
                d.legend()
                    .pos(((5, 15), (10, 20)))
                    .labels(["a", "b"])
                    .pch([1, 2])
                    .draw()
            },
        },
        Case {
            name: "right_long_title_seglen",
            r: r#"legend("right", legend=c("lo","hi"), lty=1, lwd=2, seg.len=3, title="A rather long legend title")"#,
            run: |d| {
                d.legend()
                    .pos("right")
                    .labels(["lo", "hi"])
                    .lty(1)
                    .lwd(2)
                    .seg_len(3)
                    .title("A rather long legend title")
                    .draw()
            },
        },
        Case {
            name: "par_cex",
            r: r#"par(cex = 1.5); legend("topright", legend=c("alpha","beta gamma"), pch=1:2, lty=1, cex=0.8, title="Key")"#,
            run: |d| {
                d.par().cex(1.5).draw();
                d.legend()
                    .pos("topright")
                    .labels(["alpha", "beta gamma"])
                    .pch([1, 2])
                    .lty(1)
                    .cex(0.8)
                    .title("Key")
                    .draw()
            },
        },
        Case {
            name: "topleft_text_width",
            r: r#"legend("topleft", legend=c("a","b"), pch=1:2, text.width=2)"#,
            run: |d| {
                d.legend()
                    .pos("topleft")
                    .labels(["a", "b"])
                    .pch([1, 2])
                    .text_width(2)
                    .draw()
            },
        },
    ]
}

/// Compares `legend()`'s return value with R's, printed by the script as
/// `name left top w h text.x... text.y...`. Tolerance: 1/5000 of the axis
/// range (about 0.1pt on these pages).
fn check_return_value(name: &str, ours: &LegendLayout, r_stdout: &str, usr: (f64, f64, f64, f64)) {
    let line = r_stdout
        .lines()
        .find(|l| l.split_whitespace().next() == Some(name))
        .unwrap_or_else(|| panic!("{name}: no return value printed by R"));
    let r: Vec<f64> = line
        .split_whitespace()
        .skip(1)
        .map(|v| v.parse().unwrap())
        .collect();
    let n = ours.text_x.len();
    assert_eq!(r.len(), 4 + 2 * n, "{name}: number of returned values");
    let (tx, ty) = (
        (usr.1 - usr.0).abs() / 5000.0,
        (usr.3 - usr.2).abs() / 5000.0,
    );
    let mut ours_flat = vec![(ours.left, tx), (ours.top, ty), (ours.w, tx), (ours.h, ty)];
    ours_flat.extend(ours.text_x.iter().map(|&v| (v, tx)));
    ours_flat.extend(ours.text_y.iter().map(|&v| (v, ty)));
    for (k, (&(a, tol), &b)) in ours_flat.iter().zip(&r).enumerate() {
        assert!(
            (a - b).abs() < tol,
            "{name}: returned value {k} differs: ours={a} r={b}\nours={ours:?}\nr={r:?}"
        );
    }
}

#[test]
fn legend_layout_matches_r() {
    if !common::r_available() {
        return;
    }
    let cases = cases();
    let page = (6.0, 4.0);
    let codes: Vec<String> = cases
        .iter()
        .map(|c| {
            format!(
                "plot(x, y, pch = 19, xlab = 'x', ylab = 'y')\n\
                 l <- {{ {} }}\n\
                 cat('{}', unlist(l$rect[c('left', 'top', 'w', 'h')]), l$text$x, l$text$y, '\\n')",
                c.r, c.name
            )
        })
        .collect();
    let r_cases: Vec<_> = cases
        .iter()
        .zip(&codes)
        .map(|(c, r)| (c.name, r.as_str()))
        .collect();
    let run = r::run_r(
        "legend",
        page,
        "x <- 1:25; y <- sin(x * 0.35) * 8 + x * 0.6",
        &r_cases,
    );

    let x: Vec<f64> = (1..=25).map(|i| i as f64).collect();
    let y: Vec<f64> = x.iter().map(|v| (v * 0.35).sin() * 8.0 + v * 0.6).collect();

    for (c, r_pdf) in cases.iter().zip(&run.pdfs) {
        let (ours, (layout, usr)) = r::ours(page, |dev| {
            dev.plot().x(&x).y(&y).pch(19).xlab("x").ylab("y").draw();
            let layout = (c.run)(dev);
            (layout, dev.usr())
        });
        check_return_value(c.name, &layout, &run.stdout, usr);
        pdf::assert_same_pdf(
            c.name,
            pdf::parse(&ours),
            pdf::parse(r_pdf),
            &pdf::Compare::default(),
        );
    }
}

/// On log axes R's return value is in `usr` units (log10), as ours.
#[test]
fn legend_return_value_on_log_axes_matches_r() {
    if !common::r_available() {
        return;
    }
    let script = "pdf(NULL, width = 6, height = 4)\n\
                  plot(c(1, 1000), c(2, 500), log = 'xy')\n\
                  l <- legend('topleft', legend = c('a', 'bb'), pch = 1:2, inset = 0.05)\n\
                  cat('log', unlist(l$rect[c('left', 'top', 'w', 'h')]), l$text$x, l$text$y, '\\n')\n";
    let out = Command::new("Rscript")
        .args(["-e", script])
        .output()
        .expect("run Rscript");
    assert!(
        out.status.success(),
        "Rscript failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let mut dev = Device::new(PdfBackend::new(6.0, 4.0), Par::default());
    let opts = PlotOptions {
        xlog: true,
        ylog: true,
        ..PlotOptions::default()
    };
    dev.plot()
        .x([1.0, 1000.0])
        .y([2.0, 500.0])
        .with(&opts)
        .draw();
    let layout = dev
        .legend()
        .pos("topleft")
        .labels(["a", "bb"])
        .pch([1, 2])
        .inset((0.05, 0.05))
        .draw();
    check_return_value(
        "log",
        &layout,
        &String::from_utf8_lossy(&out.stdout),
        dev.usr(),
    );
}
