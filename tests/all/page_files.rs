// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! One file per page, as R's `png("Rplot%03d.png")`: a PNG or SVG file
//! name with a C integer format gets one file per page, each the page as
//! drawn on its own; without one the file holds the last page; a PDF keeps
//! every page in one file named for page 1, or with `onefile(false)` one
//! file per page as R's `pdf(onefile = FALSE)` (file names and page counts
//! cross-checked with R). Page numbers are formatted as
//! R's `sprintf()` (cross-checked with R when it's available), and names
//! with other `%` conversions are refused at `open()`, as R's
//! `checkIntFormat()`.

use crate::common;

use std::path::PathBuf;
use std::process::Command;

use rs_graphics::prelude::*;

fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir()
        .join("rs_graphics_page_files")
        .join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Page `k` (from 0) of the test drawing.
fn page<B: Backend>(dev: &mut Device<B>, k: usize) {
    let y: Vec<f64> = (0..5).map(|i| ((i + k) % 5) as f64).collect();
    dev.plot()
        .x([1, 2, 3, 4, 5])
        .y(&y)
        .pch(1 + k as i32)
        .main(format!("page {}", k + 1))
        .draw();
}

/// Page `k` drawn alone, on the backend an opened device would use.
fn alone(format: &str, k: usize, bg: Option<Color>) -> Vec<u8> {
    let backend: AnyBackend = match format {
        "png" => RasterBackend::new(4, 3, 72, bg.unwrap_or(Color::WHITE)).into(),
        "pdf" => PdfBackend::new(4, 3).into(),
        _ => SvgBackend::new(4, 3).into(),
    };
    let mut dev = Device::new(backend, Par::default());
    if let Some(bg) = bg {
        dev.par().bg(bg).draw();
    }
    page(&mut dev, k);
    dev.finish()
}

#[test]
fn numbered_png_and_svg_pages_get_a_file_each() {
    for format in ["png", "svg"] {
        let d = dir(format);
        let mut dev = Device::format(format, d.join(format!("Rplot%03d.{format}")))
            .width(4)
            .height(3)
            .open()
            .unwrap();
        for k in 0..3 {
            page(&mut dev, k);
        }
        dev.close().unwrap();
        let mut names: Vec<String> = std::fs::read_dir(&d)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        let want: Vec<String> = (1..=3).map(|k| format!("Rplot{k:03}.{format}")).collect();
        assert_eq!(names, want, "{format}");
        for (k, name) in want.iter().enumerate() {
            let got = std::fs::read(d.join(name)).unwrap();
            assert!(got == alone(format, k, None), "{format} page {}", k + 1);
        }
    }
}

#[test]
fn pages_follow_the_page_layout() {
    // Three plots on a 1 x 2 page: two pages.
    let d = dir("mfrow");
    let mut dev = Device::png(d.join("p%d.png"))
        .width(4)
        .height(3)
        .open()
        .unwrap();
    dev.par().mfrow(1, 2).draw();
    for k in 0..3 {
        page(&mut dev, k);
    }
    dev.close().unwrap();
    assert!(d.join("p1.png").exists() && d.join("p2.png").exists());
    assert!(!d.join("p3.png").exists());
}

#[test]
fn without_a_page_number_the_file_holds_the_last_page() {
    // Also with a transparent background, where the earlier pages must not
    // show through.
    for bg in [None, Some(Color::TRANSPARENT)] {
        let d = dir(if bg.is_some() {
            "last_transparent"
        } else {
            "last"
        });
        let path = d.join("plot.png");
        let mut builder = Device::png(&path).width(4).height(3);
        if let Some(c) = bg {
            builder = builder.bg(c);
        }
        let mut dev = builder.open().unwrap();
        for k in 0..3 {
            page(&mut dev, k);
        }
        dev.close().unwrap();
        assert_eq!(std::fs::read_dir(&d).unwrap().count(), 1);
        assert!(
            std::fs::read(&path).unwrap() == alone("png", 2, bg),
            "bg {bg:?}"
        );
    }
}

#[test]
fn a_pdf_keeps_its_pages_in_one_file_named_for_page_one() {
    let d = dir("pdf");
    let mut dev = Device::pdf(d.join("Rplot%03d.pdf")).open().unwrap();
    for k in 0..3 {
        page(&mut dev, k);
    }
    dev.close().unwrap();
    let pdf = std::fs::read(d.join("Rplot001.pdf")).unwrap();
    assert!(String::from_utf8_lossy(&pdf).contains("/Count 3"));
    assert_eq!(std::fs::read_dir(&d).unwrap().count(), 1);
}

/// The files in `d`, sorted.
fn files(d: &std::path::Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(d)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    names
}

#[test]
fn a_pdf_with_onefile_false_gets_a_file_per_page() {
    let d = dir("pdf_onefile_false");
    let mut dev = Device::pdf(d.join("Rplot%03d.pdf"))
        .width(4)
        .height(3)
        .onefile(false)
        .open()
        .unwrap();
    for k in 0..3 {
        page(&mut dev, k);
    }
    dev.close().unwrap();
    let want: Vec<String> = (1..=3).map(|k| format!("Rplot{k:03}.pdf")).collect();
    assert_eq!(files(&d), want);
    for (k, name) in want.iter().enumerate() {
        let got = std::fs::read(d.join(name)).unwrap();
        assert!(got == alone("pdf", k, None), "page {}", k + 1);
    }

    // Without a page number, the file holds the last page.
    let d = dir("pdf_onefile_false_last");
    let mut dev = Device::pdf(d.join("plot.pdf"))
        .width(4)
        .height(3)
        .onefile(false)
        .open()
        .unwrap();
    for k in 0..3 {
        page(&mut dev, k);
    }
    dev.close().unwrap();
    assert_eq!(files(&d), ["plot.pdf"]);
    assert!(std::fs::read(d.join("plot.pdf")).unwrap() == alone("pdf", 2, None));
}

