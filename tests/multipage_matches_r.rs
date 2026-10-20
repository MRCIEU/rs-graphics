// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks multi-page PDF output against a real R `pdf()` device's
//! uncompressed output: the number of pages, and on each page every painted
//! path in drawing order (operator and vertices, `re` expanded; circles by
//! operator only) and every text item (origin and string). Pages start as
//! in R: each `plot.new()` on a one-figure page, `mfrow` wrapping, and
//! `par(fig = )` without `new = TRUE`; `par(new = TRUE)` adds none.
//! Skipped if `Rscript` isn't available. Also, without R: compressed pages
//! inflate to the uncompressed ones.

mod common;

use std::process::Command;

use rs_graphics::backend::pdf::PdfBackend;
use rs_graphics::device::Device;
use rs_graphics::highlevel::PlotOptions;
use rs_graphics::par::Par;
use rs_graphics::graphics::Graphics;

const PAGE_W: f64 = 6.0;
const PAGE_H: f64 = 5.0;

type Dev = Device<PdfBackend>;

/// R: `pl(n, main)` is `plot(1:n, main = main, xlab = 'x', ylab = 'y')`.
const R_PRELUDE: &str = "pl <- function(n, main) plot(1:n, main = main, xlab = 'x', ylab = 'y')\n";

fn pl(d: &mut Dev, n: usize, main: &'static str) {
    let x: Vec<f64> = (1..=n).map(|i| i as f64).collect();
    let o = PlotOptions {
        main: Some(main.into()),
        xlab: Some("x".into()),
        ylab: Some("y".into()),
        ..PlotOptions::default()
    };
    d.plot(&x, &x).with(&o).draw();
}

