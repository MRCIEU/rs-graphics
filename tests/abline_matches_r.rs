//! Cross-checks `abline()` with vectors of `h`/`v` and recycled `col`/
//! `lty`/`lwd`, and `coef =`, against a real R `pdf()` device's uncompressed output: every
//! stroked path in drawing order (vertices, colour, width, dash pattern),
//! on linear and log axes, and dashes with square and butt line ends. Skipped if `Rscript` isn't available.

use std::process::Command;

use rs_graphics::backend::LineEnd;
use rs_graphics::backend::pdf::PdfBackend;
use rs_graphics::color::{Color, ColorSpec};
use rs_graphics::device::{AblineOptions, Device};
use rs_graphics::lty::LineType;
use rs_graphics::par::Par;
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

fn col(names: &[&str]) -> Vec<ColorSpec> {
    names
        .iter()
        .map(|n| ColorSpec::Color(Color::parse(n).unwrap()))
        .collect()
}

struct Case {
    name: &'static str,
    /// R code after `plot.new()`/`plot.window()`.
    r: &'static str,
    log: &'static str,
    lims: ((f64, f64), (f64, f64)),
    draw: fn(&mut Device<PdfBackend>),
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "vectors",
            r: "abline(h = c(2, 5, 8), v = c(3, 7), col = c('red', 'blue'), \
                       lty = 1:3, lwd = c(1, 2))",
            log: "",
            lims: ((0.0, 10.0), (0.0, 10.0)),
            draw: |d| {
                d.abline(&AblineOptions {
                    h: vec![2.0, 5.0, 8.0],
                    v: vec![3.0, 7.0],
                    col: col(&["red", "blue"]),
                    lty: (1..=3).map(LineType::from_code).collect(),
                    lwd: vec![1.0, 2.0],
                    ..Default::default()
                });
            },
        },
        Case {
            name: "coef_first",
            r: "abline(a = 1, b = 0.5, h = c(4, 6), v = 9, \
                       col = c('darkgreen', 'orange', 'purple'), lwd = 3)",
            log: "",
            lims: ((0.0, 10.0), (0.0, 10.0)),
            draw: |d| {
                d.abline(&AblineOptions {
                    coef: Some((1.0, 0.5)),
                    h: vec![4.0, 6.0],
                    v: vec![9.0],
                    col: col(&["darkgreen", "orange", "purple"]),
                    lwd: vec![3.0],
                    ..Default::default()
                });
            },
        },
        Case {
            name: "coef",
            r: "x <- 1:10; y <- c(2.1, 2.9, 4.2, 4.8, 6.3, 6.9, 8.1, 9.2, 9.8, 11.4)\n\
                abline(coef = coef(lm(y ~ x)), col = 'red')\n\
                abline(coef = c(2, -0.3, 99), lty = 2)",
            log: "",
            lims: ((0.0, 10.0), (0.0, 12.0)),
            draw: |d| {
                // Least squares, as `lm(y ~ x)`.
                let x: Vec<f64> = (1..=10).map(f64::from).collect();
                let y = [2.1, 2.9, 4.2, 4.8, 6.3, 6.9, 8.1, 9.2, 9.8, 11.4];
                let (mx, my) = (x.iter().sum::<f64>() / 10.0, y.iter().sum::<f64>() / 10.0);
                let sxy: f64 = x.iter().zip(&y).map(|(a, b)| (a - mx) * (b - my)).sum();
                let sxx: f64 = x.iter().map(|a| (a - mx) * (a - mx)).sum();
                let slope = sxy / sxx;
                let red = col(&["red"])[0];
                let fit = AblineOptions {
                    coef: Some((my - slope * mx, slope)),
                    col: vec![red],
                    ..AblineOptions::default()
                };
                d.abline(&fit);
                // R ignores coef values after the first two.
                let first_two = AblineOptions {
                    coef: Some((2.0, -0.3)),
                    lty: vec![LineType::DASHED],
                    ..AblineOptions::default()
                };
                d.abline(&first_two);
            },
        },
        Case {
            name: "line_ends",
            r: "par(lend = 'square'); abline(h = c(2, 4), lty = c(2, 3), lwd = c(1, 3))\n\
                par(lend = 'butt'); abline(h = c(6, 8), lty = c('dotdash', '42'), lwd = 2)",
            log: "",
            lims: ((0.0, 10.0), (0.0, 10.0)),
            draw: |d| {
                d.par.lend = LineEnd::Square;
                d.abline(&AblineOptions {
                    h: vec![2.0, 4.0],
                    lty: vec![LineType::from_code(2), LineType::from_code(3)],
                    lwd: vec![1.0, 3.0],
                    ..Default::default()
                });
                d.par.lend = LineEnd::Butt;
                d.abline(&AblineOptions {
                    h: vec![6.0, 8.0],
                    lty: vec![LineType::from_code(4), LineType::parse("42").unwrap()],
                    lwd: vec![2.0],
                    ..Default::default()
                });
            },
        },
        Case {
            name: "par_defaults_log",
            r: "par(col = 'grey40', lty = 2, lwd = 2)\n\
                abline(h = c(1, 10, 100), v = c(2, 20))\n\
                abline(a = 1, b = 1, untf = TRUE, h = 50, col = c('red', 'blue'))",
            log: "xy",
            lims: ((1.0, 100.0), (0.5, 500.0)),
            draw: |d| {
                d.par.col = Color::parse("grey40").unwrap();
                d.par.lty = LineType::from_code(2);
                d.par.lwd = 2.0;
                d.abline(&AblineOptions {
                    h: vec![1.0, 10.0, 100.0],
                    v: vec![2.0, 20.0],
                    ..Default::default()
                });
                d.abline(&AblineOptions {
                    coef: Some((1.0, 1.0)),
                    untf: true,
                    h: vec![50.0],
                    col: col(&["red", "blue"]),
                    ..Default::default()
                });
            },
        },
    ]
}

