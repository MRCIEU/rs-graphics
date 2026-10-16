//! Cross-checks every plotting symbol - `pch = 0:25` plus character `pch`
//! (`"."`, letters, punctuation) - against a real R `pdf()` device's
//! uncompressed output. Each scenario draws a chart of all the symbols (in
//! the spirit of the classic "pch chart") with a different `col`/`bg`/
//! `cex`/`lwd`/`par(cex)`, and every painted path is compared: its bounding
//! box, whether it is filled and/or stroked, fill and stroke colours, line
//! width and line cap; character symbols are compared by text origin, size
//! and string. Skipped if `Rscript` isn't available.

use std::process::Command;

use rs_graphics::backend::pdf::PdfBackend;
use rs_graphics::color::{Color, ColorSpec};
use rs_graphics::device::Device;
use rs_graphics::par::{Par, Sides};

fn rscript_available() -> bool {
    Command::new("Rscript")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

const PAGE_W: f64 = 6.0;
const PAGE_H: f64 = 4.0;
const CHARS: &[char] = &['.', 'a', 'M', '+', '*', '#', 'o', '%'];

/// `(x, y)` user coordinates of `pch = i` in the chart: four rows of seven,
/// with character symbols on a fifth row.
fn pch_pos(i: usize) -> (f64, f64) {
    ((i % 7) as f64 + 1.0, 5.0 - (i / 7) as f64)
}

fn char_pos(i: usize) -> (f64, f64) {
    (i as f64 + 0.5, 1.0)
}

struct Case {
    name: &'static str,
    /// R colour names (`None` = `NA` for `bg`).
    col: &'static str,
    bg: Option<&'static str>,
    cex: f64,
    lwd: f64,
    par_cex: f64,
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "defaults",
            col: "black",
            bg: None,
            cex: 1.0,
            lwd: 1.0,
            par_cex: 1.0,
        },
        Case {
            name: "chart_blue_red_cex2",
            col: "blue",
            bg: Some("red"),
            cex: 2.0,
            lwd: 1.0,
            par_cex: 1.0,
        },
        Case {
            name: "small_thick",
            col: "darkgreen",
            bg: Some("yellow"),
            cex: 0.7,
            lwd: 3.0,
            par_cex: 1.0,
        },
        Case {
            name: "par_cex",
            col: "purple",
            bg: Some("orange"),
            cex: 1.0,
            lwd: 2.0,
            par_cex: 1.5,
        },
    ]
}

fn r_script(c: &Case, path: &str) -> String {
    let xs: Vec<String> = (0..26).map(|i| pch_pos(i).0.to_string()).collect();
    let ys: Vec<String> = (0..26).map(|i| pch_pos(i).1.to_string()).collect();
    let cx: Vec<String> = (0..CHARS.len()).map(|i| char_pos(i).0.to_string()).collect();
    let chars: Vec<String> = CHARS.iter().map(|c| format!("'{c}'")).collect();
    let bg = c.bg.map_or("NA".to_string(), |b| format!("'{b}'"));
    format!(
        "pdf(file = '{path}', width = {PAGE_W}, height = {PAGE_H}, compress = FALSE)\n\
         par(mar = c(0, 0, 0, 0), cex = {pc})\n\
         plot.new(); plot.window(c(0, 8), c(0, 6))\n\
         points(c({xs}), c({ys}), pch = 0:25, col = '{col}', bg = {bg}, cex = {cex}, lwd = {lwd})\n\
         points(c({cx}), rep(1, {nc}), pch = c({chars}), col = '{col}', cex = {cex})\n\
         invisible(dev.off())\n",
        pc = c.par_cex,
        xs = xs.join(", "),
        ys = ys.join(", "),
        col = c.col,
        cex = c.cex,
        lwd = c.lwd,
        cx = cx.join(", "),
        nc = CHARS.len(),
        chars = chars.join(", "),
    )
}

