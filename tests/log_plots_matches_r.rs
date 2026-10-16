//! Cross-checks `abline()`, `hist()` (incl. `add = TRUE`) and `barplot()`
//! on log axes, `par(lab)`, `plot(xaxt = "n")` and a horizontal
//! `boxplot()` with axes, against a real R `pdf()` device's uncompressed
//! output: every painted path in drawing order (operator and vertices,
//! `re` expanded to its corners) and every text item in order (origin and
//! string). Skipped if `Rscript` isn't available.
//!
//! `hist()` gets `ylab = "n"`: R's `pdf()` never applies the last kerning
//! pair of each first character (`devPS.c`'s `KPend` is inclusive but
//! looped over exclusively), e.g. "F r" in "Frequency", and we deliberately
//! don't copy that.

use std::process::Command;

use rs_graphics::backend::pdf::PdfBackend;
use rs_graphics::color::{Color, ColorSpec};
use rs_graphics::device::Device;
use rs_graphics::highlevel::{
    barplot, barplot_matrix, boxplot, hist, plot, BarplotOptions, BoxplotOptions, HistOptions,
    PlotOptions,
};
use rs_graphics::lty::LineType;
use rs_graphics::par::{AxisType, Par};

fn rscript_available() -> bool {
    Command::new("Rscript")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

const PAGE_W: f64 = 7.0;
const PAGE_H: f64 = 6.0;

type Dev = Device<PdfBackend>;

/// Data shared with R (`hx` there).
fn hist_data() -> Vec<f64> {
    (0..60)
        .map(|i| 100.0 + 10.0 * ((i as f64) * 0.37).sin() + 0.1 * i as f64)
        .collect()
}

struct Case {
    name: &'static str,
    r: &'static str,
    draw: fn(&mut Dev),
}

fn solid() -> LineType {
    LineType::Pattern(&[])
}

fn red() -> ColorSpec {
    ColorSpec::Color(Color::parse("red").unwrap())
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "abline_loglog",
            r: "plot(c(1, 1000), c(1, 1e4), log = 'xy', xlab = 'x', ylab = 'y')\n\
                abline(h = 10); abline(v = 100, col = 'red'); abline(0, 1)\n\
                abline(a = 5, b = 2, untf = TRUE, lty = 2)\n\
                abline(h = -1); abline(a = -50, b = 3, untf = TRUE, col = 'red')",
            draw: |d| {
                let opts = PlotOptions {
                    xlog: true,
                    ylog: true,
                    xlab: Some("x"),
                    ylab: Some("y"),
                    ..PlotOptions::default()
                };
                plot(d, &[1.0, 1000.0], &[1.0, 1e4], &opts);
                let black = ColorSpec::Index(1);
                d.abline(None, None, Some(10.0), None, black, 1.0, solid());
                d.abline(None, None, None, Some(100.0), red(), 1.0, solid());
                d.abline(Some(0.0), Some(1.0), None, None, black, 1.0, solid());
                d.abline_untf(5.0, 2.0, black, 1.0, LineType::from_code(2));
                d.abline(None, None, Some(-1.0), None, black, 1.0, solid());
                d.abline_untf(-50.0, 3.0, red(), 1.0, solid());
            },
        },
        Case {
            name: "abline_logx_xpd",
            r: "plot(c(1, 1000), c(0, 10), log = 'x', xlab = 'x', ylab = 'y')\n\
                abline(1, 2); abline(a = 1, b = 0.01, untf = TRUE)\n\
                par(xpd = TRUE); abline(h = 5, v = 10, col = 'red')",
            draw: |d| {
                let opts = PlotOptions {
                    xlog: true,
                    xlab: Some("x"),
                    ylab: Some("y"),
                    ..PlotOptions::default()
                };
                plot(d, &[1.0, 1000.0], &[0.0, 10.0], &opts);
                let black = ColorSpec::Index(1);
                d.abline(Some(1.0), Some(2.0), None, None, black, 1.0, solid());
                d.abline_untf(1.0, 0.01, black, 1.0, solid());
                d.par.xpd = true;
                d.abline(None, None, Some(5.0), Some(10.0), red(), 1.0, solid());
            },
        },
        Case {
            name: "hist_logx",
            r: "hist(hx, log = 'x', ylab = 'n')",
            draw: |d| {
                let o = HistOptions { xlog: true, main: Some("Histogram of hx"), xlab: Some("hx"), ylab: Some("n"), ..Default::default() };
                hist(d, &hist_data(), &o);
            },
        },
        Case {
            name: "hist_logy",
            r: "suppressWarnings(hist(hx, log = 'y', ylab = 'n'))",
            draw: |d| {
                let o = HistOptions { ylog: true, main: Some("Histogram of hx"), xlab: Some("hx"), ylab: Some("n"), ..Default::default() };
                hist(d, &hist_data(), &o);
            },
        },
        Case {
            name: "hist_add",
            r: "hist(hx, log = 'x', ylab = 'n')\n\
                hist(hx * 1.05, add = TRUE, col = 'pink', border = 'red')",
            draw: |d| {
                let o = HistOptions { xlog: true, main: Some("Histogram of hx"), xlab: Some("hx"), ylab: Some("n"), ..Default::default() };
                hist(d, &hist_data(), &o);
                let more: Vec<f64> = hist_data().iter().map(|v| v * 1.05).collect();
                let o = HistOptions {
                    add: true,
                    col: ColorSpec::Color(Color::parse("pink").unwrap()),
                    border: red(),
                    ..Default::default()
                };
                hist(d, &more, &o);
            },
        },
        Case {
            name: "barplot_logy",
            r: "barplot(c(3, 40, 700, 12), names.arg = c('a', 'b', 'c', 'd'), log = 'y')",
            draw: |d| {
                let o = BarplotOptions {
                    names: Some(["a", "b", "c", "d"].map(String::from).to_vec()),
                    ylog: true,
                    ..Default::default()
                };
                barplot(d, &[3.0, 40.0, 700.0, 12.0], &o);
            },
        },
        Case {
            name: "barplot_stacked_logy",
            // No legend: on a log axis R misplaces it (log10 `usr` values
            // used as data), which we deliberately don't copy.
            r: "barplot(cbind(c(2, 5), c(30, 60), c(400, 100)), log = 'y')",
            draw: |d| {
                let o = BarplotOptions { ylog: true, ..Default::default() };
                barplot_matrix(d, &[vec![2.0, 5.0], vec![30.0, 60.0], vec![400.0, 100.0]], &o);
            },
        },
        Case {
            name: "barplot_horiz_beside_logx",
            r: "barplot(cbind(c(2, 5), c(30, 60)), beside = TRUE, horiz = TRUE, \
                        log = 'x', xlim = c(1, 100))",
            draw: |d| {
                let o = BarplotOptions {
                    beside: true,
                    horiz: true,
                    xlog: true,
                    xlim: Some((1.0, 100.0)),
                    ..Default::default()
                };
                barplot_matrix(d, &[vec![2.0, 5.0], vec![30.0, 60.0]], &o);
            },
        },
        Case {
            name: "plot_xaxt",
            r: "plot(1:10, xaxt = 'n', xlab = 'x', ylab = 'y')\n\
                axis(1, at = c(2, 5, 8), labels = c('a', 'b', 'c'))",
            draw: |d| {
                let x: Vec<f64> = (1..=10).map(f64::from).collect();
                let o = PlotOptions {
                    xaxt: Some(AxisType::None),
                    xlab: Some("x"),
                    ylab: Some("y"),
                    ..Default::default()
                };
                plot(d, &x, &x, &o);
                let fg = ColorSpec::Index(1);
                let labels = ["a", "b", "c"].map(String::from).to_vec();
                d.axis(1, Some(vec![2.0, 5.0, 8.0]), Some(labels), fg, 1.0, 1, 0);
            },
        },
        Case {
            name: "boxplot_horizontal",
            r: "boxplot(list(c(1, 2, 2.5, 3, 7), c(2, 3, 3.5, 4, 4.2, 5)), horizontal = TRUE, \
                        names = c('a', 'b'), main = 'H', xlab = 'value')",
            draw: |d| {
                let o = BoxplotOptions {
                    horizontal: true,
                    names: Some(vec!["a".into(), "b".into()]),
                    main: Some("H"),
                    xlab: Some("value"),
                    ..Default::default()
                };
                boxplot(d, &[vec![1.0, 2.0, 2.5, 3.0, 7.0], vec![2.0, 3.0, 3.5, 4.0, 4.2, 5.0]], &o);
            },
        },
        Case {
            name: "lab",
            r: "par(lab = c(10, 3, 7)); plot(c(0, 1), c(0, 1), xlab = 'x', ylab = 'y')\n\
                par(new = TRUE, lab = c(2, 12, 7)); \
                plot(c(1, 1e6), c(1, 50), log = 'xy', axes = FALSE, ann = FALSE); \
                axis(3); axis(4)",
            draw: |d| {
                d.par.lab = [10, 3, 7];
                let o = PlotOptions { xlab: Some("x"), ylab: Some("y"), ..Default::default() };
                plot(d, &[0.0, 1.0], &[0.0, 1.0], &o);
                d.set_new(true);
                d.par.lab = [2, 12, 7];
                let o = PlotOptions { xlog: true, ylog: true, axes: false, ..Default::default() };
                plot(d, &[1.0, 1e6], &[1.0, 50.0], &o);
                let fg = ColorSpec::Index(1);
                d.axis(3, None, None, fg, 1.0, 1, 0);
                d.axis(4, None, None, fg, 1.0, 1, 0);
            },
        },
    ]
}

