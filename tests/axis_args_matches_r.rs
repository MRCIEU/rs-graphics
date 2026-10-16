//! Cross-checks `axis()`'s arguments (`line`, `pos`, `outer`, `tck`/`tcl`,
//! `hadj`, `padj`, `gap.axis`, `tick`, `labels`, `lty`, `lwd`/`lwd.ticks`,
//! `col`/`col.ticks`/`col.axis`, `cex.axis`, `font`, `xaxt`/`yaxt`) and
//! its tick limits (reversed axes, `par(xpd = TRUE)`) against a real R `pdf()` device's
//! uncompressed output: every stroked path in drawing order (vertices,
//! colour, width, dash pattern) and every label (origin, direction,
//! colour). Skipped if `Rscript` isn't available.

use std::process::Command;

use rs_graphics::backend::pdf::PdfBackend;
use rs_graphics::color::{Color, ColorSpec};
use rs_graphics::device::{AxisOptions, Device};
use rs_graphics::lty::LineType;
use rs_graphics::par::{AxisType, Par, Sides};

fn rscript_available() -> bool {
    Command::new("Rscript")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

const PAGE_W: f64 = 7.0;
const PAGE_H: f64 = 6.0;

struct Case {
    name: &'static str,
    /// R code run after `plot.new()`/`plot.window()`.
    r: &'static str,
    /// `par()` settings for R, run before `plot.new()`.
    r_par: &'static str,
    xlim: (f64, f64),
    setup: fn(&mut Par),
    draw: fn(&mut Device<PdfBackend>),
}

fn col(name: &str) -> Option<ColorSpec> {
    Some(ColorSpec::Color(Color::parse(name).unwrap()))
}

fn seq(from: f64, to: f64, by: f64) -> Vec<f64> {
    let n = ((to - from) / by).round() as usize;
    (0..=n).map(|i| from + i as f64 * by).collect()
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "line",
            r_par: "",
            r: "axis(1, line = 1); axis(2, line = -1); \
                axis(3, line = 2, las = 2); axis(4, line = 0.5, las = 1)",
            xlim: (0.0, 10.0),
            setup: |_| {},
            draw: |d| {
                d.axis_with(1, &AxisOptions { line: Some(1.0), ..Default::default() });
                d.axis_with(2, &AxisOptions { line: Some(-1.0), ..Default::default() });
                let o = AxisOptions { line: Some(2.0), las: Some(2), ..Default::default() };
                d.axis_with(3, &o);
                let o = AxisOptions { line: Some(0.5), las: Some(1), ..Default::default() };
                d.axis_with(4, &o);
            },
        },
        Case {
            name: "pos",
            r_par: "",
            r: "axis(1, pos = 2); axis(2, pos = 3, las = 1); \
                axis(3, pos = 8, line = 3); axis(4, pos = 12)",
            xlim: (0.0, 10.0),
            setup: |_| {},
            draw: |d| {
                d.axis_with(1, &AxisOptions { pos: Some(2.0), ..Default::default() });
                let o = AxisOptions { pos: Some(3.0), las: Some(1), ..Default::default() };
                d.axis_with(2, &o);
                let o = AxisOptions { pos: Some(8.0), line: Some(3.0), ..Default::default() };
                d.axis_with(3, &o);
                d.axis_with(4, &AxisOptions { pos: Some(12.0), ..Default::default() });
            },
        },
        Case {
            name: "outer",
            r_par: "par(oma = c(2, 3, 2, 1))",
            r: "axis(1, outer = TRUE); axis(2, outer = TRUE, line = 1); \
                axis(3, outer = TRUE, line = -1); axis(4, outer = TRUE, las = 2)",
            xlim: (0.0, 10.0),
            setup: |p| p.oma = Sides::new(2.0, 3.0, 2.0, 1.0),
            draw: |d| {
                d.axis_with(1, &AxisOptions { outer: true, ..Default::default() });
                let o = AxisOptions { outer: true, line: Some(1.0), ..Default::default() };
                d.axis_with(2, &o);
                let o = AxisOptions { outer: true, line: Some(-1.0), ..Default::default() };
                d.axis_with(3, &o);
                let o = AxisOptions { outer: true, las: Some(2), ..Default::default() };
                d.axis_with(4, &o);
            },
        },
        Case {
            name: "tck",
            r_par: "",
            r: "axis(1, tck = 0.02); axis(2, tck = -0.03); \
                axis(3, tck = 1, col.ticks = 'grey'); axis(4, tcl = 0.4)",
            xlim: (0.0, 10.0),
            setup: |_| {},
            draw: |d| {
                d.axis_with(1, &AxisOptions { tck: Some(0.02), ..Default::default() });
                d.axis_with(2, &AxisOptions { tck: Some(-0.03), ..Default::default() });
                let o = AxisOptions { tck: Some(1.0), col_ticks: col("grey"), ..Default::default() };
                d.axis_with(3, &o);
                d.axis_with(4, &AxisOptions { tcl: Some(0.4), ..Default::default() });
            },
        },
        Case {
            name: "adj",
            r_par: "",
            r: "axis(1, hadj = 0, padj = c(0, 1)); \
                axis(2, las = 1, hadj = 0.5, padj = -0.5); \
                axis(3, las = 2, hadj = 0.2); axis(4, padj = c(1, NA, 0.5))",
            xlim: (0.0, 10.0),
            setup: |_| {},
            draw: |d| {
                let o = AxisOptions { hadj: Some(0.0), padj: vec![0.0, 1.0], ..Default::default() };
                d.axis_with(1, &o);
                let o = AxisOptions {
                    las: Some(1),
                    hadj: Some(0.5),
                    padj: vec![-0.5],
                    ..Default::default()
                };
                d.axis_with(2, &o);
                let o = AxisOptions { las: Some(2), hadj: Some(0.2), ..Default::default() };
                d.axis_with(3, &o);
                let o = AxisOptions { padj: vec![1.0, f64::NAN, 0.5], ..Default::default() };
                d.axis_with(4, &o);
            },
        },
        Case {
            name: "gap",
            r_par: "",
            r: "axis(1, at = seq(0, 10, 0.5), gap.axis = -0.5); \
                axis(3, at = seq(0, 10, 0.5), gap.axis = 2); \
                axis(2, at = seq(0, 10, 0.25), las = 1, gap.axis = 0); \
                axis(4, at = seq(0, 10, 0.25), las = 1, gap.axis = 3)",
            xlim: (0.0, 10.0),
            setup: |_| {},
            draw: |d| {
                let at = Some(seq(0.0, 10.0, 0.5));
                let o = AxisOptions { at: at.clone(), gap_axis: Some(-0.5), ..Default::default() };
                d.axis_with(1, &o);
                let o = AxisOptions { at, gap_axis: Some(2.0), ..Default::default() };
                d.axis_with(3, &o);
                let at = Some(seq(0.0, 10.0, 0.25));
                let o = AxisOptions {
                    at: at.clone(),
                    las: Some(1),
                    gap_axis: Some(0.0),
                    ..Default::default()
                };
                d.axis_with(2, &o);
                let o = AxisOptions { at, las: Some(1), gap_axis: Some(3.0), ..Default::default() };
                d.axis_with(4, &o);
            },
        },
        Case {
            name: "styles",
            r_par: "par(lwd = 3)",
            r: "axis(1, lwd = 2, lwd.ticks = 0.5, col = 'red', col.ticks = 'blue', \
                     col.axis = 'darkgreen', cex.axis = 0.8, font = 2); \
                axis(2, lty = 'dashed', labels = FALSE); \
                axis(3, tick = FALSE, at = c(2, 5, 7), labels = c('a', 'bb', 'ccc')); \
                axis(4, lwd = 0, lwd.ticks = 2)",
            xlim: (0.0, 10.0),
            setup: |p| p.lwd = 3.0,
            draw: |d| {
                let o = AxisOptions {
                    lwd: 2.0,
                    lwd_ticks: Some(0.5),
                    col: col("red"),
                    col_ticks: col("blue"),
                    col_axis: col("darkgreen"),
                    cex_axis: Some(0.8),
                    font: Some(2),
                    ..Default::default()
                };
                d.axis_with(1, &o);
                let o = AxisOptions {
                    lty: LineType::from_code(2),
                    draw_labels: false,
                    ..Default::default()
                };
                d.axis_with(2, &o);
                let o = AxisOptions {
                    tick: false,
                    at: Some(vec![2.0, 5.0, 7.0]),
                    labels: Some(vec!["a".into(), "bb".into(), "ccc".into()]),
                    ..Default::default()
                };
                d.axis_with(3, &o);
                let o = AxisOptions { lwd: 0.0, lwd_ticks: Some(2.0), ..Default::default() };
                d.axis_with(4, &o);
            },
        },
        Case {
            name: "axt",
            r_par: "par(xaxt = 'n')",
            r: "axis(1); axis(3, xaxt = 's'); axis(2, yaxt = 'n'); axis(4)",
            xlim: (0.0, 10.0),
            setup: |p| p.xaxt = AxisType::None,
            draw: |d| {
                d.axis_with(1, &AxisOptions::default());
                let o = AxisOptions { axt: Some(AxisType::Standard), ..Default::default() };
                d.axis_with(3, &o);
                let o = AxisOptions { axt: Some(AxisType::None), ..Default::default() };
                d.axis_with(2, &o);
                d.axis_with(4, &AxisOptions::default());
            },
        },
        Case {
            name: "reversed_xpd",
            r_par: "",
            r: "axis(3); par(xpd = TRUE); axis(1, at = c(-1, 0, 5, 10, 11), labels = c('a', 'b', 'c', 'd', 'e')); \
                axis(2, at = c(-2, 5, 12), labels = c('lo', 'mid', 'hi'), las = 1); \
                axis(4, at = c(8, 2), labels = c('eight', 'two'))",
            xlim: (10.0, 0.0),
            setup: |_| {},
            draw: |d| {
                d.axis_with(3, &AxisOptions::default());
                d.par.xpd = true;
                // Letters, not negative numbers: R's `pdf()` draws `-` as
                // the wider `/minus` glyph, which we deliberately don't.
                let o = AxisOptions {
                    at: Some(vec![-1.0, 0.0, 5.0, 10.0, 11.0]),
                    labels: Some(["a", "b", "c", "d", "e"].map(String::from).to_vec()),
                    ..Default::default()
                };
                d.axis_with(1, &o);
                let o = AxisOptions {
                    at: Some(vec![-2.0, 5.0, 12.0]),
                    labels: Some(["lo", "mid", "hi"].map(String::from).to_vec()),
                    las: Some(1),
                    ..Default::default()
                };
                d.axis_with(2, &o);
                let o = AxisOptions {
                    at: Some(vec![8.0, 2.0]),
                    labels: Some(vec!["eight".into(), "two".into()]),
                    ..Default::default()
                };
                d.axis_with(4, &o);
            },
        },
    ]
}

