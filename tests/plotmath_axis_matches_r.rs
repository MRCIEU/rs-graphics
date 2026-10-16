//! Cross-checks plotmath expression labels on `axis()` against a real R
//! `pdf()` device's uncompressed output: every text item (font, size,
//! origin, direction, string) and every stroked line (the `/` slash), on
//! all four sides, with rotation, `cex.axis`, `hadj`/`padj` and `las`.
//! Skipped if `Rscript` isn't available.
//!
//! Not compared: two or more digits in a name in a non-plain font. R's
//! `RenderSymbolStr` means to draw digits plain, but its font switch flips
//! back on every second consecutive digit (`bolditalic(x12)` draws "1"
//! plain and "2" bold italic); we draw them all plain.

use std::collections::HashMap;
use std::process::Command;

use rs_graphics::backend::pdf::PdfBackend;
use rs_graphics::device::{AxisOptions, Device};
use rs_graphics::par::Par;
use rs_graphics::plotmath::Label;

fn rscript_available() -> bool {
    Command::new("Rscript")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

const PAGE_W: f64 = 8.0;
const PAGE_H: f64 = 7.0;

struct Case {
    side: i32,
    exprs: &'static [&'static str],
    /// Extra R arguments to `axis()`.
    r_args: &'static str,
    opts: fn() -> AxisOptions,
}

const CASES: &[Case] = &[
    Case {
        side: 1,
        exprs: &[
            "10^5",
            "alpha[i]^2",
            "-log[10](italic(p))",
            "x[i]^2",
            "paste(\"P = \", 0.05)",
            "e^x^2",
        ],
        r_args: "",
        opts: AxisOptions::default,
    },
    Case {
        side: 2,
        exprs: &["beta", "10^-3", "a %+-% b", "x == y", "bold(x) ~ italic(y)"],
        r_args: "",
        opts: AxisOptions::default,
    },
    Case {
        side: 3,
        exprs: &[
            "a/b",
            "list(x, y, z)",
            "f(x, y)",
            "group(\"[\", x, \"]\")",
            "x * cdots * y",
        ],
        r_args: ", cex.axis = 1.5",
        opts: || AxisOptions {
            cex_axis: Some(1.5),
            ..AxisOptions::default()
        },
    },
    Case {
        side: 4,
        exprs: &[
            "Omega^2",
            "infinity",
            "2 %*% 10^4",
            "x %->% y",
            // One digit only: see `consecutive_digits_are_all_plain`.
            "bolditalic(abc1) <= plain(y)",
        ],
        r_args: ", las = 1, hadj = 0.3, padj = c(0, 1)",
        opts: || AxisOptions {
            las: Some(1),
            hadj: Some(0.3),
            padj: vec![0.0, 1.0],
            ..AxisOptions::default()
        },
    },
];

/// A drawn text item: font, size, origin, direction signs, string.
#[derive(Debug)]
struct Text {
    font: String,
    size: f64,
    x: f64,
    y: f64,
    dir: (i32, i32),
    text: Vec<u8>,
}

#[derive(Debug, PartialEq)]
enum Tok {
    Word(String),
    Str(Vec<u8>),
}

/// Content-stream tokens, with `( ... )` strings unescaped to bytes.
fn tokens(content: &str) -> Vec<Tok> {
    // `content` holds one char per byte (see the test).
    let b: Vec<u8> = content.chars().map(|c| c as u8).collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'(' => {
                let mut s = Vec::new();
                i += 1;
                while b[i] != b')' {
                    if b[i] == b'\\' {
                        i += 1;
                        if b[i].is_ascii_digit() {
                            let oct = std::str::from_utf8(&b[i..i + 3]).unwrap();
                            s.push(u8::from_str_radix(oct, 8).unwrap());
                            i += 3;
                            continue;
                        }
                    }
                    s.push(b[i]);
                    i += 1;
                }
                i += 1;
                out.push(Tok::Str(s));
            }
            c if c.is_ascii_whitespace() => i += 1,
            _ => {
                let start = i;
                while i < b.len() && !b[i].is_ascii_whitespace() && b[i] != b'(' {
                    i += 1;
                }
                out.push(Tok::Word(b[start..i].iter().map(|&c| c as char).collect()));
            }
        }
    }
    out
}