#[derive(Debug)]
struct Painted {
    op: String,
    vertices: Vec<f64>,
}

#[derive(Debug)]
struct Text {
    string: String,
    x: f64,
    y: f64,
}

/// Painted paths (clipping paths excluded) and text items, in order.
fn extract(pdf: &str) -> (Vec<Painted>, Vec<Text>) {
    let mut painted = Vec::new();
    let mut texts = Vec::new();
    let mut path: Vec<f64> = Vec::new();
    let mut curved = false;
    let mut pending = false;
    let mut nums: Vec<f64> = Vec::new();
    for line in pdf.lines() {
        // A text matrix, then the string(s) shown (on the same line in R's
        // output, the next line in ours), kerning numbers dropped.
        if let Some(i) = line.find(" Tm") {
            let m: Vec<f64> = line[..i]
                .split_whitespace()
                .rev()
                .take(2)
                .filter_map(|t| t.parse().ok())
                .collect();
            if m.len() == 2 {
                texts.push(Text { string: String::new(), x: m[1], y: m[0] });
                pending = true;
            }
        }
        if pending && (line.contains("Tj") || line.contains("TJ")) {
            let start = line.find(" Tm").map_or(0, |i| i + 3);
            let mut depth = 0;
            let string = &mut texts.last_mut().unwrap().string;
            for c in line[start..].chars() {
                match c {
                    '(' => depth += 1,
                    ')' => depth -= 1,
                    _ if depth > 0 => string.push(c),
                    _ => {}
                }
            }
            pending = false;
            continue;
        }
        if line.contains(" Tm") {
            continue;
        }
        for t in line.split_whitespace() {
            if let Ok(v) = t.parse::<f64>() {
                nums.push(v);
                continue;
            }
            match t {
                "m" | "l" if nums.len() >= 2 => path.extend(&nums[nums.len() - 2..]),
                "re" if nums.len() >= 4 => {
                    // Normalised to a positive width and height (R writes
                    // some with a negative height).
                    let mut n = nums[nums.len() - 4..].to_vec();
                    for (p, d) in [(0, 2), (1, 3)] {
                        if n[d] < 0.0 {
                            n[p] += n[d];
                            n[d] = -n[d];
                        }
                    }
                    path.extend([
                        n[0],
                        n[1],
                        n[0] + n[2],
                        n[1],
                        n[0] + n[2],
                        n[1] + n[3],
                        n[0],
                        n[1] + n[3],
                    ]);
                }
                // Circles (points) are compared by operator only: R and we
                // start them at different points.
                "c" => curved = true,
                "n" => {
                    path.clear();
                    curved = false;
                }
                "S" | "s" | "f" | "F" | "f*" | "B" | "B*" | "b" | "b*" => {
                    let mut vertices = std::mem::take(&mut path);
                    if std::mem::take(&mut curved) {
                        vertices.clear();
                    }
                    painted.push(Painted {
                        op: t.to_string(),
                        vertices,
                    });
                }
                _ => {}
            }
            nums.clear();
        }
    }
    (painted, texts)
}

