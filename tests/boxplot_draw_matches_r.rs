//! Cross-checks how `boxplot()` draws (as `bxp()` does) against a real R
//! `pdf()` device's uncompressed output: every painted path, in drawing
//! order, with its vertices (so line direction, which sets where dashes
//! fall, counts), fill/stroke, colours, line width, line cap and dash
//! pattern. Curved paths (outlier circles) are compared by bounding box.
//! Covers `notch`, `varwidth` and `horizontal`. Axes are off, so only the
//! boxes themselves are compared. Skipped if `Rscript` isn't available.

use std::process::Command;

use rs_graphics::backend::pdf::PdfBackend;
use rs_graphics::color::{Color, ColorSpec};
use rs_graphics::device::Device;
use rs_graphics::highlevel::{boxplot, BoxplotOptions};
use rs_graphics::par::Par;

fn rscript_available() -> bool {
    Command::new("Rscript")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

const PAGE_W: f64 = 6.0;
const PAGE_H: f64 = 4.0;

/// Three groups; the first and last have outliers on both sides.
fn groups() -> Vec<Vec<f64>> {
    vec![
        vec![-6.0, 1.0, 2.0, 2.5, 3.0, 3.5, 4.0, 5.0, 14.0],
        vec![2.0, 3.0, 3.0, 4.0, 6.0, 7.5],
        vec![0.5, 4.0, 4.2, 4.4, 5.0, 5.1, 5.6, 6.0, 11.0],
    ]
}

struct Case {
    name: &'static str,
    /// R colour names for `col` and `border`.
    col: &'static str,
    border: &'static str,
    lwd: f64,
    notch: bool,
    varwidth: bool,
    horizontal: bool,
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "defaults",
            col: "lightgray",
            border: "black",
            lwd: 1.0,
            notch: false,
            varwidth: false,
            horizontal: false,
        },
        Case {
            name: "coloured_lwd2",
            col: "lightblue",
            border: "darkred",
            lwd: 2.0,
            notch: false,
            varwidth: false,
            horizontal: false,
        },
        Case {
            name: "notch",
            col: "lightgray",
            border: "black",
            lwd: 1.0,
            notch: true,
            varwidth: false,
            horizontal: false,
        },
        Case {
            name: "varwidth",
            col: "lightgray",
            border: "black",
            lwd: 1.0,
            notch: false,
            varwidth: true,
            horizontal: false,
        },
        Case {
            name: "horizontal",
            col: "lightgray",
            border: "black",
            lwd: 1.0,
            notch: false,
            varwidth: false,
            horizontal: true,
        },
        Case {
            name: "all_three",
            col: "lightblue",
            border: "darkred",
            lwd: 2.0,
            notch: true,
            varwidth: true,
            horizontal: true,
        },
    ]
}

fn r_script(c: &Case, path: &str) -> String {
    let groups: Vec<String> = groups()
        .iter()
        .map(|g| {
            let v: Vec<String> = g.iter().map(|x| x.to_string()).collect();
            format!("c({})", v.join(", "))
        })
        .collect();
    format!(
        "pdf(file = '{path}', width = {PAGE_W}, height = {PAGE_H}, compress = FALSE)\n\
         par(lwd = {lwd})\n\
         suppressWarnings(boxplot(list({groups}), col = '{col}', border = '{border}', \
                 axes = FALSE, notch = {notch}, varwidth = {varwidth}, horizontal = {horizontal}))\n\
         invisible(dev.off())\n",
        lwd = c.lwd,
        groups = groups.join(", "),
        col = c.col,
        border = c.border,
        notch = if c.notch { "TRUE" } else { "FALSE" },
        varwidth = if c.varwidth { "TRUE" } else { "FALSE" },
        horizontal = if c.horizontal { "TRUE" } else { "FALSE" },
    )
}

fn draw_ours(c: &Case) -> String {
    let par = Par {
        lwd: c.lwd,
        ..Default::default()
    };
    let mut dev = Device::new(PdfBackend::new(PAGE_W, PAGE_H).with_compression(false), par);
    let opts = BoxplotOptions {
        col: ColorSpec::Color(Color::parse(c.col).unwrap()),
        border: ColorSpec::Color(Color::parse(c.border).unwrap()),
        axes: false,
        notch: c.notch,
        varwidth: c.varwidth,
        horizontal: c.horizontal,
        ..Default::default()
    };
    boxplot(&mut dev, &groups(), &opts);
    String::from_utf8_lossy(&dev.into_backend().finish()).into_owned()
}

// -------------------------------------------------------------------
// A minimal PDF content-stream interpreter (see `pch_matches_r.rs`), keeping
// paths in drawing order with their vertices and dash pattern.
// -------------------------------------------------------------------

#[derive(Clone, Debug)]
struct Gs {
    fill: [f64; 3],
    stroke: [f64; 3],
    lwd: f64,
    cap: i32,
    dash: Vec<f64>,
}

#[derive(Debug)]
struct Painted {
    vertices: Vec<(f64, f64)>,
    curved: bool,
    fill: Option<[f64; 3]>,
    /// `(colour, line width, line cap, dash array)`.
    stroke: Option<([f64; 3], f64, i32, Vec<f64>)>,
}

