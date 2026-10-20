// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks `legend()` layout (box rectangle + every label origin)
//! against a real R `pdf()` device's uncompressed output, for keyword
//! positions, `inset`, multi-column / horizontal layouts, titles, filled
//! boxes, `cex`/`intersp`, `par(cex)` and corner-pair positioning. Skipped if
//! `Rscript` isn't available.

mod common;

use std::process::Command;

use rs_graphics::backend::pdf::PdfBackend;
use rs_graphics::color::{Color, ColorSpec};
use rs_graphics::device::{
    Device, LegendBty, LegendEntry, LegendKeyword, LegendLayout, LegendOptions, LegendPos,
};
use rs_graphics::highlevel::PlotOptions;
use rs_graphics::lty::LineType;
use rs_graphics::par::Par;
use rs_graphics::graphics::Graphics;

/// `(x, y)` of every text matrix, sorted.
fn text_origins(pdf: &str) -> Vec<(f64, f64)> {
    let mut out = Vec::new();
    for line in pdf.lines() {
        let Some(idx) = line.find(" Tm") else {
            continue;
        };
        // Reversed: [y, x, ...].
        let nums: Vec<f64> = line[..idx]
            .split_whitespace()
            .rev()
            .take(6)
            .filter_map(|t| t.parse().ok())
            .collect();
        if nums.len() == 6 {
            out.push((nums[1], nums[0]));
        }
    }
    out.sort_by(|a, b| a.partial_cmp(b).unwrap());
    out
}

/// `[x, y_bottom, w, h]` of every drawn rectangle (`... re` without a clip),
/// ignoring full-page backgrounds, normalised so `h > 0`.
fn rects(pdf: &str) -> Vec<[f64; 4]> {
    let mut out = Vec::new();
    for line in pdf.lines() {
        let line = line.trim();
        if !line.ends_with(" re") {
            continue;
        }
        let nums: Vec<f64> = line
            .split_whitespace()
            .filter_map(|t| t.parse().ok())
            .collect();
        // Skip the page background and the plot-region `box()` (which R
        // draws as a polyline rather than a rectangle).
        if nums.len() != 4
            || nums[2] >= 431.0
            || (nums[0] - 59.04).abs() < 0.01 && (nums[1] - 73.44).abs() < 0.01
        {
            continue;
        }
        let (x, y, w, h) = (nums[0], nums[1], nums[2], nums[3]);
        out.push(if h < 0.0 {
            [x, y + h, w, -h]
        } else {
            [x, y, w, h]
        });
    }
    out.sort_by(|a, b| a.partial_cmp(b).unwrap());
    out
}

fn black() -> ColorSpec {
    ColorSpec::Color(Color::BLACK)
}

fn named(s: &str) -> ColorSpec {
    ColorSpec::Color(Color::parse(s).unwrap())
}

fn solid() -> LineType {
    LineType::from_code(1)
}

