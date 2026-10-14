//! Cross-checks multi-figure output (`mfrow`/`mfcol`, `oma`, `mfg`,
//! `layout()` with spanning figures / `lcm()` / `respect`, and `par(cex)`)
//! against a real R `pdf()` device's uncompressed output: every bar
//! rectangle and every text origin. Skipped if `Rscript` isn't available.

use std::process::Command;

use rs_graphics::backend::pdf::PdfBackend;
use rs_graphics::device::Device;
use rs_graphics::highlevel::{barplot, plot, BarplotOptions, PlotOptions, PlotType};
use rs_graphics::layout::{Layout, LayoutSize};
use rs_graphics::par::{Par, Sides};

fn rscript_available() -> bool {
    Command::new("Rscript")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

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
    // Coordinates are printed with 2 (R) vs 3 (ours) decimals, so sort on
    // coarsely rounded values to get the same order from both.
    let key = |p: &(f64, f64)| ((p.0 * 10.0).round() as i64, (p.1 * 10.0).round() as i64);
    out.sort_by_key(key);
    out
}

/// `[x, y_bottom, w, h]` of every drawn rectangle, ignoring the full-page
/// background, normalised so `h > 0`.
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
        if nums.len() != 4 || (nums[2] >= PAGE_W * 72.0 - 1.0 && nums[3] >= PAGE_H * 72.0 - 1.0) {
            continue;
        }
        let (x, y, w, h) = (nums[0], nums[1], nums[2], nums[3]);
        out.push(if h < 0.0 {
            [x, y + h, w, -h]
        } else {
            [x, y, w, h]
        });
    }
    let key = |r: &[f64; 4]| r.map(|v| (v * 10.0).round() as i64);
    out.sort_by_key(key);
    out
}

const PAGE_W: f64 = 8.0;
const PAGE_H: f64 = 6.0;

/// R: `bp(h, main)`.
const R_PRELUDE: &str = "bp <- function(h, main) barplot(h, names.arg = LETTERS[seq_along(h)], main = main, ylab = 'v')\n";

fn bp(d: &mut Device<PdfBackend>, h: &[f64], main: &str) {
    let o = BarplotOptions {
        names: Some(
            (0..h.len())
                .map(|i| ((b'A' + i as u8) as char).to_string())
                .collect(),
        ),
        main: Some(main),
        ylab: Some("v"),
        ..Default::default()
    };
    barplot(d, h, &o);
}

