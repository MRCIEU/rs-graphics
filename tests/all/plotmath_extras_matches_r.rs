// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks plotmath's fractions (`frac`, `over`, `atop`), radicals
//! (`sqrt`), accents (`hat`, `tilde`, `dot`, `ring`, `bar`, `widehat`,
//! `widetilde`), `abs`, `underline`, `bgroup` delimiters, big operators
//! (`sum`, `prod`, `integral`, `union`, `intersect`, `lim`, `min`, `max`,
//! `inf`, `sup`, ...) and `{}` against a real R `pdf()` device's
//! uncompressed output, in display and script styles, rotated and scaled:
//! every text item (font, size, origin, direction, string) and every
//! stroked line (fraction bars, radicals, accents, `abs` bars). Skipped
//! if `Rscript` isn't available.

use crate::common;

use std::collections::HashMap;
use std::process::Command;

use rs_graphics::prelude::*;

const PAGE_W: f64 = 8.0;
const PAGE_H: f64 = 9.0;

/// `(x, y, expression, cex, srt)`, each drawn with `text()`.
const ITEMS: &[(f64, f64, &str, f64, f64)] = &[
    (1.0, 9.6, "frac(1, 2)", 1.0, 0.0),
    (3.0, 9.6, "frac(x^2 + 1, alpha[i])", 1.0, 0.0),
    (5.0, 9.6, "over(a, b + c)", 1.0, 0.0),
    (7.0, 9.6, "atop(n, k)", 1.0, 0.0),
    (9.0, 9.6, "e^{frac(-x^2, 2)}", 1.0, 0.0),
    (1.0, 8.5, "sqrt(x)", 1.0, 0.0),
    (3.0, 8.5, "sqrt(x^2 + y^2, 3)", 1.0, 0.0),
    (5.0, 8.5, "sqrt(frac(a, b))", 1.0, 0.0),
    (7.0, 8.5, "root(italic(f), n + 1)", 1.0, 0.0),
    (9.0, 8.5, "x^sqrt(y)", 1.5, 0.0),
    (1.0, 7.4, "hat(x) + tilde(y) + dot(z) + ring(A)", 1.0, 0.0),
    (3.6, 7.4, "bar(x) == widehat(xyz)", 1.0, 0.0),
    (6.2, 7.4, "widetilde(abc) * hat(italic(f))", 1.0, 0.0),
    (8.8, 7.4, "bar(italic(W))[i]", 1.0, 0.0),
    (1.0, 6.3, "abs(x - y)", 1.0, 0.0),
    (3.0, 6.3, "underline(bold(x)) + underline(g)", 1.0, 0.0),
    (5.0, 6.3, "abs(frac(1, n))", 1.0, 0.0),
    (7.0, 6.3, "{}^14*C", 1.0, 0.0),
    (9.0, 6.3, "{a + b}^2", 1.0, 0.0),
    (1.0, 5.2, "bgroup(\"(\", frac(a, b), \")\")", 1.0, 0.0),
    (3.0, 5.2, "bgroup(\"[\", x, \"]\")", 1.0, 0.0),
    (5.0, 5.2, "bgroup(\"{\", atop(a, b), \"}\")", 1.0, 0.0),
    (7.0, 5.2, "bgroup(\"|\", frac(x, y), \"|\")", 1.0, 0.0),
    (
        9.0,
        5.2,
        "bgroup(\".\", frac(1, 2), \"|\") * group(lceil, x, rfloor)",
        1.0,
        0.0,
    ),
    (1.0, 3.9, "sum(x[i], i == 1, n)", 1.0, 0.0),
    (3.0, 3.9, "prod(p[k], k == 1, K)", 1.0, 0.0),
    (5.0, 3.9, "integral(f(x) * dx, a, b)", 1.0, 0.0),
    (
        7.0,
        3.9,
        "union(A[i], i == 1, n) * intersect(B[j], j)",
        1.0,
        0.0,
    ),
    (9.0, 3.9, "lim(f(x), x %->% 0)", 1.0, 0.0),
    (1.0, 2.6, "max(x[i], i) - min(y)", 1.0, 0.0),
    (3.0, 2.6, "inf(S) + sup(S, k)", 1.0, 0.0),
    (5.0, 2.6, "liminf(a[n], n %->% infinity)", 1.0, 0.0),
    (
        7.0,
        2.6,
        "textstyle(sum(x[i], i == 1, n) + integral(g, 0, 1))",
        1.0,
        0.0,
    ),
    (9.0, 2.6, "e^{sum(k, k == 1, n)}", 1.0, 0.0),
    (1.5, 1.2, "frac(sqrt(x), 2)", 1.3, 30.0),
    (4.0, 1.2, "bgroup(\"(\", sum(x[i], i), \")\")", 1.0, 90.0),
    (
        6.5,
        1.2,
        "paste(\"Mean \", bar(x), \" = \", frac(1, n) * sum(x[i], i == 1, n))",
        0.8,
        0.0,
    ),
    (9.0, 1.2, "integral(f, a, b)", 2.0, -20.0),
    (1.0, 0.3, "frac(atop(a, b), atop(c, d))", 1.0, 0.0),
    (3.0, 0.3, "frac(1, x^2) + frac(1, y[i]^2)", 1.0, 0.0),
    (
        5.0,
        0.3,
        "bgroup(\"{\", x, \"}\") + bgroup(\"(\", x, \")\")",
        1.0,
        0.0,
    ),
    (7.0, 0.3, "sqrt(x, n)", 1.0, 0.0),
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
fn plotmath_extras_match_r() {
    if !common::r_available() {
        return;
    }
    let dir = std::env::temp_dir();
    let r_pdf = dir.join("rs_graphics_plotmath_extras.pdf");
    let mut script = format!(
        "pdf(file = '{}', width = {PAGE_W}, height = {PAGE_H}, compress = FALSE)\n\
         par(mar = c(1, 1, 1, 1))\n\
         plot.new(); plot.window(c(0, 10), c(0, 10))\n",
        r_pdf.display()
    );
    for &(x, y, e, cex, srt) in ITEMS {
        script.push_str(&format!(
            "text({x}, {y}, expression({e}), cex = {cex}, srt = {srt})\n"
        ));
    }
    script.push_str("invisible(dev.off())\n");
    let script_path = dir.join("rs_graphics_plotmath_extras.R");
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

    let mut d = Device::new(
        PdfBackend::new(PAGE_W, PAGE_H).with_compression(false),
        Par::default(),
    );
    d.par().mar([1, 1, 1, 1]).draw();
    d.plot_new();
    d.plot_window().xlim(0, 10).ylim(0, 10).draw();
    for &(x, y, e, cex, srt) in ITEMS {
        d.text()
            .x([x])
            .y([y])
            .labels([Label::math(e)])
            .cex(cex)
            .srt(srt)
            .draw();
    }

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
        assert!(ok, "text differs:\n ours={a:?}\n r=   {b:?}");
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
