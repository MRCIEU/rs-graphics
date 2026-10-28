// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Running R and our device for the `*_matches_r` tests.

use std::process::Command;

use rs_graphics::prelude::*;

/// `v` as R code: `c(1, 2.5, 3)`.
pub fn r_vec(v: &[f64]) -> String {
    let v: Vec<String> = v.iter().map(|x| x.to_string()).collect();
    format!("c({})", v.join(", "))
}

/// What one R run gave: each case's PDF, and what the script printed.
pub struct RRun {
    pub pdfs: Vec<Vec<u8>>,
    pub stdout: String,
}

/// Runs `prelude`, then each case's R code on its own uncompressed
/// `pdf()` device of `page` inches, in one `Rscript`. Files are named
/// `rs_graphics_<tag>_<case>.pdf` in the temp directory.
pub fn run_r(tag: &str, page: (f64, f64), prelude: &str, cases: &[(&str, &str)]) -> RRun {
    let sized: Vec<_> = cases
        .iter()
        .map(|&(name, code)| (name, page, code))
        .collect();
    run_r_sized(tag, prelude, &sized)
}

/// [`run_r`] with a page size per case.
pub fn run_r_sized(tag: &str, prelude: &str, cases: &[(&str, (f64, f64), &str)]) -> RRun {
    let dir = std::env::temp_dir();
    let path = |name: &str| dir.join(format!("rs_graphics_{tag}_{name}.pdf"));
    let mut script = format!("{prelude}\n");
    for (name, page, code) in cases {
        script.push_str(&format!(
            "pdf(file = '{}', width = {}, height = {}, compress = FALSE)\n{code}\ninvisible(dev.off())\n",
            path(name).display(),
            page.0,
            page.1
        ));
    }
    let script_path = dir.join(format!("rs_graphics_{tag}.R"));
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
    RRun {
        pdfs: cases
            .iter()
            .map(|(name, _, _)| std::fs::read(path(name)).unwrap())
            .collect(),
        stdout: String::from_utf8(out.stdout).unwrap(),
    }
}

/// Draws with `f` on an uncompressed PDF device of `page` inches with the
/// default `par()`; returns the file and what `f` returned.
pub fn ours<T>(page: (f64, f64), f: impl FnOnce(&mut Device<PdfBackend>) -> T) -> (Vec<u8>, T) {
    ours_with(page, Par::default(), f)
}

/// [`ours`], starting from `par`.
pub fn ours_with<T>(
    page: (f64, f64),
    par: Par,
    f: impl FnOnce(&mut Device<PdfBackend>) -> T,
) -> (Vec<u8>, T) {
    let mut dev = Device::new(PdfBackend::new(page.0, page.1).with_compression(false), par);
    let value = f(&mut dev);
    (dev.into_backend().finish(), value)
}
