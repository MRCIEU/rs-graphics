// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks kerning inside PDF strings against a real R `pdf()`
//! device's uncompressed output: a string with kerning pairs is a `TJ`
//! array (`[(A) 70 (V)] TJ`), one without a plain `Tj`, in all four
//! Helvetica faces, with Latin-1 characters, scaled and rotated. Compares
//! every text item's font, size, origin and its pieces and kerns.
//!
//! R's lookup never applies the last kerning pair of each first character
//! (`devPS.c`: `KPend` is stored inclusive but looped over exclusively),
//! a bug we don't copy, so the strings here avoid those pairs;
//! `last_pair_of_a_character_is_kerned` checks ours on its own. Skipped if
//! `Rscript` isn't available.

use crate::common;

use std::collections::HashMap;
use std::process::Command;

use rs_graphics::prelude::*;

const PAGE_W: f64 = 7.0;
const PAGE_H: f64 = 7.0;

/// `(x, y, label, font, cex, srt)`, drawn with `text()`.
const ITEMS: &[(f64, f64, &str, i32, f64, f64)] = &[
    (5.0, 9.5, "Toyota AVATAR", 1, 1.0, 0.0),
    (5.0, 8.7, "WAVE Yo Te", 1, 1.5, 0.0),
    (5.0, 7.9, "no pairs here", 1, 1.0, 0.0),
    (5.0, 7.1, "Toyota AVATAR", 2, 1.0, 0.0),
    (5.0, 6.3, "Toyota AVATAR", 3, 1.2, 0.0),
    (5.0, 5.5, "Toyota AVATAR", 4, 0.8, 0.0),
    (5.0, 4.7, "caf\u{e9} \u{c5}V", 1, 1.0, 0.0),
    (5.0, 3.9, "(LT) \\ Av", 1, 1.0, 0.0),
    (2.0, 2.0, "Tilted Type", 2, 1.0, 30.0),
    (8.0, 2.0, "Vertical", 1, 1.0, 90.0),
    (5.0, 0.8, "A", 1, 1.0, 0.0),
];

/// One text item: font, size, origin, and its pieces (`Some` string or a
/// `None` for each kern, with the kern value).
#[derive(Debug, PartialEq)]
struct Text {
    font: String,
    size: f64,
    origin: (f64, f64),
    pieces: Vec<Piece>,
}

#[derive(Debug, PartialEq)]
enum Piece {
    Str(Vec<u8>),
    Kern(f64),
}

enum Tok {
    Word(String),
    Str(Vec<u8>),
}

/// Content-stream tokens; `( ... )` strings unescaped to bytes, `[`/`]`
/// separate words.
fn tokens(content: &[u8]) -> Vec<Tok> {
    let b = content;
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
            b'[' | b']' => {
                out.push(Tok::Word((b[i] as char).to_string()));
                i += 1;
            }
            c if c.is_ascii_whitespace() => i += 1,
            _ => {
                let start = i;
                while i < b.len() && !b[i].is_ascii_whitespace() && !b"([]".contains(&b[i]) {
                    i += 1;
                }
                out.push(Tok::Word(
                    String::from_utf8_lossy(&b[start..i]).into_owned(),
                ));
            }
        }
    }
    out
}

/// Font resource name -> base font (R's font objects, or our fixed ones).
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
            if let (Some(name), Some("/BaseFont"), Some(base)) = (w.next(), w.next(), w.next()) {
                map.insert(name.to_string(), base.trim_start_matches('/').to_string());
            }
        }
    }
    map
}

