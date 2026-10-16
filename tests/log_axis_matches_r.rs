//! Cross-checks log axes against a real R `pdf()` device's uncompressed
//! output: `plot(x, y, log = ...)` over ranges that hit each of R's log
//! tick rules (`GLPretty()` + `CreateAtVector()`: 1-2-5, 1-5, powers of
//! ten with decimation, a linear axis for small ranges, a reversed axis),
//! with non-positive values that must be dropped, plus explicit `at` in
//! data units. Compares every label (string and origin), every two-point
//! stroke (axis lines and ticks) and the number of points drawn. Skipped if
//! `Rscript` isn't available.

use std::process::Command;

use rs_graphics::backend::pdf::PdfBackend;
use rs_graphics::device::{AxisOptions, Device};
use rs_graphics::highlevel::{plot, PlotOptions};
use rs_graphics::par::Par;

fn rscript_available() -> bool {
    Command::new("Rscript")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

const PAGE_W: f64 = 7.0;
const PAGE_H: f64 = 6.0;
const N: usize = 20;

struct Case {
    name: &'static str,
    x: (f64, f64),
    y: (f64, f64),
    xlog: bool,
    ylog: bool,
    xlim: Option<(f64, f64)>,
    /// Extra `axis(3, at = ...)` call, in data units.
    at3: Option<&'static [f64]>,
}

/// `N` values spaced evenly in log10 (or linearly) from `r.0` to `r.1`,
/// after a 0 and a -1 that a log axis must drop.
fn values(r: (f64, f64), log: bool) -> Vec<f64> {
    let mut v = vec![0.0, -1.0];
    for i in 0..N {
        let t = i as f64 / (N - 1) as f64;
        v.push(if log {
            10f64.powf(r.0.log10() + t * (r.1.log10() - r.0.log10()))
        } else {
            r.0 + t * (r.1 - r.0)
        });
    }
    v
}

fn r_vec(v: &[f64]) -> String {
    let s: Vec<String> = v.iter().map(|x| format!("{x:e}")).collect();
    format!("c({})", s.join(", "))
}

fn cases() -> Vec<Case> {
    let c = |name, x, y, xlog, ylog| Case {
        name,
        x,
        y,
        xlog,
        ylog,
        xlim: None,
        at3: None,
    };
    vec![
        // y: 2.4 decades -> 1, 2, 5; x: 6 decades -> powers of ten.
        c("one_two_five", (1.0, 1e6), (3.0, 700.0), true, true),
        // x: 3 decades -> 1, 5; y linear.
        c("one_five", (1.0, 1000.0), (0.0, 10.0), true, false),
        // 10 decades -> every other power of ten.
        c("decimated", (1e-5, 1e5), (2.0, 15.0), true, true),
        // 200 decades -> a large step, starting further in.
        c("huge", (1e-100, 1e100), (0.02, 0.9), true, true),
        // y: 2..15 is under a decade -> linear pretty() ticks.
        c("small", (0.5, 4.0), (2.0, 15.0), false, true),
        Case {
            xlim: Some((1e4, 1.0)),
            at3: Some(&[1.0, 3.0, 30.0, 300.0, 3000.0, 1e5]),
            ..c("reversed_at", (1.0, 1e4), (1.0, 100.0), true, false)
        },
    ]
}

fn r_script(c: &Case, path: &str) -> String {
    let log = format!(
        "{}{}",
        if c.xlog { "x" } else { "" },
        if c.ylog { "y" } else { "" }
    );
    let xlim = c
        .xlim
        .map(|(a, b)| format!(", xlim = c({a:e}, {b:e})"))
        .unwrap_or_default();
    let at3 = c
        .at3
        .map(|a| format!("axis(3, at = {})\n", r_vec(a)))
        .unwrap_or_default();
    format!(
        "pdf(file = '{path}', width = {PAGE_W}, height = {PAGE_H}, compress = FALSE)\n\
         suppressWarnings(plot({x}, {y}, log = '{log}', xlab = 'x', ylab = 'y'{xlim}))\n\
         {at3}invisible(dev.off())\n",
        x = r_vec(&values(c.x, c.xlog)),
        y = r_vec(&values(c.y, c.ylog)),
    )
}

fn draw_ours(c: &Case) -> String {
    let mut dev = Device::new(
        PdfBackend::new(PAGE_W, PAGE_H).with_compression(false),
        Par::default(),
    );
    let opts = PlotOptions {
        xlog: c.xlog,
        ylog: c.ylog,
        xlim: c.xlim,
        xlab: Some("x"),
        ylab: Some("y"),
        ..PlotOptions::default()
    };
    plot(&mut dev, &values(c.x, c.xlog), &values(c.y, c.ylog), &opts);
    if let Some(at) = c.at3 {
        let o = AxisOptions {
            at: Some(at.to_vec()),
            ..Default::default()
        };
        dev.axis_with(3, &o);
    }
    String::from_utf8_lossy(&dev.into_backend().finish()).into_owned()
}

#[derive(Debug, PartialEq)]
struct Extracted {
    /// `(string, x, y)` of every label, sorted by position.
    labels: Vec<(String, f64, f64)>,
    /// Every stroked two-point path (`[x0, y0, x1, y1]`), in order.
    segments: Vec<Vec<f64>>,
    /// Number of curved paths (`pch = 1` circles).
    curves: usize,
}

fn extract(pdf: &str) -> Extracted {
    let mut labels = Vec::new();
    let mut segments = Vec::new();
    let mut curves = 0;
    let mut path: Vec<f64> = Vec::new();
    let mut curved = false;
    let mut nums: Vec<f64> = Vec::new();
    // Set by a text matrix, so strings in the document info don't count.
    let mut origin = None;
    for t in pdf.split_whitespace() {
        if let Ok(v) = t.parse::<f64>() {
            nums.push(v);
            continue;
        }
        match t {
            "m" | "l" if nums.len() >= 2 => path.extend(&nums[nums.len() - 2..]),
            "c" => curved = true,
            "Tm" if nums.len() >= 6 => origin = Some((nums[nums.len() - 2], nums[nums.len() - 1])),
            "S" | "s" | "f" | "B" | "b" | "n" => {
                if curved && t != "n" {
                    curves += 1;
                } else if t == "S" && path.len() == 4 {
                    segments.push(path.clone());
                }
                path.clear();
                curved = false;
            }
            _ if t.starts_with('(') && t.ends_with(")") => {
                if let Some((x, y)) = origin.take() {
                    labels.push((t[1..t.len() - 1].to_string(), x, y));
                }
            }
            _ => {}
        }
        nums.clear();
    }
    let key = |p: &(String, f64, f64)| ((p.1 * 10.0).round() as i64, (p.2 * 10.0).round() as i64);
    labels.sort_by_key(key);
    Extracted {
        labels,
        segments,
        curves,
    }
}

#[test]
fn log_axes_match_r() {
    if !rscript_available() {
        eprintln!("Rscript not found on PATH; skipping R cross-check.");
        return;
    }
    let dir = std::env::temp_dir();
    let cases = cases();
    let mut script = String::new();
    for c in &cases {
        let path = dir.join(format!("rs_graphics_log_axis_{}.pdf", c.name));
        script.push_str(&r_script(c, &path.display().to_string()));
    }
    let script_path = dir.join("rs_graphics_log_axis.R");
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
        let path = dir.join(format!("rs_graphics_log_axis_{}.pdf", c.name));
        let r = extract(&String::from_utf8_lossy(&std::fs::read(&path).unwrap()));
        let ours = extract(&draw_ours(c));
        let name = c.name;

        assert_eq!(ours.curves, r.curves, "{name}: points drawn");
        let strings = |e: &Extracted| e.labels.iter().map(|l| l.0.clone()).collect::<Vec<_>>();
        assert_eq!(strings(&ours), strings(&r), "{name}: labels");
        for (a, b) in ours.labels.iter().zip(&r.labels) {
            // R's `pdf()` draws `-` as the wider `/minus` glyph, which we
            // deliberately don't, so "1e-05" is laid out differently.
            if a.0.contains('-') {
                continue;
            }
            assert!(
                (a.1 - b.1).abs() < 0.05 && (a.2 - b.2).abs() < 0.05,
                "{name}: label origin differs: ours={a:?} r={b:?}"
            );
        }
        assert_eq!(
            ours.segments.len(),
            r.segments.len(),
            "{name}: segment count\nours={:?}\nr={:?}",
            ours.segments,
            r.segments
        );
        for (a, b) in ours.segments.iter().zip(&r.segments) {
            assert!(
                a.iter().zip(b).all(|(p, q)| (p - q).abs() < 0.05),
                "{name}: segment differs: ours={a:?} r={b:?}"
            );
        }
    }
}