/// Font resource name -> base font, from the PDF's font objects (R's
/// `/Name /F2 /BaseFont /Helvetica`) or, for ours, the fixed resources.
fn base_fonts(pdf: &str) -> HashMap<String, String> {
    let mut map: HashMap<String, String> = [
        ("/F1", "Helvetica"),
        ("/F2", "Helvetica-Bold"),
        ("/F3", "Helvetica-Oblique"),
        ("/F4", "Helvetica-BoldOblique"),
        ("/F5", "Symbol"),
    ]
    .into_iter()
    .map(|(a, b)| (a.to_string(), b.to_string()))
    .collect();
    if pdf.contains("/Name /F") {
        map.clear();
        for part in pdf.split("/Name ").skip(1) {
            let mut w = part.split_whitespace();
            let (name, kw, base) = (w.next(), w.next(), w.next());
            if let (Some(name), Some("/BaseFont"), Some(base)) = (name, kw, base) {
                map.insert(name.to_string(), base.trim_start_matches('/').to_string());
            }
        }
    }
    map
}

fn extract(pdf: &str) -> (Vec<Text>, Vec<Vec<f64>>) {
    let fonts = base_fonts(pdf);
    let mut texts = Vec::new();
    let mut lines = Vec::new();
    let mut rest = pdf;
    while let Some(i) = rest.find("stream\n") {
        let content = &rest[i + 7..];
        let j = content.find("endstream").unwrap_or(content.len());
        let body = &content[..j];
        rest = &content[j..];
        if !body.contains(" Tf") && !body.contains(" m") {
            continue;
        }
        let (mut font, mut tf_size, mut m) = (String::new(), 0.0, [0.0; 6]);
        let mut nums: Vec<f64> = Vec::new();
        let mut path: Vec<f64> = Vec::new();
        let mut last_str: Vec<u8> = Vec::new();
        let mut last_name = String::new();
        for t in tokens(body) {
            let w = match t {
                Tok::Str(s) => {
                    last_str = s;
                    continue;
                }
                Tok::Word(w) => w,
            };
            if let Ok(v) = w.parse::<f64>() {
                nums.push(v);
                continue;
            }
            match w.as_str() {
                "Tf" => {
                    font = fonts.get(&last_name).cloned().unwrap_or_default();
                    tf_size = nums[nums.len() - 1];
                }
                "Tm" => m = nums[nums.len() - 6..].try_into().unwrap(),
                "Tj" => {
                    let scale = (m[0] * m[0] + m[1] * m[1]).sqrt();
                    let sgn = |v: f64| if v.abs() < 1e-9 { 0 } else { v.signum() as i32 };
                    texts.push(Text {
                        font: font.clone(),
                        size: tf_size * scale,
                        x: m[4],
                        y: m[5],
                        dir: (sgn(m[0]), sgn(m[1])),
                        text: last_str.clone(),
                    });
                }
                "m" | "l" => path.extend(&nums[nums.len() - 2..]),
                "S" => lines.push(std::mem::take(&mut path)),
                "n" | "f" | "B" | "b" | "re" => path.clear(),
                w if w.starts_with('/') => last_name = w.to_string(),
                _ => {}
            }
            nums.clear();
        }
    }
    (texts, lines)
}

