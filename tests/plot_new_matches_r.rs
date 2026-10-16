//! Cross-checks the user coordinates `plot.new()` leaves without a
//! `plot.window()` call (R's `C_plot_new`: linear axes, `GScale(0, 1)` on
//! both, so -0.04..1.04 with `xaxs = "r"`), including after a log plot and
//! with `xaxs = yaxs = "i"`. Compares every stroked line, the bounding box
//! of every curved path (the points) and every label (string and origin)
//! with an uncompressed R `pdf()`. Skipped if `Rscript` isn't available.

use std::process::Command;

use rs_graphics::backend::pdf::PdfBackend;
use rs_graphics::color::ColorSpec;
use rs_graphics::device::{AxisOptions, Device};
use rs_graphics::par::{AxisStyle, Par};
use rs_graphics::prelude::*;

fn rscript_available() -> bool {
    Command::new("Rscript")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

const PAGE_W: f64 = 6.0;
const PAGE_H: f64 = 5.0;

struct Case {
    name: &'static str,
    r: &'static str,
    draw: fn(&mut Device<PdfBackend>),
}

fn axes(d: &mut Device<PdfBackend>, sides: &[i32]) {
    for &s in sides {
        d.axis(s, &AxisOptions::default());
    }
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "default",
            r: "plot.new(); points(c(0, 0.5, 1), c(0, 0.5, 1)); axis(1); axis(2)",
            draw: |d| {
                d.plot_new();
                let v = [0.0, 0.5, 1.0];
                d.points(
                    &v,
                    &v,
                    &XyOptions {
                        pch: vec![1],
                        col: vec![ColorSpec::Index(1)],
                        ..XyOptions::default()
                    },
                );
                axes(d, &[1, 2]);
            },
        },
        Case {
            name: "after_log",
            r: "plot.new(); plot.window(c(1, 1000), c(1, 1000), log = 'xy'); axis(1); axis(2)\n\
                par(new = TRUE); plot.new(); axis(3); axis(4)",
            draw: |d| {
                d.plot_new();
                d.plot_window(
                    (1.0, 1000.0),
                    (1.0, 1000.0),
                    &PlotWindowOptions {
                        xlog: true,
                        ylog: true,
                        ..PlotWindowOptions::default()
                    },
                );
                axes(d, &[1, 2]);
                d.set_new(true);
                d.plot_new();
                axes(d, &[3, 4]);
            },
        },
        Case {
            name: "xaxs_i",
            r: "par(xaxs = 'i', yaxs = 'i'); plot.new(); points(0.25, 0.75); axis(1); axis(2)",
            draw: |d| {
                d.par.xaxs = AxisStyle::Exact;
                d.par.yaxs = AxisStyle::Exact;
                d.plot_new();
                d.points(
                    &[0.25],
                    &[0.75],
                    &XyOptions {
                        pch: vec![1],
                        col: vec![ColorSpec::Index(1)],
                        ..XyOptions::default()
                    },
                );
                axes(d, &[1, 2]);
            },
        },
    ]
}

#[derive(Debug, Default)]
struct Extracted {
    /// Vertices of stroked paths without curves.
    lines: Vec<Vec<f64>>,
    /// Bounding boxes (all points, incl. control points) of curved paths.
    curves: Vec<[f64; 4]>,
    labels: Vec<(String, f64, f64)>,
}

fn extract(pdf: &str) -> Extracted {
    let mut out = Extracted::default();
    let mut path: Vec<f64> = Vec::new();
    let mut curved = false;
    let mut nums: Vec<f64> = Vec::new();
    let mut origin: Option<(f64, f64)> = None;
    for line in pdf.lines() {
        if let Some(i) = line.find(" Tm") {
            let m: Vec<f64> = line[..i]
                .split_whitespace()
                .rev()
                .take(2)
                .filter_map(|t| t.parse().ok())
                .collect();
            origin = (m.len() == 2).then(|| (m[1], m[0]));
        }
        if let (Some((x, y)), Some(a), Some(b)) = (origin, line.find('('), line.rfind(')')) {
            if line.contains("Tj") {
                out.labels.push((line[a + 1..b].to_string(), x, y));
                origin = None;
                continue;
            }
        }
        for t in line.split_whitespace() {
            if let Ok(v) = t.parse::<f64>() {
                nums.push(v);
                continue;
            }
            match t {
                "m" | "l" if nums.len() >= 2 => path.extend(&nums[nums.len() - 2..]),
                "c" if nums.len() >= 6 => {
                    path.extend(&nums[nums.len() - 6..]);
                    curved = true;
                }
                "S" | "s" | "f" | "B" | "b" => {
                    if curved {
                        let xs = path.iter().step_by(2);
                        let ys = path.iter().skip(1).step_by(2);
                        let (min, max) = (f64::INFINITY, f64::NEG_INFINITY);
                        out.curves.push([
                            xs.clone().fold(min, |a, &b| a.min(b)),
                            xs.fold(max, |a, &b| a.max(b)),
                            ys.clone().fold(min, |a, &b| a.min(b)),
                            ys.fold(max, |a, &b| a.max(b)),
                        ]);
                    } else if t == "S" {
                        out.lines.push(path.clone());
                    }
                    path.clear();
                    curved = false;
                }
                "n" | "re" => {
                    path.clear();
                    curved = false;
                }
                _ => {}
            }
            nums.clear();
        }
    }
    out
}

fn close(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 0.05)
}

#[test]
fn plot_new_coordinates_match_r() {
    if !rscript_available() {
        eprintln!("Rscript not found on PATH; skipping R cross-check.");
        return;
    }
    let dir = std::env::temp_dir();
    let cases = cases();
    let mut script = String::new();
    for c in &cases {
        let path = dir.join(format!("rs_graphics_plot_new_{}.pdf", c.name));
        script.push_str(&format!(
            "pdf(file = '{}', width = {PAGE_W}, height = {PAGE_H}, compress = FALSE)\n{}\n\
             invisible(dev.off())\n",
            path.display(),
            c.r
        ));
    }
    let script_path = dir.join("rs_graphics_plot_new.R");
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
        let path = dir.join(format!("rs_graphics_plot_new_{}.pdf", c.name));
        let r = extract(&String::from_utf8_lossy(&std::fs::read(&path).unwrap()));
        let mut dev = Device::new(
            PdfBackend::new(PAGE_W, PAGE_H).with_compression(false),
            Par::default(),
        );
        (c.draw)(&mut dev);
        let ours = extract(&String::from_utf8_lossy(&dev.into_backend().finish()));
        let name = c.name;

        assert!(!r.lines.is_empty(), "{name}: no lines from R");
        assert_eq!(ours.lines.len(), r.lines.len(), "{name}: line count\nours={ours:#?}\nr={r:#?}");
        for (a, b) in ours.lines.iter().zip(&r.lines) {
            assert!(close(a, b), "{name}: line differs: ours={a:?} r={b:?}");
        }
        assert_eq!(ours.curves.len(), r.curves.len(), "{name}: point count");
        for (a, b) in ours.curves.iter().zip(&r.curves) {
            assert!(close(a, b), "{name}: point differs: ours={a:?} r={b:?}");
        }
        let strings = |l: &[(String, f64, f64)]| l.iter().map(|t| t.0.clone()).collect::<Vec<_>>();
        assert_eq!(strings(&ours.labels), strings(&r.labels), "{name}: labels");
        for (a, b) in ours.labels.iter().zip(&r.labels) {
            assert!(close(&[a.1, a.2], &[b.1, b.2]), "{name}: label origin: ours={a:?} r={b:?}");
        }
    }
}
