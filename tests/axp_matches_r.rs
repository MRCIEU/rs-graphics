//! Cross-checks `par(xaxp/yaxp)` against R: the values `plot.window()`
//! sets (linear, log with each tick-rule code, reversed, tiny ranges,
//! `par(lab)`), and the ticks `axis()` draws when they are changed
//! afterwards, overridden inline, or passed to `plot()` (labels with their
//! origins, and every two-point stroke, vs. an uncompressed R `pdf()`).
//! Skipped if `Rscript` isn't available.

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

/// Runs `script` from its own file (the tests run in parallel).
fn rscript(name: &str, script: &str) -> String {
    let path = std::env::temp_dir().join(format!("rs_graphics_axp_{name}.R"));
    std::fs::write(&path, script).unwrap();
    let out = Command::new("Rscript").arg(&path).output().expect("run Rscript");
    assert!(
        out.status.success(),
        "Rscript failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// `(xlim, ylim, log, lab)`.
type Window = ((f64, f64), (f64, f64), &'static str, [i32; 2]);

/// Windows whose `xaxp`/`yaxp` are compared.
const WINDOWS: &[Window] = &[
    ((0.0, 10.0), (-3.0, 7.5), "", [5, 5]),
    ((10.0, 0.0), (0.001, 0.0013), "", [5, 5]),
    ((1.0, 1e6), (3.0, 700.0), "xy", [5, 5]),
    ((1.0, 1000.0), (2.0, 15.0), "xy", [5, 5]),
    ((1e-5, 1e5), (1e4, 1.0), "xy", [5, 5]),
    ((0.0, 1.0), (1.0, 1e6), "y", [10, 3]),
    ((5.0, 5.0 + 1e-14), (100.0, 100.0 * (1.0 + 1e-14)), "y", [5, 5]),
];

#[test]
fn axp_values_match_r() {
    if !rscript_available() {
        eprintln!("Rscript not found on PATH; skipping R cross-check.");
        return;
    }
    let mut script = String::from("pdf(NULL)\n");
    for (xlim, ylim, log, lab) in WINDOWS {
        script.push_str(&format!(
            "par(lab = c({}, {}, 7)); plot.new()\n\
             suppressWarnings(plot.window(c({:e}, {:e}), c({:e}, {:e}), log = '{log}'))\n\
             cat(sprintf('%.17g', c(par('xaxp'), par('yaxp'))), '\\n')\n",
            lab[0], lab[1], xlim.0, xlim.1, ylim.0, ylim.1
        ));
    }
    let r = rscript("values", &script);
    assert_eq!(r.lines().count(), WINDOWS.len(), "R output: {r}");
    for (line, (xlim, ylim, log, lab)) in r.lines().zip(WINDOWS) {
        let want: Vec<f64> = line.split_whitespace().map(|t| t.parse().unwrap()).collect();
        let par = Par {
            lab: [lab[0], lab[1], 7],
            ..Par::default()
        };
        let mut dev = Device::new(PdfBackend::new(PAGE_W, PAGE_H), par);
        dev.plot_new();
        dev.plot_window(*xlim, *ylim, log.contains('x'), log.contains('y'));
        let ours: Vec<f64> = dev.par.xaxp.iter().chain(&dev.par.yaxp).copied().collect();
        let ok = ours
            .iter()
            .zip(&want)
            .all(|(a, b)| (a - b).abs() <= 1e-12 * b.abs().max(1e-300));
        assert!(ok, "xlim {xlim:?} ylim {ylim:?} log '{log}': ours {ours:?}, R {want:?}");
    }
}

struct Case {
    name: &'static str,
    r: &'static str,
    draw: fn(&mut Device<PdfBackend>),
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "par_after_window",
            r: "plot.new(); plot.window(c(0, 10), c(1, 1000), log = 'y')\n\
                par(xaxp = c(0, 10, 2), yaxp = c(1, 1000, 2))\n\
                axis(1); axis(2); axis(3, xaxp = c(2, 8, 3)); axis(4, yaxp = c(1, 100, 3))",
            draw: |d| {
                d.plot_new();
                d.plot_window((0.0, 10.0), (1.0, 1000.0), false, true);
                d.par.xaxp = [0.0, 10.0, 2.0];
                d.par.yaxp = [1.0, 1000.0, 2.0];
                d.axis_with(1, &AxisOptions::default());
                d.axis_with(2, &AxisOptions::default());
                let o = AxisOptions { axp: Some([2.0, 8.0, 3.0]), ..Default::default() };
                d.axis_with(3, &o);
                let o = AxisOptions { axp: Some([1.0, 100.0, 3.0]), ..Default::default() };
                d.axis_with(4, &o);
            },
        },
        Case {
            name: "plot_inline",
            r: "plot(1:10, xaxp = c(1, 10, 3), yaxp = c(2, 8, 6), xlab = 'x', ylab = 'y')",
            draw: |d| {
                let x: Vec<f64> = (1..=10).map(f64::from).collect();
                let o = PlotOptions {
                    xaxp: Some([1.0, 10.0, 3.0]),
                    yaxp: Some([2.0, 8.0, 6.0]),
                    xlab: Some("x".into()),
                    ylab: Some("y".into()),
                    ..Default::default()
                };
                plot(d, &x, &x, &o);
                // Like R, `par()` keeps what `plot.window()` computed.
                assert_eq!(d.par.xaxp, [2.0, 10.0, 4.0]);
            },
        },
    ]
}

/// A label's string and origin.
type Label = (String, f64, f64);

/// Labels in order, and every stroked two-point path.
fn extract(pdf: &str) -> (Vec<Label>, Vec<Vec<f64>>) {
    let mut labels = Vec::new();
    let mut segments = Vec::new();
    let mut path: Vec<f64> = Vec::new();
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
                labels.push((line[a + 1..b].to_string(), x, y));
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
                "S" => {
                    if path.len() == 4 {
                        segments.push(path.clone());
                    }
                    path.clear();
                }
                "n" | "f" | "B" | "b" | "s" | "re" => path.clear(),
                _ => {}
            }
            nums.clear();
        }
    }
    (labels, segments)
}