#[test]
fn plotmath_axis_labels_match_r() {
    if !rscript_available() {
        eprintln!("Rscript not found on PATH; skipping R cross-check.");
        return;
    }
    let dir = std::env::temp_dir();
    let r_pdf = dir.join("rs_graphics_plotmath_axis.pdf");
    let mut script = format!(
        "pdf(file = '{}', width = {PAGE_W}, height = {PAGE_H}, compress = FALSE)\n\
         par(mar = c(5, 6, 5, 6)); plot.new(); plot.window(c(0, 6), c(0, 6))\n",
        r_pdf.display()
    );
    for c in CASES {
        script.push_str(&format!(
            "axis({}, at = 1:{}, labels = expression({}){})\n",
            c.side,
            c.exprs.len(),
            c.exprs.join(", "),
            c.r_args
        ));
    }
    script.push_str("invisible(dev.off())\n");
    let script_path = dir.join("rs_graphics_plotmath_axis.R");
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

    let mut par = Par::default();
    par.mar = rs_graphics::par::Sides::new(5.0, 6.0, 5.0, 6.0);
    let mut dev = Device::new(PdfBackend::new(PAGE_W, PAGE_H).with_compression(false), par);
    dev.plot_new();
    dev.plot_window((0.0, 6.0), (0.0, 6.0), false, false);
    for c in CASES {
        let opts = AxisOptions {
            at: Some((1..=c.exprs.len()).map(|i| i as f64).collect()),
            labels: Some(c.exprs.iter().map(|e| Some(Label::math(e))).collect()),
            ..(c.opts)()
        };
        dev.axis_with(c.side, &opts);
    }
    // Byte for byte: R writes Symbol codes above 127 as raw bytes.
    let latin1 = |b: &[u8]| -> String { b.iter().map(|&c| c as char).collect() };
    let ours = extract(&latin1(&dev.into_backend().finish()));
    let r = extract(&latin1(&std::fs::read(&r_pdf).unwrap()));

    let show = |t: &[Text]| {
        t.iter()
            .map(|t| {
                format!(
                    "{} {:.2} ({:.2}, {:.2}) {:?}",
                    t.font,
                    t.size,
                    t.x,
                    t.y,
                    String::from_utf8_lossy(&t.text)
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    assert_eq!(
        ours.0.len(),
        r.0.len(),
        "text count\nours:\n{}\nr:\n{}",
        show(&ours.0),
        show(&r.0)
    );
    for (a, b) in ours.0.iter().zip(&r.0) {
        let ok = a.font == b.font
            && (a.size - b.size).abs() < 0.01
            && (a.x - b.x).abs() < 0.05
            && (a.y - b.y).abs() < 0.05
            && a.dir == b.dir
            && a.text == b.text;
        assert!(
            ok,
            "text differs:\n ours={a:?}\n r=   {b:?}\nours:\n{}\nr:\n{}",
            show(&ours.0),
            show(&r.0)
        );
    }
    assert_eq!(
        ours.1.len(),
        r.1.len(),
        "stroke count\nours={:?}\nr={:?}",
        ours.1,
        r.1
    );
    for (a, b) in ours.1.iter().zip(&r.1) {
        assert!(
            a.len() == b.len() && a.iter().zip(b).all(|(p, q)| (p - q).abs() < 0.05),
            "stroke differs: ours={a:?} r={b:?}"
        );
    }
}

#[test]
fn consecutive_digits_are_all_plain() {
    let mut dev = Device::new(
        PdfBackend::new(PAGE_W, PAGE_H).with_compression(false),
        Par::default(),
    );
    dev.plot_new();
    let opts = AxisOptions {
        at: Some(vec![0.5]),
        labels: Some(vec![Some(Label::math("bolditalic(x12)"))]),
        ..AxisOptions::default()
    };
    dev.axis_with(1, &opts);
    let latin1 = |b: &[u8]| -> String { b.iter().map(|&c| c as char).collect() };
    let fonts: Vec<(String, Vec<u8>)> = extract(&latin1(&dev.into_backend().finish()))
        .0
        .into_iter()
        .map(|t| (t.font, t.text))
        .collect();
    let want = [
        ("Helvetica-BoldOblique", "x"),
        ("Helvetica", "1"),
        ("Helvetica", "2"),
    ];
    let want: Vec<(String, Vec<u8>)> = want
        .iter()
        .map(|(f, t)| (f.to_string(), t.as_bytes().to_vec()))
        .collect();
    assert_eq!(fonts, want);
}

/// Expressions in `title()`, `mtext()` (inner, outer, perpendicular),
/// `text()` (default and two-element `adj`, `srt`, `cex`, `col`) and
/// `legend()` (labels, including a tall one, and title).
#[test]
fn plotmath_title_mtext_text_legend_match_r() {
    if !rscript_available() {
        eprintln!("Rscript not found on PATH; skipping R cross-check.");
        return;
    }
    let dir = std::env::temp_dir();
    let r_pdf = dir.join("rs_graphics_plotmath_other.pdf");
    let script = format!(
        "pdf(file = '{}', width = {PAGE_W}, height = {PAGE_H}, compress = FALSE)\n\
         par(mar = c(5, 6, 5, 6), oma = c(2, 0, 2, 0))\n\
         plot.new(); plot.window(c(0, 10), c(0, 10))\n\
         title(main = expression(Main ~ alpha^2), sub = expression(sub[i]),\n\
               xlab = expression(x[1] + x[2]), ylab = expression(-log[10](italic(p))))\n\
         mtext(expression(beta^2), side = 3, line = 0.5, adj = 0)\n\
         mtext(expression(Omega ~ infinity), side = 4, line = 1)\n\
         mtext(expression(10^-5), side = 1, outer = TRUE, line = 0.5)\n\
         par(las = 1); mtext(expression(gamma[k]), side = 2, line = 3, at = 8); par(las = 0)\n\
         text(2, 8, expression(x^2 + y^2 == r^2))\n\
         text(7, 3, expression(alpha %+-% beta), adj = c(0, 1), cex = 1.5, srt = 30)\n\
         text(5, 2, expression(paste(\"P = \", 0.05)), col = 'red')\n\
         legend('topright', legend = expression(alpha, beta^2, x[i]^(y^2)), title = expression(Sigma[j]), lty = 1:3)\n\
         invisible(dev.off())\n",
        r_pdf.display()
    );
    let script_path = dir.join("rs_graphics_plotmath_other.R");
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

    use rs_graphics::color::{Color, ColorSpec};
    use rs_graphics::device::{
        LegendEntry, LegendKeyword, LegendOptions, LegendPos, MtextOptions, TitleOptions,
    };
    use rs_graphics::lty::LineType;
    use rs_graphics::par::Sides;
    let par = Par {
        mar: Sides::new(5.0, 6.0, 5.0, 6.0),
        oma: Sides::new(2.0, 0.0, 2.0, 0.0),
        ..Par::default()
    };
    let mut d = Device::new(PdfBackend::new(PAGE_W, PAGE_H).with_compression(false), par);
    d.plot_new();
    d.plot_window((0.0, 10.0), (0.0, 10.0), false, false);
    d.title_with(&TitleOptions {
        main: Some(Label::math("Main ~ alpha^2")),
        sub: Some(Label::math("sub[i]")),
        xlab: Some(Label::math("x[1] + x[2]")),
        ylab: Some(Label::math("-log[10](italic(p))")),
    });
    let mt = |side, line| MtextOptions {
        side,
        line,
        ..MtextOptions::default()
    };
    d.mtext(
        Label::math("beta^2"),
        &MtextOptions {
            adj: Some(0.0),
            ..mt(3, 0.5)
        },
    );
    d.mtext(Label::math("Omega ~ infinity"), &mt(4, 1.0));
    d.mtext(
        Label::math("10^-5"),
        &MtextOptions {
            outer: true,
            ..mt(1, 0.5)
        },
    );
    d.par.las = 1;
    d.mtext(
        Label::math("gamma[k]"),
        &MtextOptions {
            at: Some(8.0),
            ..mt(2, 3.0)
        },
    );
    d.par.las = 0;
    let black = ColorSpec::Index(1);
    d.text(
        2.0,
        8.0,
        Label::math("x^2 + y^2 == r^2"),
        0.5,
        None,
        0.0,
        black,
        1.0,
        1,
    );
    d.text(
        7.0,
        3.0,
        Label::math("alpha %+-% beta"),
        0.0,
        Some(1.0),
        30.0,
        black,
        1.5,
        1,
    );
    let red = ColorSpec::Color(Color::parse("red").unwrap());
    d.text(
        5.0,
        2.0,
        Label::math("paste(\"P = \", 0.05)"),
        0.5,
        None,
        0.0,
        red,
        1.0,
        1,
    );
    let entries: Vec<LegendEntry> = ["alpha", "beta^2", "x[i]^(y^2)"]
        .iter()
        .enumerate()
        .map(|(i, e)| {
            LegendEntry::line(
                Label::math(e),
                black,
                1.0,
                LineType::from_code(i as i32 + 1),
            )
        })
        .collect();
    let opts = LegendOptions {
        title: Some(Label::math("Sigma[j]")),
        ..LegendOptions::default()
    };
    d.legend_with(LegendPos::Keyword(LegendKeyword::TopRight), &entries, &opts);

    let latin1 = |b: &[u8]| -> String { b.iter().map(|&c| c as char).collect() };
    let ours = extract(&latin1(&d.into_backend().finish()));
    let r = extract(&latin1(&std::fs::read(&r_pdf).unwrap()));
    let show = |t: &[Text]| {
        t.iter()
            .map(|t| {
                format!(
                    "{} {:.2} ({:.2}, {:.2}) {:?}",
                    t.font,
                    t.size,
                    t.x,
                    t.y,
                    String::from_utf8_lossy(&t.text)
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    assert_eq!(
        ours.0.len(),
        r.0.len(),
        "text count\nours:\n{}\nr:\n{}",
        show(&ours.0),
        show(&r.0)
    );
    for (a, b) in ours.0.iter().zip(&r.0) {
        let ok = a.font == b.font
            && (a.size - b.size).abs() < 0.01
            && (a.x - b.x).abs() < 0.05
            && (a.y - b.y).abs() < 0.05
            && a.dir == b.dir
            && a.text == b.text;
        assert!(
            ok,
            "text differs:\n ours={a:?}\n r=   {b:?}\nours:\n{}\nr:\n{}",
            show(&ours.0),
            show(&r.0)
        );
    }
    assert_eq!(
        ours.1.len(),
        r.1.len(),
        "stroke count\nours={:?}\nr={:?}",
        ours.1,
        r.1
    );
    for (a, b) in ours.1.iter().zip(&r.1) {
        assert!(
            a.len() == b.len() && a.iter().zip(b).all(|(p, q)| (p - q).abs() < 0.05),
            "stroke differs: ours={a:?} r={b:?}"
        );
    }
}

#[test]
fn plotmath_high_level_titles_match_r() {
    if !rscript_available() {
        eprintln!("Rscript not found on PATH; skipping R cross-check.");
        return;
    }
    let dir = std::env::temp_dir();
    let r_pdf = dir.join("rs_graphics_plotmath_highlevel.pdf");
    let script = format!(
        "pdf(file = '{}', width = {PAGE_W}, height = {PAGE_H}, compress = FALSE)\n\
         par(mfrow = c(2, 3))\n\
         plot(1:10, (1:10)^2, main = expression(y == x^2), sub = expression(n[obs]),\n\
              xlab = expression(x[i]), ylab = expression(-log[10](italic(p))))\n\
         x <- c(1, 2, 2, 3, 3, 3, 4, 4, 5)\n\
         hist(x, main = expression(Histogram ~ of ~ X[1]), ylab = expression(count[i]))\n\
         barplot(c(3, 5, 2), names.arg = c('a', 'b', 'c'), main = expression(bold(Bars)),\n\
                 xlab = expression(group[k]))\n\
         boxplot(list(c(1, 2, 3, 4, 9), c(2, 3, 4)), main = expression(sigma^2),\n\
                 ylab = expression(mu %+-% sigma))\n\
         pie(c(1, 2, 3), labels = c('a', 'b', 'c'), main = expression(pi * r^2))\n\
         invisible(dev.off())\n",
        r_pdf.display()
    );
    let script_path = dir.join("rs_graphics_plotmath_highlevel.R");
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

    use rs_graphics::highlevel::{
        BarplotOptions, BoxplotOptions, HistOptions, PieOptions, PlotOptions, barplot, boxplot,
        hist, pie, plot,
    };
    let math = |s: &str| Some(Label::math(s));
    let abc = || Some(vec!["a".to_string(), "b".to_string(), "c".to_string()]);
    let mut d = Device::new(
        PdfBackend::new(PAGE_W, PAGE_H).with_compression(false),
        Par::default(),
    );
    d.mfrow(2, 3);
    let x: Vec<f64> = (1..=10).map(f64::from).collect();
    let y: Vec<f64> = x.iter().map(|v| v * v).collect();
    plot(
        &mut d,
        &x,
        &y,
        &PlotOptions {
            main: math("y == x^2"),
            sub: math("n[obs]"),
            xlab: math("x[i]"),
            ylab: math("-log[10](italic(p))"),
            ..PlotOptions::default()
        },
    );
    hist(
        &mut d,
        &[1.0, 2.0, 2.0, 3.0, 3.0, 3.0, 4.0, 4.0, 5.0],
        &HistOptions {
            main: math("Histogram ~ of ~ X[1]"),
            ylab: math("count[i]"),
            ..HistOptions::default()
        },
    );
    barplot(
        &mut d,
        &[3.0, 5.0, 2.0],
        &BarplotOptions {
            names: abc(),
            main: math("bold(Bars)"),
            xlab: math("group[k]"),
            ..BarplotOptions::default()
        },
    );
    boxplot(
        &mut d,
        &[vec![1.0, 2.0, 3.0, 4.0, 9.0], vec![2.0, 3.0, 4.0]],
        &BoxplotOptions {
            main: math("sigma^2"),
            ylab: math("mu %+-% sigma"),
            ..BoxplotOptions::default()
        },
    );
    pie(
        &mut d,
        &[1.0, 2.0, 3.0],
        &PieOptions {
            labels: abc(),
            main: math("pi * r^2"),
            ..PieOptions::default()
        },
    );

    let latin1 = |b: &[u8]| -> String { b.iter().map(|&c| c as char).collect() };
    let ours = extract(&latin1(&d.into_backend().finish())).0;
    let r = extract(&latin1(&std::fs::read(&r_pdf).unwrap())).0;
    let show = |t: &[Text]| {
        t.iter()
            .map(|t| {
                format!(
                    "{} {:.2} ({:.2}, {:.2}) {:?}",
                    t.font,
                    t.size,
                    t.x,
                    t.y,
                    String::from_utf8_lossy(&t.text)
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    assert_eq!(
        ours.len(),
        r.len(),
        "text count\nours:\n{}\nr:\n{}",
        show(&ours),
        show(&r)
    );
    for (a, b) in ours.iter().zip(&r) {
        let ok = a.font == b.font
            && (a.size - b.size).abs() < 0.01
            && (a.x - b.x).abs() < 0.05
            && (a.y - b.y).abs() < 0.05
            && a.dir == b.dir
            && a.text == b.text;
        assert!(
            ok,
            "text differs:\n ours={a:?}\n r=   {b:?}\nours:\n{}\nr:\n{}",
            show(&ours),
            show(&r)
        );
    }
}