fn r_script(c: &Case, path: &str) -> String {
    format!(
        "pdf(file = '{path}', width = {PAGE_W}, height = {PAGE_H}, compress = FALSE)\n\
         {par}\n\
         plot.new(); plot.window(xlim = c({x0}, {x1}), ylim = c(0, 10))\n\
         {body}\n\
         invisible(dev.off())\n",
        par = c.r_par,
        x0 = c.xlim.0,
        x1 = c.xlim.1,
        body = c.r,
    )
}

fn draw_ours(c: &Case) -> String {
    let mut par = Par::default();
    (c.setup)(&mut par);
    let mut dev = Device::new(
        PdfBackend::new(PAGE_W, PAGE_H).with_compression(false),
        par,
    );
    dev.plot_new();
    dev.plot_window(c.xlim, (0.0, 10.0), false, false);
    (c.draw)(&mut dev);
    String::from_utf8_lossy(&dev.into_backend().finish()).into_owned()
}

// -------------------------------------------------------------------
// A minimal PDF content-stream interpreter (see `boxplot_draw_matches_r.rs`)
// -------------------------------------------------------------------

#[derive(Clone, Debug)]
struct Gs {
    fill: [f64; 3],
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

#[derive(Debug)]
struct Label {
    /// Text-matrix direction (signs) and origin.
    dir: (i32, i32),
    origin: (f64, f64),
    colour: [f64; 3],
}

fn tokens(content: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut chars = content.chars().peekable();
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
        } else if c == '[' || c == ']' {
            chars.next();
            out.push(c.to_string());
        } else {
            let mut s = String::new();
            while let Some(&c) = chars.peek() {
                if c.is_whitespace() || c == '[' || c == ']' {
                    break;
                }
                s.push(c);
                chars.next();
            }
            out.push(s);
        }
    }
    out
}