impl Painted {
    fn bbox(&self) -> [f64; 4] {
        let xs = self.vertices.iter().map(|p| p.0);
        let ys = self.vertices.iter().map(|p| p.1);
        [
            xs.clone().fold(f64::INFINITY, f64::min),
            ys.clone().fold(f64::INFINITY, f64::min),
            xs.fold(f64::NEG_INFINITY, f64::max),
            ys.fold(f64::NEG_INFINITY, f64::max),
        ]
    }
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

/// Every painted path of the page's content stream, in order, minus clips
/// and full-page backgrounds.
fn interpret(pdf: &str) -> Vec<Painted> {
    let mut painted = Vec::new();
    let mut rest = pdf;
    while let Some(i) = rest.find("stream\n") {
        let body = &rest[i + 7..];
        let Some(j) = body.find("endstream") else {
            break;
        };
        let content = &body[..j];
        rest = &body[j + 9..];
        if !content.contains(" m") && !content.contains(" re") {
            continue; // ICC profile, fonts, ...
        }
        let mut gs = Gs {
            fill: [0.0; 3],
            stroke: [0.0; 3],
            lwd: 1.0,
            cap: 0,
            dash: Vec::new(),
        };
        let mut stack: Vec<Gs> = Vec::new();
        let mut nums: Vec<f64> = Vec::new();
        let mut array: Option<Vec<f64>> = None;
        let mut path: Vec<(f64, f64)> = Vec::new();
        let mut curved = false;
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
                    // Only dash arrays appear here; `d` follows with a phase.
                    gs.dash = array.take().unwrap();
                    continue;
                }
                "q" => stack.push(gs.clone()),
                "Q" => gs = stack.pop().unwrap(),
                "rg" | "scn" => gs.fill = last(3).try_into().unwrap(),
                "RG" | "SCN" => gs.stroke = last(3).try_into().unwrap(),
                "g" => gs.fill = [nums[nums.len() - 1]; 3],
                "G" => gs.stroke = [nums[nums.len() - 1]; 3],
                "w" => gs.lwd = nums[nums.len() - 1],
                "J" => gs.cap = nums[nums.len() - 1] as i32,
                "m" | "l" => path.push((nums[nums.len() - 2], nums[nums.len() - 1])),
                "c" => {
                    let n = last(6);
                    path.extend([(n[0], n[1]), (n[2], n[3]), (n[4], n[5])]);
                    curved = true;
                }
                "re" => {
                    let n = last(4);
                    path.extend([
                        (n[0], n[1]),
                        (n[0] + n[2], n[1]),
                        (n[0] + n[2], n[1] + n[3]),
                        (n[0], n[1] + n[3]),
                    ]);
                }
                "W" | "W*" | "h" => {}
                "n" => {
                    path.clear();
                    curved = false;
                }
                "S" | "s" | "f" | "F" | "f*" | "B" | "B*" | "b" | "b*" => {
                    let fill = t.starts_with(['f', 'F', 'B', 'b']);
                    let stroke = t.starts_with(['S', 's', 'B', 'b']);
                    let p = Painted {
                        vertices: std::mem::take(&mut path),
                        curved: std::mem::take(&mut curved),
                        fill: fill.then_some(gs.fill),
                        stroke: stroke.then(|| (gs.stroke, gs.lwd, gs.cap, gs.dash.clone())),
                    };
                    let b = p.bbox();
                    if b[2] - b[0] < 0.9 * PAGE_W * 72.0 {
                        painted.push(p);
                    }
                }
                _ => {}
            }
            nums.clear();
        }
    }
    painted
}

fn close(a: &[f64], b: &[f64], tol: f64) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < tol)
}

#[test]
fn boxplot_drawing_matches_r() {
    if !rscript_available() {
        eprintln!("Rscript not found on PATH; skipping R cross-check.");
        return;
    }
    let dir = std::env::temp_dir();
    let cases = cases();
    let mut script = String::new();
    for c in &cases {
        let path = dir.join(format!("rs_graphics_bxp_{}.pdf", c.name));
        script.push_str(&r_script(c, &path.display().to_string()));
    }
    let script_path = dir.join("rs_graphics_bxp.R");
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
        let path = dir.join(format!("rs_graphics_bxp_{}.pdf", c.name));
        let r_pdf = String::from_utf8_lossy(&std::fs::read(&path).unwrap()).into_owned();
        let r_paths = interpret(&r_pdf);
        let our_paths = interpret(&draw_ours(c));
        let name = c.name;

        assert_eq!(
            our_paths.len(),
            r_paths.len(),
            "{name}: painted path count\nours={our_paths:#?}\nr={r_paths:#?}"
        );
        for (k, (a, b)) in our_paths.iter().zip(&r_paths).enumerate() {
            let ctx = format!("{name}, path {k}:\n ours={a:?}\n r=   {b:?}");
            assert_eq!(a.curved, b.curved, "curvedness differs. {ctx}");
            if a.curved {
                assert!(close(&a.bbox(), &b.bbox(), 0.05), "bbox differs. {ctx}");
            } else {
                let flat = |p: &Painted| -> Vec<f64> {
                    p.vertices.iter().flat_map(|&(x, y)| [x, y]).collect()
                };
                assert!(close(&flat(a), &flat(b), 0.05), "vertices differ. {ctx}");
            }
            assert_eq!(a.fill.is_some(), b.fill.is_some(), "fill differs. {ctx}");
            assert_eq!(a.stroke.is_some(), b.stroke.is_some(), "stroke differs. {ctx}");
            if let (Some(fa), Some(fb)) = (a.fill, b.fill) {
                assert!(close(&fa, &fb, 0.005), "fill colour differs. {ctx}");
            }
            if let (Some(sa), Some(sb)) = (&a.stroke, &b.stroke) {
                assert!(close(&sa.0, &sb.0, 0.005), "stroke colour differs. {ctx}");
                assert!((sa.1 - sb.1).abs() < 0.005, "line width differs. {ctx}");
                assert_eq!(sa.2, sb.2, "line cap differs. {ctx}");
                assert!(close(&sa.3, &sb.3, 0.005), "dash pattern differs. {ctx}");
            }
        }
    }
}