struct Case {
    name: &'static str,
    r: &'static str,
    run: fn(&mut Device<PdfBackend>) -> Option<LegendLayout>,
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "topright_title_merge",
            r: r#"legend("topright", legend=c("alpha","beta gamma"), col=c(1,2), pch=c(19,1), lty=c(1,2), lwd=c(1,2), title="Key")"#,
            run: |d| {
                let e = vec![
                    LegendEntry::point("alpha", ColorSpec::Index(1), 19)
                        .lwd(1.0)
                        .lty(solid()),
                    LegendEntry::point("beta gamma", ColorSpec::Index(2), 1)
                        .lwd(2.0)
                        .lty(LineType::from_code(2)),
                ];
                let o = LegendOptions {
                    title: Some("Key".into()),
                    ..Default::default()
                };
                d.legend(LegendPos::Keyword(LegendKeyword::TopRight), &e)
                    .with(&o)
                    .draw()
            },
        },
        Case {
            name: "bottomleft_inset",
            r: r#"legend("bottomleft", legend=c("a","bb","ccc"), pch=c(1,2,3), inset=0.05)"#,
            run: |d| {
                let e = vec![
                    LegendEntry::point("a", black(), 1),
                    LegendEntry::point("bb", black(), 2),
                    LegendEntry::point("ccc", black(), 3),
                ];
                let o = LegendOptions {
                    inset: Some((0.05, 0.05)),
                    ..Default::default()
                };
                d.legend(LegendPos::Keyword(LegendKeyword::BottomLeft), &e)
                    .with(&o)
                    .draw()
            },
        },
        Case {
            name: "center_ncol2_nobox",
            r: r#"legend("center", legend=c("one","two","three"), lty=1:3, col=2:4, ncol=2, bty="n")"#,
            run: |d| {
                let e = vec![
                    LegendEntry::line("one", ColorSpec::Index(2), 1.0, LineType::from_code(1)),
                    LegendEntry::line("two", ColorSpec::Index(3), 1.0, LineType::from_code(2)),
                    LegendEntry::line("three", ColorSpec::Index(4), 1.0, LineType::from_code(3)),
                ];
                let o = LegendOptions {
                    ncol: Some(2),
                    bty: Some(LegendBty::None),
                    ..Default::default()
                };
                d.legend(LegendPos::Keyword(LegendKeyword::Center), &e)
                    .with(&o)
                    .draw()
            },
        },
        Case {
            name: "top_horiz_fill",
            r: r#"legend("top", legend=c("x","yy"), fill=c("red","blue"), horiz=TRUE)"#,
            run: |d| {
                let e = vec![
                    LegendEntry::filled("x", named("red")),
                    LegendEntry::filled("yy", named("blue")),
                ];
                let o = LegendOptions {
                    horiz: true,
                    ..Default::default()
                };
                d.legend(LegendPos::Keyword(LegendKeyword::Top), &e)
                    .with(&o)
                    .draw()
            },
        },
        Case {
            name: "xy_cex_intersp_title",
            r: r#"legend(5, 20, legend=c("p","qq","rrr"), pch=1:3, cex=0.8, y.intersp=1.5, x.intersp=2, title="T")"#,
            run: |d| {
                let e = vec![
                    LegendEntry::point("p", black(), 1),
                    LegendEntry::point("qq", black(), 2),
                    LegendEntry::point("rrr", black(), 3),
                ];
                let o = LegendOptions {
                    cex: Some(0.8),
                    y_intersp: Some(1.5),
                    x_intersp: Some(2.0),
                    title: Some("T".into()),
                    ..Default::default()
                };
                d.legend(LegendPos::Xy(5.0, 20.0), &e).with(&o).draw()
            },
        },
        Case {
            name: "corners",
            r: r#"legend(c(5,15), c(10,20), legend=c("a","b"), pch=1:2)"#,
            run: |d| {
                let e = vec![
                    LegendEntry::point("a", black(), 1),
                    LegendEntry::point("b", black(), 2),
                ];
                d.legend(LegendPos::Corners(5.0, 10.0, 15.0, 20.0), &e)
                    .draw()
            },
        },
        Case {
            name: "right_long_title_seglen",
            r: r#"legend("right", legend=c("lo","hi"), lty=1, lwd=2, seg.len=3, title="A rather long legend title")"#,
            run: |d| {
                let e = vec![
                    LegendEntry::line("lo", black(), 2.0, solid()),
                    LegendEntry::line("hi", black(), 2.0, solid()),
                ];
                let o = LegendOptions {
                    seg_len: Some(3.0),
                    title: Some("A rather long legend title".into()),
                    ..Default::default()
                };
                d.legend(LegendPos::Keyword(LegendKeyword::Right), &e)
                    .with(&o)
                    .draw()
            },
        },
        Case {
            name: "par_cex",
            r: r#"par(cex = 1.5); legend("topright", legend=c("alpha","beta gamma"), pch=1:2, lty=1, cex=0.8, title="Key")"#,
            run: |d| {
                d.par.cex = 1.5;
                let e = vec![
                    LegendEntry::point("alpha", black(), 1)
                        .lwd(1.0)
                        .lty(solid()),
                    LegendEntry::point("beta gamma", black(), 2)
                        .lwd(1.0)
                        .lty(solid()),
                ];
                let o = LegendOptions {
                    cex: Some(0.8),
                    title: Some("Key".into()),
                    ..Default::default()
                };
                d.legend(LegendPos::Keyword(LegendKeyword::TopRight), &e)
                    .with(&o)
                    .draw()
            },
        },
        Case {
            name: "topleft_text_width",
            r: r#"legend("topleft", legend=c("a","b"), pch=1:2, text.width=2)"#,
            run: |d| {
                let e = vec![
                    LegendEntry::point("a", black(), 1),
                    LegendEntry::point("b", black(), 2),
                ];
                let o = LegendOptions {
                    text_width: Some(2.0),
                    ..Default::default()
                };
                d.legend(LegendPos::Keyword(LegendKeyword::TopLeft), &e)
                    .with(&o)
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
    let dir = std::env::temp_dir();
    let mut script = String::from("x <- 1:25; y <- sin(x * 0.35) * 8 + x * 0.6\n");
    for c in &cases {
        script.push_str(&format!(
            "pdf(file = '{}/rs_graphics_legend_{}.pdf', width = 6, height = 4, compress = FALSE)\n\
             plot(x, y, pch = 19, xlab = 'x', ylab = 'y')\n\
             l <- {{ {} }}\n\
             cat('{}', unlist(l$rect[c('left', 'top', 'w', 'h')]), l$text$x, l$text$y, '\\n')\n\
             invisible(dev.off())\n",
            dir.display(),
            c.name,
            c.r,
            c.name
        ));
    }
    let script_path = dir.join("rs_graphics_legend.R");
    std::fs::write(&script_path, script).unwrap();
    let out = Command::new("Rscript")
        .arg(&script_path)
        .output()
        .expect("run Rscript");
    assert!(
        out.status.success(),
        "Rscript failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let x: Vec<f64> = (1..=25).map(|i| i as f64).collect();
    let y: Vec<f64> = x.iter().map(|v| (v * 0.35).sin() * 8.0 + v * 0.6).collect();

    for c in &cases {
        let path = dir.join(format!("rs_graphics_legend_{}.pdf", c.name));
        let r_pdf = String::from_utf8_lossy(&std::fs::read(&path).unwrap()).into_owned();

        let mut dev = Device::new(PdfBackend::new(6.0, 4.0).with_compression(false), Par::default());
        let opts = PlotOptions {
            pch: vec![19],
            xlab: Some("x".into()),
            ylab: Some("y".into()),
            ..PlotOptions::default()
        };
        dev.plot(&x, &y).with(&opts).draw();
        let layout = (c.run)(&mut dev).expect("legend drawn");
        check_return_value(
            c.name,
            &layout,
            &String::from_utf8_lossy(&out.stdout),
            dev.usr(),
        );
        let ours = String::from_utf8_lossy(&dev.into_backend().finish()).into_owned();

        let (r_t, our_t) = (text_origins(&r_pdf), text_origins(&ours));
        assert_eq!(our_t.len(), r_t.len(), "{}: text item count", c.name);
        for (a, b) in our_t.iter().zip(&r_t) {
            assert!(
                (a.0 - b.0).abs() < 0.05 && (a.1 - b.1).abs() < 0.05,
                "{}: text origin differs: ours={a:?} r={b:?}\nours={our_t:?}\nr={r_t:?}",
                c.name
            );
        }

        let (r_r, our_r) = (rects(&r_pdf), rects(&ours));
        assert_eq!(
            our_r.len(),
            r_r.len(),
            "{}: rect count\nours={our_r:?}\nr={r_r:?}",
            c.name
        );
        for (a, b) in our_r.iter().zip(&r_r) {
            for k in 0..4 {
                assert!(
                    (a[k] - b[k]).abs() < 0.05,
                    "{}: rect differs: ours={a:?} r={b:?}",
                    c.name
                );
            }
        }
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
    dev.plot(&[1.0, 1000.0], &[2.0, 500.0]).with(&opts).draw();
    let e = vec![
        LegendEntry::point("a", black(), 1),
        LegendEntry::point("bb", black(), 2),
    ];
    let o = LegendOptions {
        inset: Some((0.05, 0.05)),
        ..Default::default()
    };
    let layout = dev
        .legend(LegendPos::Keyword(LegendKeyword::TopLeft), &e)
        .with(&o)
        .draw()
        .expect("legend drawn");
    check_return_value(
        "log",
        &layout,
        &String::from_utf8_lossy(&out.stdout),
        dev.usr(),
    );
}