#[derive(Clone, Debug)]
struct Gs {
    stroke: [f64; 3],
    lwd: f64,
    dash: Vec<f64>,
}

#[derive(Debug)]
struct Stroked {
    vertices: Vec<f64>,
    colour: [f64; 3],
    lwd: f64,
    dash: Vec<f64>,
}

fn interpret(pdf: &str) -> Vec<Stroked> {
    let mut out = Vec::new();
    let mut gs = Gs {
        stroke: [0.0; 3],
        lwd: 1.0,
        dash: Vec::new(),
    };
    let mut stack = Vec::new();
    let mut nums: Vec<f64> = Vec::new();
    let mut array: Option<Vec<f64>> = None;
    let mut path: Vec<f64> = Vec::new();
    let mut rest = pdf;
    while let Some(i) = rest.find("stream\n") {
        let content = &rest[i + 7..];
        let j = content.find("endstream").unwrap_or(content.len());
        let content_str = &content[..j];
        rest = &content[j..];
        if !content_str.contains(" m") {
            continue;
        }
        for t in content_str
            .replace('[', " [ ")
            .replace(']', " ] ")
            .split_whitespace()
        {
            if let Ok(v) = t.parse::<f64>() {
                match &mut array {
                    Some(a) => a.push(v),
                    None => nums.push(v),
                }
                continue;
            }
            let last = |k: usize| nums[nums.len() - k..].to_vec();
            match t {
                "[" => array = Some(Vec::new()),
                "]" => {
                    gs.dash = array.take().unwrap_or_default();
                    continue;
                }
                "q" => stack.push(gs.clone()),
                "Q" => gs = stack.pop().unwrap(),
                "RG" | "SCN" => gs.stroke = last(3).try_into().unwrap(),
                "G" => gs.stroke = [nums[nums.len() - 1]; 3],
                "w" => gs.lwd = nums[nums.len() - 1],
                "m" | "l" => path.extend(last(2)),
                "n" | "f" | "B" | "b" => path.clear(),
                "S" => out.push(Stroked {
                    vertices: std::mem::take(&mut path),
                    colour: gs.stroke,
                    lwd: gs.lwd,
                    dash: gs.dash.clone(),
                }),
                _ => {}
            }
            nums.clear();
        }
    }
    out
}

fn close(a: &[f64], b: &[f64], tol: f64) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < tol)
}

#[test]
fn abline_matches_r() {
    if !rscript_available() {
        eprintln!("Rscript not found on PATH; skipping R cross-check.");
        return;
    }
    let dir = std::env::temp_dir();
    let cases = cases();
    let mut script = String::new();
    for c in &cases {
        let path = dir.join(format!("rs_graphics_abline_{}.pdf", c.name));
        let ((x0, x1), (y0, y1)) = c.lims;
        script.push_str(&format!(
            "pdf(file = '{}', width = {PAGE_W}, height = {PAGE_H}, compress = FALSE)\n\
             plot.new(); plot.window(c({x0}, {x1}), c({y0}, {y1}), log = '{}')\n{}\n\
             invisible(dev.off())\n",
            path.display(),
            c.log,
            c.r
        ));
    }
    let script_path = dir.join("rs_graphics_abline.R");
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
        let path = dir.join(format!("rs_graphics_abline_{}.pdf", c.name));
        let r = interpret(&String::from_utf8_lossy(&std::fs::read(&path).unwrap()));
        let mut dev = Device::new(
            PdfBackend::new(PAGE_W, PAGE_H).with_compression(false),
            Par::default(),
        );
        dev.plot_new();
        let (xl, yl) = c.lims;
        dev.plot_window(
            xl,
            yl,
            &PlotWindowOptions {
                xlog: c.log.contains('x'),
                ylog: c.log.contains('y'),
                ..PlotWindowOptions::default()
            },
        );
        (c.draw)(&mut dev);
        let ours = interpret(&String::from_utf8_lossy(&dev.into_backend().finish()));
        let name = c.name;

        assert!(!r.is_empty(), "{name}: nothing from R");
        assert_eq!(ours.len(), r.len(), "{name}: line count\nours={ours:#?}\nr={r:#?}");
        for (k, (a, b)) in ours.iter().zip(&r).enumerate() {
            let ctx = format!("{name}, line {k}:\n ours={a:?}\n r=   {b:?}");
            assert!(close(&a.vertices, &b.vertices, 0.05), "vertices differ. {ctx}");
            assert!(close(&a.colour, &b.colour, 0.005), "colour differs. {ctx}");
            // R writes widths to 2 decimals.
            assert!((a.lwd - b.lwd).abs() < 0.0051, "width differs. {ctx}");
            assert!(close(&a.dash, &b.dash, 0.005), "dash differs. {ctx}");
        }
    }
}

#[test]
#[should_panic(expected = "'a' and 'b' must be finite")]
fn abline_coef_must_be_finite() {
    let mut dev = Device::new(PdfBackend::new(PAGE_W, PAGE_H), Par::default());
    dev.plot_new();
    dev.plot_window((0.0, 1.0), (0.0, 1.0), &PlotWindowOptions::default());
    dev.abline(&AblineOptions {
        coef: Some((f64::NAN, 1.0)),
        ..AblineOptions::default()
    });
}