#[test]
fn onefile_is_for_pdf_only() {
    let d = dir("onefile_png");
    let mut dev = Device::png(d.join("p%d.png"))
        .width(4)
        .height(3)
        .onefile(true)
        .open()
        .unwrap();
    for k in 0..2 {
        page(&mut dev, k);
    }
    dev.close().unwrap();
    assert_eq!(files(&d), ["p1.png", "p2.png"]);
}

/// R's `pdf()` with and without `onefile`: the same files, with the same
/// numbers of pages.
#[test]
fn pdf_files_and_pages_match_r() {
    if !common::r_available() {
        return;
    }
    for onefile in [true, false] {
        let r_dir = dir(&format!("r_pdf_{onefile}"));
        let ours = dir(&format!("our_pdf_{onefile}"));
        let flag = if onefile { "TRUE" } else { "FALSE" };
        let script = format!(
            "pdf(file = '{}', onefile = {flag}, width = 4, height = 3)\n\
             for (k in 1:3) plot(1:5, main = k)\ninvisible(dev.off())\n",
            r_dir.join("Rplot%03d.pdf").display()
        );
        let path = std::env::temp_dir().join(format!("rs_graphics_page_files_{onefile}.R"));
        std::fs::write(&path, script).unwrap();
        let out = Command::new("Rscript")
            .arg(&path)
            .output()
            .expect("run Rscript");
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );

        let mut dev = Device::pdf(ours.join("Rplot%03d.pdf"))
            .width(4)
            .height(3)
            .onefile(onefile)
            .open()
            .unwrap();
        for k in 0..3 {
            page(&mut dev, k);
        }
        dev.close().unwrap();

        assert_eq!(files(&ours), files(&r_dir), "onefile = {flag}");
        // R's files are compressed too; its page tree says how many pages.
        let pages = |d: &std::path::Path| -> Vec<bool> {
            files(d)
                .iter()
                .map(|f| {
                    let pdf =
                        String::from_utf8_lossy(&std::fs::read(d.join(f)).unwrap()).into_owned();
                    pdf.contains(if onefile { "/Count 3" } else { "/Count 1" })
                })
                .collect()
        };
        assert!(pages(&r_dir).iter().all(|&b| b), "R, onefile = {flag}");
        assert!(pages(&ours).iter().all(|&b| b), "ours, onefile = {flag}");
    }
}

#[test]
fn names_with_other_conversions_are_refused() {
    let d = dir("invalid");
    for bad in ["a%s.png", "a%d%d.png", "a%.png", "a%f.png"] {
        let err = Device::png(d.join(bad)).open().map(|_| ()).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput, "{bad}");
        assert!(err.to_string().contains("invalid 'file' argument"), "{err}");
    }
    // `%%` is a `%`.
    let dev = Device::png(d.join("100%%.png")).open().unwrap();
    dev.close().unwrap();
    assert!(d.join("100%.png").exists());
}

/// Formats accepted by R's `checkIntFormat()`, numbered as R's `sprintf()`
/// (which, unlike the C `snprintf()` R's devices use, has no `%u`).
const FORMATS: &[&str] = &[
    "p%d", "p%03d", "p%5d", "p%-4d|", "p%+d", "p% d", "p%.3d", "p%5.3d", "p%i", "p%o", "p%#o",
    "p%x", "p%#x", "p%X", "p%#X", "p%-05d|", "p%05.3d", "q%%r%02d", "%d.%%",
];

#[test]
fn page_numbers_are_formatted_as_rs_sprintf() {
    if !common::r_available() {
        return;
    }
    let pages = [1, 9, 12];
    let mut script = String::new();
    for f in FORMATS {
        for p in pages {
            script.push_str(&format!("cat(sprintf('{f}', {p}L), '\\n', sep = '')\n"));
        }
    }
    let path = std::env::temp_dir().join("rs_graphics_page_files.R");
    std::fs::write(&path, script).unwrap();
    let out = Command::new("Rscript")
        .arg(&path)
        .output()
        .expect("run Rscript");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let r = String::from_utf8(out.stdout).unwrap();
    let mut r = r.lines();

    // Twelve pages each: one file per page, and the files R's `sprintf()`
    // names for pages 1, 9 and 12 are among them.
    for (n, f) in FORMATS.iter().enumerate() {
        let d = dir(&format!("fmt{n}"));
        let mut dev = Device::png(d.join(f)).width(3).height(3).open().unwrap();
        for _ in 0..12 {
            dev.plot_new();
        }
        dev.close().unwrap();
        assert_eq!(std::fs::read_dir(&d).unwrap().count(), 12, "{f}");
        for p in pages {
            let want = r.next().unwrap();
            assert!(d.join(want).exists(), "{f} page {p}: no file {want:?}");
        }
    }
}