fn sgn(v: f64) -> i32 {
    if v > 1e-9 {
        1
    } else if v < -1e-9 {
        -1
    } else {
        0
    }
}

fn interpret(pdf: &str) -> (Vec<Stroked>, Vec<Label>) {
    let mut strokes = Vec::new();
    let mut labels = Vec::new();
    let mut rest = pdf;
    while let Some(i) = rest.find("stream\n") {
        let body = &rest[i + 7..];
        let Some(j) = body.find("endstream") else {
            break;
        };
        let content = &body[..j];
        rest = &body[j + 9..];
        if !content.contains(" m") && !content.contains(" Tm") {
            continue;
        }
        let mut gs = Gs {
            fill: [0.0; 3],
            stroke: [0.0; 3],
            lwd: 1.0,
            dash: Vec::new(),
        };
        let mut stack: Vec<Gs> = Vec::new();
        let mut nums: Vec<f64> = Vec::new();
        let mut array: Option<Vec<f64>> = None;
        let mut path: Vec<f64> = Vec::new();
        for t in tokens(content) {
            if let Ok(v) = t.parse::<f64>() {
                match &mut array {
                    Some(a) => a.push(v),
                    None => nums.push(v),
                }
                continue;
            }
            let last = |k: usize| nums[nums.len() - k..].to_vec();
            match t.as_str() {
                "[" => array = Some(Vec::new()),
                "]" => {
                    gs.dash = array.take().unwrap_or_default();
                    continue;
                }
                "q" => stack.push(gs.clone()),
                "Q" => gs = stack.pop().unwrap(),
                "rg" | "scn" => gs.fill = last(3).try_into().unwrap(),
                "RG" | "SCN" => gs.stroke = last(3).try_into().unwrap(),
                "g" => gs.fill = [nums[nums.len() - 1]; 3],
                "G" => gs.stroke = [nums[nums.len() - 1]; 3],
                "w" => gs.lwd = nums[nums.len() - 1],
                "m" | "l" => path.extend(last(2)),
                "n" => path.clear(),
                "S" => strokes.push(Stroked {
                    vertices: std::mem::take(&mut path),
                    colour: gs.stroke,
                    lwd: gs.lwd,
                    dash: gs.dash.clone(),
                }),
                "f" | "F" | "f*" | "B" | "b" | "s" => path.clear(),
                "Tm" => {
                    let m = last(6);
                    labels.push(Label {
                        dir: (sgn(m[0]), sgn(m[1])),
                        origin: (m[4], m[5]),
                        colour: gs.fill,
                    });
                }
                _ => {}
            }
            nums.clear();
        }
    }
    (strokes, labels)
}