struct Case {
    name: &'static str,
    r: &'static str,
    run: fn(&mut Device<PdfBackend>),
    /// Compare rectangles. Off where `box()` is drawn (a rectangle for us,
    /// a polyline in R).
    check_rects: bool,
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "mfrow_2x2",
            r: "par(mfrow = c(2, 2))\n\
                bp(c(3, 5, 2), 'one'); bp(c(4, 1, 6, 2), 'two')\n\
                bp(c(2, 2, 7), 'three'); bp(c(5, 3), 'four')",
            run: |d| {
                d.mfrow(2, 2);
                bp(d, &[3.0, 5.0, 2.0], "one");
                bp(d, &[4.0, 1.0, 6.0, 2.0], "two");
                bp(d, &[2.0, 2.0, 7.0], "three");
                bp(d, &[5.0, 3.0], "four");
            },
            check_rects: true,
        },
        Case {
            name: "mfrow_3x2",
            r: "par(mfrow = c(3, 2))\n\
                for (k in 1:6) bp(c(k, 7 - k, 3), paste('fig', k))",
            run: |d| {
                d.mfrow(3, 2);
                for k in 1..=6 {
                    bp(d, &[k as f64, 7.0 - k as f64, 3.0], &format!("fig {k}"));
                }
            },
            check_rects: true,
        },
        Case {
            name: "mfcol_2x3_partial",
            r: "par(mfcol = c(2, 3))\n\
                bp(c(1, 4, 2), 'a'); bp(c(3, 3, 1), 'b'); bp(c(5, 2, 4), 'c')",
            run: |d| {
                d.mfcol(2, 3);
                bp(d, &[1.0, 4.0, 2.0], "a");
                bp(d, &[3.0, 3.0, 1.0], "b");
                bp(d, &[5.0, 2.0, 4.0], "c");
            },
            check_rects: true,
        },
        Case {
            name: "mfrow_2x1_oma",
            r: "par(mfrow = c(2, 1), oma = c(1, 2, 3, 1))\n\
                bp(c(3, 5, 2), 'top'); bp(c(4, 1, 6), 'bottom')",
            run: |d| {
                d.par.oma = Sides::new(1.0, 2.0, 3.0, 1.0);
                d.mfrow(2, 1);
                bp(d, &[3.0, 5.0, 2.0], "top");
                bp(d, &[4.0, 1.0, 6.0], "bottom");
            },
            check_rects: true,
        },
        Case {
            name: "mfg",
            r: "par(mfrow = c(2, 2))\n\
                bp(c(3, 5, 2), 'first'); par(mfg = c(2, 2)); bp(c(4, 1, 6), 'last')",
            run: |d| {
                d.mfrow(2, 2);
                bp(d, &[3.0, 5.0, 2.0], "first");
                d.mfg(2, 2);
                bp(d, &[4.0, 1.0, 6.0], "last");
            },
            check_rects: true,
        },
        Case {
            name: "layout_span_heights",
            r: "layout(matrix(c(1, 1, 2, 3), 2, byrow = TRUE), heights = c(1, 2))\n\
                bp(c(3, 5, 2), 'wide'); bp(c(4, 1), 'left'); bp(c(2, 6, 1), 'right')",
            run: |d| {
                let l = Layout::new(&[vec![1, 1], vec![2, 3]])
                    .unwrap()
                    .with_heights(&[LayoutSize::Relative(1.0), LayoutSize::Relative(2.0)]);
                d.layout(l);
                bp(d, &[3.0, 5.0, 2.0], "wide");
                bp(d, &[4.0, 1.0], "left");
                bp(d, &[2.0, 6.0, 1.0], "right");
            },
            check_rects: true,
        },
        Case {
            name: "layout_cm_widths",
            r: "layout(matrix(1:2, 1), widths = c(lcm(4), 1))\n\
                bp(c(3, 5), 'narrow'); bp(c(4, 1, 6), 'wide')",
            run: |d| {
                let l = Layout::new(&[vec![1, 2]])
                    .unwrap()
                    .with_widths(&[LayoutSize::Cm(4.0), LayoutSize::Relative(1.0)]);
                d.layout(l);
                bp(d, &[3.0, 5.0], "narrow");
                bp(d, &[4.0, 1.0, 6.0], "wide");
            },
            check_rects: true,
        },
        Case {
            name: "layout_mixed_cm_heights",
            r: "layout(matrix(c(1, 2, 3, 3), 2, byrow = TRUE), widths = c(1, 2), heights = c(lcm(5), 1))\n\
                bp(c(3, 5), 'a'); bp(c(4, 1, 6), 'b'); bp(c(2, 2, 7), 'c')",
            run: |d| {
                let l = Layout::new(&[vec![1, 2], vec![3, 3]])
                    .unwrap()
                    .with_widths(&[LayoutSize::Relative(1.0), LayoutSize::Relative(2.0)])
                    .with_heights(&[LayoutSize::Cm(5.0), LayoutSize::Relative(1.0)]);
                d.layout(l);
                bp(d, &[3.0, 5.0], "a");
                bp(d, &[4.0, 1.0, 6.0], "b");
                bp(d, &[2.0, 2.0, 7.0], "c");
            },
            check_rects: true,
        },
        Case {
            name: "layout_respect",
            r: "layout(matrix(1:4, 2, byrow = TRUE), respect = TRUE)\n\
                for (k in 1:4) bp(c(k, 2), paste('r', k))",
            run: |d| {
                let l = Layout::new(&[vec![1, 2], vec![3, 4]])
                    .unwrap()
                    .with_respect(true);
                d.layout(l);
                for k in 1..=4 {
                    bp(d, &[k as f64, 2.0], &format!("r {k}"));
                }
            },
            check_rects: true,
        },
        Case {
            name: "layout_respect_matrix",
            r: "layout(matrix(1:2, 1), widths = c(1, 2), respect = matrix(c(TRUE, FALSE), 1))\n\
                bp(c(3, 5), 'sq'); bp(c(4, 1, 6), 'free')",
            run: |d| {
                let l = Layout::new(&[vec![1, 2]])
                    .unwrap()
                    .with_widths(&[LayoutSize::Relative(1.0), LayoutSize::Relative(2.0)])
                    .with_respect_matrix(&[vec![true, false]])
                    .unwrap();
                d.layout(l);
                bp(d, &[3.0, 5.0], "sq");
                bp(d, &[4.0, 1.0, 6.0], "free");
            },
            check_rects: true,
        },
        Case {
            name: "par_cex_half",
            r: "par(cex = 0.5)\nbp(c(3, 5, 2), 'small')",
            run: |d| {
                d.par.cex = 0.5;
                bp(d, &[3.0, 5.0, 2.0], "small");
            },
            check_rects: true,
        },
        Case {
            name: "mfrow_2x2_plot",
            r: "par(mfrow = c(2, 2))\n\
                for (k in 1:4) plot(1:10, (1:10)^2, type = 'n', main = paste('p', k), xlab = 'x', ylab = 'y')",
            run: |d| {
                d.mfrow(2, 2);
                let x: Vec<f64> = (1..=10).map(f64::from).collect();
                let y: Vec<f64> = x.iter().map(|v| v * v).collect();
                for k in 1..=4 {
                    let title = format!("p {k}");
                    let o = PlotOptions {
                        type_: PlotType::None,
                        main: Some(&title),
                        xlab: Some("x"),
                        ylab: Some("y"),
                        ..Default::default()
                    };
                    plot(d, &x, &y, &o);
                }
            },
            check_rects: false,
        },
    ]
}

