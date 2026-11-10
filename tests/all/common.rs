// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Shared by the `*_matches_r` tests.

pub mod pdf;
pub mod r;

use std::process::Command;

/// `Rscript`, with R's messages untranslated (`LANGUAGE=en`), so that
/// tests comparing them don't depend on the locale (R's British English
/// translation writes "colour" where R's own message, and ours, says
/// "color"). Every test that runs R starts it through this.
pub fn rscript() -> Command {
    let mut c = Command::new("Rscript");
    c.env("LANGUAGE", "en");
    c
}

/// Whether `Rscript` is on `PATH`, so the R cross-checks can run. When it
/// isn't, the test is skipped (prints a note and returns `false`), unless
/// `RS_GRAPHICS_REQUIRE_R` is set, as in CI: then it fails, so a missing R
/// can't let the cross-checks pass unnoticed.
pub fn r_available() -> bool {
    let found = Command::new("Rscript")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !found {
        assert!(
            std::env::var_os("RS_GRAPHICS_REQUIRE_R").is_none(),
            "RS_GRAPHICS_REQUIRE_R is set, but Rscript isn't on PATH"
        );
        eprintln!("Rscript not found on PATH; skipping R cross-check.");
    }
    found
}