struct Case {
    name: &'static str,
    pages: usize,
    r: &'static str,
    draw: fn(&mut Dev),
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "three_plots",
            pages: 3,
            r: "pl(3, 'one'); pl(4, 'two'); pl(5, 'three')",
            draw: |d| {
                pl(d, 3, "one");
                pl(d, 4, "two");
                pl(d, 5, "three");
            },
        },
        Case {
            name: "mfrow_wraps",
            pages: 2,
            r: "par(mfrow = c(1, 2)); pl(3, 'A'); pl(4, 'B'); pl(5, 'C')",
            draw: |d| {
                d.par().mfrow(1, 2).draw();
                pl(d, 3, "A");
                pl(d, 4, "B");
                pl(d, 5, "C");
            },
        },
        Case {
            name: "fig_and_new",
            pages: 2,
            r: "pl(3, 'whole')\n\
                par(fig = c(0, 0.5, 0, 1)); pl(4, 'left')\n\
                par(fig = c(0.5, 1, 0, 1), new = TRUE); pl(5, 'right')",
            draw: |d| {
                pl(d, 3, "whole");
                d.par().fig([0.0, 0.5, 0.0, 1.0]).draw();
                pl(d, 4, "left");
                d.par().fig([0.5, 1.0, 0.0, 1.0]).draw();
                d.par().new(true).draw();
                pl(d, 5, "right");
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

/// Each page's (paths, texts): one per content stream that draws anything.
fn pages(pdf: &str) -> Vec<(Vec<Painted>, Vec<Text>)> {
    let mut out = Vec::new();
    let mut rest = pdf;
    while let Some(i) = rest.find("stream\n") {
        let body = &rest[i + 7..];
        let Some(j) = body.find("endstream") else {
            break;
        };
        let content = &body[..j];
        rest = &body[j + 9..];
        if content.contains(" m\n") || content.contains(" re\n") || content.contains(" Tm") {
            out.push(extract(content));
        }
    }
    out
}

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

#[test]
fn multipage_pdf_matches_r() {
    if !common::r_available() {
        return;
    }
    let dir = std::env::temp_dir();
    let cases = cases();
    let mut script = R_PRELUDE.to_string();
    for c in &cases {
        let path = dir.join(format!("rs_graphics_multipage_{}.pdf", c.name));
        script.push_str(&format!(
            "pdf(file = '{}', width = {PAGE_W}, height = {PAGE_H}, compress = FALSE)\n{}\n\
             invisible(dev.off())\n",
            path.display(),
            c.r
        ));
    }
    let script_path = dir.join("rs_graphics_multipage.R");
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
        let path = dir.join(format!("rs_graphics_multipage_{}.pdf", c.name));
        let r_pdf = String::from_utf8_lossy(&std::fs::read(&path).unwrap()).into_owned();
        let r = pages(&r_pdf);
        let mut dev = Device::new(
            PdfBackend::new(PAGE_W, PAGE_H).with_compression(false),
            Par::default(),
        );
        (c.draw)(&mut dev);
        let ours_pdf = String::from_utf8_lossy(&dev.into_backend().finish()).into_owned();
        let ours = pages(&ours_pdf);
        let name = c.name;

        assert_eq!(r.len(), c.pages, "{name}: R's page count");
        assert_eq!(ours.len(), c.pages, "{name}: our page count");
        assert!(ours_pdf.contains(&format!("/Count {}", c.pages)), "{name}: /Count");
        for (p, ((our_paths, our_texts), (r_paths, r_texts))) in ours.iter().zip(&r).enumerate() {
            assert_eq!(our_paths.len(), r_paths.len(), "{name} page {p}: path count");
            for (k, (a, b)) in our_paths.iter().zip(r_paths).enumerate() {
                let ctx = format!("{name} page {p}, path {k}:\n ours={a:?}\n r=   {b:?}");
                assert_eq!(a.op, b.op, "operator differs. {ctx}");
                assert!(
                    a.vertices.len() == b.vertices.len()
                        && a.vertices.iter().zip(&b.vertices).all(|(x, y)| (x - y).abs() < 0.05),
                    "vertices differ. {ctx}"
                );
            }
            assert_eq!(our_texts.len(), r_texts.len(), "{name} page {p}: text count");
            for (a, b) in our_texts.iter().zip(r_texts) {
                assert_eq!(a.string, b.string, "{name} page {p}: text differs");
                assert!(
                    (a.x - b.x).abs() < 0.05 && (a.y - b.y).abs() < 0.05,
                    "{name} page {p}: text origin differs: ours={a:?} r={b:?}"
                );
            }
        }
    }
}

#[test]
fn compressed_pages_inflate_to_the_uncompressed_ones() {
    let draw = |compress: bool| {
        let mut dev = Device::new(
            PdfBackend::new(PAGE_W, PAGE_H).with_compression(compress),
            Par::default(),
        );
        pl(&mut dev, 3, "one");
        pl(&mut dev, 4, "two");
        dev.into_backend().finish()
    };
    let (plain, packed) = (draw(false), draw(true));
    let packed_text = String::from_utf8_lossy(&packed);
    assert!(packed_text.contains("/Count 2"));
    assert_eq!(packed_text.matches("/FlateDecode").count(), 2);
    // Each compressed stream inflates to the matching uncompressed one.
    let find = |hay: &[u8], needle: &[u8], from: usize| {
        hay[from..]
            .windows(needle.len())
            .position(|w| w == needle)
            .map(|i| from + i)
    };
    let streams = |pdf: &[u8]| -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        let mut from = 0;
        while let Some(i) = find(pdf, b"/Length ", from) {
            let digits: String = pdf[i + 8..]
                .iter()
                .take_while(|c| c.is_ascii_digit())
                .map(|&c| c as char)
                .collect();
            let len: usize = digits.parse().unwrap();
            let start = find(pdf, b"stream\n", i).unwrap() + 7;
            out.push(pdf[start..start + len].to_vec());
            from = start + len;
        }
        out
    };
    let (a, b) = (streams(&plain), streams(&packed));
    assert_eq!(a.len(), 2);
    for (p, z) in a.iter().zip(&b) {
        assert_eq!(&miniz_oxide::inflate::decompress_to_vec_zlib(z).unwrap(), p);
    }
}