fn draw_ours(c: &Case) -> String {
    let mut par = Par::default();
    par.mar = Sides::new(0.0, 0.0, 0.0, 0.0);
    par.cex = c.par_cex;
    let mut dev = Device::new(PdfBackend::new(PAGE_W, PAGE_H).with_compression(false), par);
    dev.plot_new();
    dev.plot_window((0.0, 8.0), (0.0, 6.0), false, false);
    let col = ColorSpec::Color(Color::parse(c.col).unwrap());
    let bg = c
        .bg
        .map_or(ColorSpec::None, |b| ColorSpec::Color(Color::parse(b).unwrap()));
    dev.par.lwd = c.lwd;
    for pch in 0..26 {
        let (x, y) = pch_pos(pch);
        dev.points(&[x], &[y], pch as i32, col, bg, c.cex);
    }
    dev.par.lwd = 1.0;
    for (i, &ch) in CHARS.iter().enumerate() {
        let (x, y) = char_pos(i);
        dev.points(&[x], &[y], ch as i32, col, ColorSpec::None, c.cex);
    }
    String::from_utf8_lossy(&dev.into_backend().finish()).into_owned()
}

// -------------------------------------------------------------------
// A minimal PDF content-stream interpreter: just enough graphics state to
// describe what each painting operator puts on the page.
// -------------------------------------------------------------------

#[derive(Clone, Debug)]
struct Gs {
    fill: [f64; 3],
    stroke: [f64; 3],
    lwd: f64,
    cap: i32,
}

#[derive(Debug)]
struct Painted {
    bbox: [f64; 4],
    fill: Option<[f64; 3]>,
    /// `(colour, line width, line cap)`.
    stroke: Option<([f64; 3], f64, i32)>,
}

#[derive(Debug)]
struct Text {
    x: f64,
    y: f64,
    size: f64,
    s: String,
    col: [f64; 3],
}