fn texts(pdf: &[u8]) -> Vec<Text> {
    let latin1: String = pdf.iter().map(|&c| c as char).collect();
    let fonts = base_fonts(&latin1);
    let mut out = Vec::new();
    let mut rest = pdf;
    while let Some(i) = rest.windows(7).position(|w| w == b"stream\n") {
        let body = &rest[i + 7..];
        let j = body
            .windows(9)
            .position(|w| w == b"endstream")
            .unwrap_or(body.len());
        let content = &body[..j];
        rest = &body[j..];
        let (mut font, mut tf, mut m) = (String::new(), 0.0, [0.0; 6]);
        let mut nums: Vec<f64> = Vec::new();
        let mut array: Option<Vec<Piece>> = None;
        let mut last_str = Vec::new();
        let mut last_name = String::new();
        for t in tokens(content) {
            let w = match t {
                Tok::Str(s) => {
                    match &mut array {
                        Some(a) => a.push(Piece::Str(s)),
                        None => last_str = s,
                    }
                    continue;
                }
                Tok::Word(w) => w,
            };
            if let Ok(v) = w.parse::<f64>() {
                match &mut array {
                    Some(a) => a.push(Piece::Kern(v)),
                    None => nums.push(v),
                }
                continue;
            }
            let mut push = |pieces: Vec<Piece>, m: &[f64; 6]| {
                out.push(Text {
                    font: font.clone(),
                    size: tf * (m[0] * m[0] + m[1] * m[1]).sqrt(),
                    origin: (m[4], m[5]),
                    pieces,
                })
            };
            match w.as_str() {
                "[" => array = Some(Vec::new()),
                "]" => {}
                "Tf" => {
                    font = fonts.get(&last_name).cloned().unwrap_or_default();
                    tf = nums[nums.len() - 1];
                }
                "Tm" => m = nums[nums.len() - 6..].try_into().unwrap(),
                "Tj" => push(vec![Piece::Str(std::mem::take(&mut last_str))], &m),
                "TJ" => push(array.take().unwrap_or_default(), &m),
                w if w.starts_with('/') => last_name = w.to_string(),
                _ => {}
            }
            nums.clear();
        }
    }
    out
}

fn r_string(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            c if c.is_ascii() => out.push(c),
            c => out.push_str(&format!("\\u{:04x}", c as u32)),
        }
    }
    out.push('"');
    out
}

#[test]
fn kerned_strings_match_r() {
    if !common::r_available() {
        return;
    }
    let dir = std::env::temp_dir();
    let r_pdf = dir.join("rs_graphics_kerning.pdf");
    let mut script = format!(
        "pdf(file = '{}', width = {PAGE_W}, height = {PAGE_H}, compress = FALSE)\n\
         plot.new(); plot.window(c(0, 10), c(0, 10))\n",
        r_pdf.display()
    );
    for &(x, y, s, font, cex, srt) in ITEMS {
        script.push_str(&format!(
            "text({x}, {y}, {}, font = {font}, cex = {cex}, srt = {srt})\n",
            r_string(s)
        ));
    }
    script.push_str("invisible(dev.off())\n");
    let script_path = dir.join("rs_graphics_kerning.R");
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
    d.plot_new();
    d.plot_window().xlim(0, 10).ylim(0, 10).draw();
    for &(x, y, s, font, cex, srt) in ITEMS {
        d.text()
            .x([x])
            .y([y])
            .labels([s])
            .font(font)
            .cex(cex)
            .srt(srt)
            .draw();
    }
    let ours = texts(&d.into_backend().finish());
    let r = texts(&std::fs::read(&r_pdf).unwrap());

    assert_eq!(ours.len(), r.len(), "text count\nours={ours:#?}\nr={r:#?}");
    assert!(
        r.iter().filter(|t| t.pieces.len() > 1).count() >= 8,
        "too few kerned strings in R's output: {r:#?}"
    );
    for (a, b) in ours.iter().zip(&r) {
        let ctx = format!("\n ours={a:?}\n r=   {b:?}");
        assert_eq!(a.font, b.font, "font differs{ctx}");
        assert!((a.size - b.size).abs() < 0.01, "size differs{ctx}");
        assert!(
            (a.origin.0 - b.origin.0).abs() < 0.05 && (a.origin.1 - b.origin.1).abs() < 0.05,
            "origin differs{ctx}"
        );
        assert_eq!(a.pieces, b.pieces, "pieces differ{ctx}");
    }
}

/// R never applies the last kerning pair of each first character (its
/// bug); we do: `F r` is Helvetica's last pair for `F`, in "Frequency".
#[test]
fn last_pair_of_a_character_is_kerned() {
    let mut d = Device::new(
        PdfBackend::new(PAGE_W, PAGE_H).with_compression(false),
        Par::default(),
    );
    d.plot_new();
    d.plot_window().xlim(0, 10).ylim(0, 10).draw();
    d.text().x([5]).y([5]).labels(["Fr"]).draw();
    let t = texts(&d.into_backend().finish());
    assert_eq!(
        t[0].pieces,
        vec![
            Piece::Str(b"F".to_vec()),
            Piece::Kern(45.0),
            Piece::Str(b"r".to_vec())
        ]
    );
}