#[test]
fn layout_matches_r() {
    if !rscript_available() {
        eprintln!("Rscript not found on PATH; skipping R cross-check.");
        return;
    }
    let cases = cases();
    let dir = std::env::temp_dir();
    let mut script = String::from(R_PRELUDE);
    for c in &cases {
        script.push_str(&format!(
            "pdf(file = '{}/rs_graphics_layout_{}.pdf', width = 8, height = 6, compress = FALSE)\n\
             {}\ninvisible(dev.off())\n",
            dir.display(),
            c.name,
            c.r
        ));
    }
    let script_path = dir.join("rs_graphics_layout.R");
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

    for c in &cases {
        let path = dir.join(format!("rs_graphics_layout_{}.pdf", c.name));
        let r_pdf = String::from_utf8_lossy(&std::fs::read(&path).unwrap()).into_owned();

        let mut dev = Device::new(PdfBackend::new(PAGE_W, PAGE_H), Par::default());
        (c.run)(&mut dev);
        let ours = String::from_utf8_lossy(&dev.into_backend().finish()).into_owned();

        let (r_t, our_t) = (text_origins(&r_pdf), text_origins(&ours));
        assert_eq!(
            our_t.len(),
            r_t.len(),
            "{}: text item count\nours={our_t:?}\nr={r_t:?}",
            c.name
        );
        for (a, b) in our_t.iter().zip(&r_t) {
            assert!(
                (a.0 - b.0).abs() < 0.05 && (a.1 - b.1).abs() < 0.05,
                "{}: text origin differs: ours={a:?} r={b:?}\nours={our_t:?}\nr={r_t:?}",
                c.name
            );
        }

        if c.check_rects {
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
}