fn tokens(content: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut chars = content.chars().peekable();
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
        } else if c == '(' {
            chars.next();
            let mut s = String::from("(");
            while let Some(c) = chars.next() {
                match c {
                    '\\' => {
                        if let Some(n) = chars.next() {
                            s.push(n);
                        }
                    }
                    ')' => break,
                    _ => s.push(c),
                }
            }
            out.push(s);
        } else if c == '[' || c == ']' {
            chars.next();
            out.push(c.to_string());
        } else {
            let mut s = String::new();
            while let Some(&c) = chars.peek() {
                if c.is_whitespace() || c == '(' || c == '[' || c == ']' {
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

/// Interpret every content stream in `pdf`, returning painted paths (minus
/// clips and full-page backgrounds) and text runs.
fn interpret(pdf: &str) -> (Vec<Painted>, Vec<Text>) {
    let mut painted = Vec::new();
    let mut texts = Vec::new();
    let mut rest = pdf;
    while let Some(i) = rest.find("stream\n") {
        let body = &rest[i + 7..];
        let Some(j) = body.find("endstream") else { break };
        let content = &body[..j];
        rest = &body[j + 9..];
        if !content.contains(" m") && !content.contains(" re") && !content.contains("Tj") {
            continue; // ICC profile, fonts, ...
        }
        let mut gs = Gs {
            fill: [0.0; 3],
            stroke: [0.0; 3],
            lwd: 1.0,
            cap: 0,
        };
        let mut stack: Vec<Gs> = Vec::new();
        let mut nums: Vec<f64> = Vec::new();
        let mut path: Vec<(f64, f64)> = Vec::new();
        let mut font_size = 0.0;
        let mut origin = (0.0, 0.0, 1.0);
        for t in tokens(content) {
            if let Ok(v) = t.parse::<f64>() {
                nums.push(v);
                continue;
            }
            let last = |k: usize| nums[nums.len() - k..].to_vec();
            match t.as_str() {
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
                }
                "re" => {
                    let n = last(4);
                    path.extend([(n[0], n[1]), (n[0] + n[2], n[1] + n[3])]);
                }
                "W" | "W*" | "h" => {}
                "n" => path.clear(),
                "S" | "s" | "f" | "F" | "f*" | "B" | "B*" | "b" | "b*" => {
                    let fill = t.starts_with(['f', 'F', 'B', 'b']);
                    let stroke = t.starts_with(['S', 's', 'B', 'b']);
                    let xs = path.iter().map(|p| p.0);
                    let ys = path.iter().map(|p| p.1);
                    let bbox = [
                        xs.clone().fold(f64::INFINITY, f64::min),
                        ys.clone().fold(f64::INFINITY, f64::min),
                        xs.fold(f64::NEG_INFINITY, f64::max),
                        ys.fold(f64::NEG_INFINITY, f64::max),
                    ];
                    path.clear();
                    let full_page = bbox[2] - bbox[0] > 0.9 * PAGE_W * 72.0;
                    if !full_page {
                        painted.push(Painted {
                            bbox,
                            fill: fill.then_some(gs.fill),
                            stroke: stroke.then_some((gs.stroke, gs.lwd, gs.cap)),
                        });
                    }
                }
                "Tf" => font_size = nums[nums.len() - 1],
                "Tm" => {
                    let n = last(6);
                    origin = (n[4], n[5], n[0].hypot(n[1]));
                }
                "Tj" => {}
                s if s.starts_with('(') => texts.push(Text {
                    x: origin.0,
                    y: origin.1,
                    size: font_size * origin.2,
                    s: s[1..].to_string(),
                    col: gs.fill,
                }),
                _ => {}
            }
            nums.clear();
        }
    }
    // Group by symbol (centre rounded to whole points), then by shape.
    let key = |p: &Painted| {
        let b = p.bbox;
        let r = |v: f64| (v * 0.5).round() as i64;
        (
            r(b[0] + b[2]),
            r(b[1] + b[3]),
            ((b[2] - b[0]) * 10.0).round() as i64,
            ((b[3] - b[1]) * 10.0).round() as i64,
            p.fill.is_some(),
        )
    };
    painted.sort_by_key(key);
    texts.sort_by(|a, b| (a.x, a.y).partial_cmp(&(b.x, b.y)).unwrap());
    (painted, texts)
}

fn close(a: &[f64], b: &[f64], tol: f64) -> bool {
    a.iter().zip(b).all(|(x, y)| (x - y).abs() < tol)
}

#[test]
fn every_pch_matches_r() {
    if !rscript_available() {
        eprintln!("Rscript not found on PATH; skipping R cross-check.");
        return;
    }
    let dir = std::env::temp_dir();
    let cases = cases();
    let mut script = String::new();
    for c in &cases {
        let path = dir.join(format!("rs_graphics_pch_{}.pdf", c.name));
        script.push_str(&r_script(c, &path.display().to_string()));
    }
    let script_path = dir.join("rs_graphics_pch.R");
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
        let path = dir.join(format!("rs_graphics_pch_{}.pdf", c.name));
        let r_pdf = String::from_utf8_lossy(&std::fs::read(&path).unwrap()).into_owned();
        let (r_paths, r_texts) = interpret(&r_pdf);
        let (our_paths, our_texts) = interpret(&draw_ours(c));
        let name = c.name;

        assert_eq!(
            our_paths.len(),
            r_paths.len(),
            "{name}: painted path count\nours={our_paths:#?}\nr={r_paths:#?}"
        );
        for (a, b) in our_paths.iter().zip(&r_paths) {
            let ctx = format!("{name}:\n ours={a:?}\n r=   {b:?}");
            assert!(close(&a.bbox, &b.bbox, 0.05), "bbox differs. {ctx}");
            assert_eq!(a.fill.is_some(), b.fill.is_some(), "fill differs. {ctx}");
            assert_eq!(a.stroke.is_some(), b.stroke.is_some(), "stroke differs. {ctx}");
            if let (Some(fa), Some(fb)) = (a.fill, b.fill) {
                assert!(close(&fa, &fb, 0.005), "fill colour differs. {ctx}");
            }
            if let (Some(sa), Some(sb)) = (a.stroke, b.stroke) {
                assert!(close(&sa.0, &sb.0, 0.005), "stroke colour differs. {ctx}");
                assert!((sa.1 - sb.1).abs() < 0.005, "line width differs. {ctx}");
                assert_eq!(sa.2, sb.2, "line cap differs. {ctx}");
            }
        }

        assert_eq!(
            our_texts.len(),
            r_texts.len(),
            "{name}: text count\nours={our_texts:?}\nr={r_texts:?}"
        );
        for (a, b) in our_texts.iter().zip(&r_texts) {
            let ctx = format!("{name}:\n ours={a:?}\n r=   {b:?}");
            assert_eq!(a.s, b.s, "string differs. {ctx}");
            assert!(
                close(&[a.x, a.y, a.size], &[b.x, b.y, b.size], 0.05),
                "text origin/size differs. {ctx}"
            );
            assert!(close(&a.col, &b.col, 0.005), "text colour differs. {ctx}");
        }
    }
}