fn close(a: &[f64], b: &[f64], tol: f64) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < tol)
}

#[test]
fn axis_arguments_match_r() {
    if !rscript_available() {
        eprintln!("Rscript not found on PATH; skipping R cross-check.");
        return;
    }
    let dir = std::env::temp_dir();
    let cases = cases();
    let mut script = String::new();
    for c in &cases {
        let path = dir.join(format!("rs_graphics_axis_args_{}.pdf", c.name));
        script.push_str(&r_script(c, &path.display().to_string()));
    }
    let script_path = dir.join("rs_graphics_axis_args.R");
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
        let path = dir.join(format!("rs_graphics_axis_args_{}.pdf", c.name));
        let r_pdf = String::from_utf8_lossy(&std::fs::read(&path).unwrap()).into_owned();
        let (r_strokes, r_labels) = interpret(&r_pdf);
        let (our_strokes, our_labels) = interpret(&draw_ours(c));
        let name = c.name;

        assert_eq!(
            our_strokes.len(),
            r_strokes.len(),
            "{name}: stroked path count\nours={our_strokes:#?}\nr={r_strokes:#?}"
        );
        for (k, (a, b)) in our_strokes.iter().zip(&r_strokes).enumerate() {
            let ctx = format!("{name}, path {k}:\n ours={a:?}\n r=   {b:?}");
            assert!(close(&a.vertices, &b.vertices, 0.05), "vertices differ. {ctx}");
            assert!(close(&a.colour, &b.colour, 0.005), "colour differs. {ctx}");
            // R writes widths to 2 decimals.
            assert!((a.lwd - b.lwd).abs() < 0.0051, "line width differs. {ctx}");
            assert!(close(&a.dash, &b.dash, 0.005), "dash pattern differs. {ctx}");
        }

        assert!(!r_labels.is_empty(), "{name}: no labels from R");
        assert_eq!(
            our_labels.len(),
            r_labels.len(),
            "{name}: label count\nours={our_labels:#?}\nr={r_labels:#?}"
        );
        for (k, (a, b)) in our_labels.iter().zip(&r_labels).enumerate() {
            let ctx = format!("{name}, label {k}:\n ours={a:?}\n r=   {b:?}");
            assert_eq!(a.dir, b.dir, "direction differs. {ctx}");
            assert!(
                close(&[a.origin.0, a.origin.1], &[b.origin.0, b.origin.1], 0.05),
                "origin differs. {ctx}"
            );
            assert!(close(&a.colour, &b.colour, 0.005), "colour differs. {ctx}");
        }
    }
}