fn r_script(c: &Case, path: &str) -> String {
    format!(
        "pdf(file = '{path}', width = {PAGE_W}, height = {PAGE_H}, compress = FALSE)\n{}\n\
         invisible(dev.off())\n",
        c.r
    )
}

#[test]
fn log_plots_match_r() {
    if !rscript_available() {
        eprintln!("Rscript not found on PATH; skipping R cross-check.");
        return;
    }
    let dir = std::env::temp_dir();
    let cases = cases();
    let hx: Vec<String> = hist_data().iter().map(|v| format!("{v:e}")).collect();
    let mut script = format!("hx <- c({})\n", hx.join(", "));
    for c in &cases {
        let path = dir.join(format!("rs_graphics_log_plots_{}.pdf", c.name));
        script.push_str(&r_script(c, &path.display().to_string()));
    }
    let script_path = dir.join("rs_graphics_log_plots.R");
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
        let path = dir.join(format!("rs_graphics_log_plots_{}.pdf", c.name));
        let (r_paths, r_texts) = extract(&String::from_utf8_lossy(&std::fs::read(&path).unwrap()));
        let mut dev = Device::new(
            PdfBackend::new(PAGE_W, PAGE_H).with_compression(false),
            Par::default(),
        );
        (c.draw)(&mut dev);
        let (our_paths, our_texts) =
            extract(&String::from_utf8_lossy(&dev.into_backend().finish()));
        let name = c.name;

        let summary = |p: &[Painted]| {
            p.iter()
                .map(|p| format!("{} {:?}", p.op, p.vertices.iter().map(|v| v.round()).collect::<Vec<_>>()))
                .collect::<Vec<_>>()
                .join("\n")
        };
        assert_eq!(
            our_paths.len(),
            r_paths.len(),
            "{name}: painted path count\nours=\n{}\nr=\n{}",
            summary(&our_paths),
            summary(&r_paths)
        );
        for (k, (a, b)) in our_paths.iter().zip(&r_paths).enumerate() {
            let ctx = format!("{name}, path {k}:\n ours={a:?}\n r=   {b:?}");
            assert_eq!(a.op, b.op, "operator differs. {ctx}");
            assert!(
                a.vertices.len() == b.vertices.len()
                    && a.vertices.iter().zip(&b.vertices).all(|(p, q)| (p - q).abs() < 0.05),
                "vertices differ. {ctx}"
            );
        }
        assert_eq!(
            our_texts.len(),
            r_texts.len(),
            "{name}: text count\nours={our_texts:?}\nr={r_texts:?}"
        );
        for (a, b) in our_texts.iter().zip(&r_texts) {
            assert_eq!(a.string, b.string, "{name}: text differs");
            // R's `pdf()` draws `-` as the wider `/minus` glyph, which we
            // deliberately don't.
            if a.string.contains('-') {
                continue;
            }
            assert!(
                (a.x - b.x).abs() < 0.05 && (a.y - b.y).abs() < 0.05,
                "{name}: text origin differs: ours={a:?} r={b:?}"
            );
        }
    }
}