#[test]
fn axp_ticks_match_r() {
    if !rscript_available() {
        eprintln!("Rscript not found on PATH; skipping R cross-check.");
        return;
    }
    let dir = std::env::temp_dir();
    let cases = cases();
    let mut script = String::new();
    for c in &cases {
        let path = dir.join(format!("rs_graphics_axp_{}.pdf", c.name));
        script.push_str(&format!(
            "pdf(file = '{}', width = {PAGE_W}, height = {PAGE_H}, compress = FALSE)\n{}\n\
             invisible(dev.off())\n",
            path.display(),
            c.r
        ));
    }
    rscript("ticks", &script);
    for c in &cases {
        let path = dir.join(format!("rs_graphics_axp_{}.pdf", c.name));
        let (r_labels, r_segs) = extract(&String::from_utf8_lossy(&std::fs::read(&path).unwrap()));
        let mut dev = Device::new(
            PdfBackend::new(PAGE_W, PAGE_H).with_compression(false),
            Par::default(),
        );
        (c.draw)(&mut dev);
        let (labels, segs) = extract(&String::from_utf8_lossy(&dev.into_backend().finish()));
        let name = c.name;
        let strings = |l: &[Label]| l.iter().map(|t| t.0.clone()).collect::<Vec<_>>();
        assert_eq!(strings(&labels), strings(&r_labels), "{name}: labels");
        for (a, b) in labels.iter().zip(&r_labels) {
            assert!(
                (a.1 - b.1).abs() < 0.05 && (a.2 - b.2).abs() < 0.05,
                "{name}: label origin differs: ours={a:?} r={b:?}"
            );
        }
        assert_eq!(segs.len(), r_segs.len(), "{name}: segments\nours={segs:?}\nr={r_segs:?}");
        for (a, b) in segs.iter().zip(&r_segs) {
            assert!(
                a.iter().zip(b).all(|(p, q)| (p - q).abs() < 0.05),
                "{name}: segment differs: ours={a:?} r={b:?}"
            );
        }
    }
}
