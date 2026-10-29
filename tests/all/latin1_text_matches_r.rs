// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks non-ASCII (Latin-1) text against a real R `pdf()` device's
//! uncompressed output: the bytes written (R encodes e.g. `±` as `\261`)
//! (R writes them raw, we as octal escapes; both decode to the same bytes)
//! with the rest of each text item (see `common::pdf::assert_same`), and
//! so the text origins of left-, centre- and right-justified strings, which
//! depend on the glyphs' AFM widths (looked up by Latin-1 glyph name, not by
//! the AFM's Standard Encoding codes, where 177 is `endash`). Skipped if
//! `Rscript` isn't available.

use crate::common;
use crate::common::{pdf, r};

use rs_graphics::prelude::*;

const LABELS: &[&str] = &["beta ± 1SE", "café crème", "Größe ×2", "© 1999 ¼ ½ ¾", "ÀÉÎÕÜ àéîõü"];

#[test]
fn latin1_text_matches_r() {
    if !common::r_available() {
        return;
    }
    let page = (6.0, 6.0);
    let mut script =
        String::from("par(mar = c(0, 0, 0, 0))\nplot.new(); plot.window(c(0, 10), c(0, 16))\n");
    for (i, label) in LABELS.iter().enumerate() {
        for (j, adj) in [0.0, 0.5, 1.0].iter().enumerate() {
            script.push_str(&format!(
                "text(5, {}, '{label}', adj = c({adj}, 0), font = {})\n",
                3 * i + j + 1,
                1 + (i % 2)
            ));
        }
    }
    let r_pdf = r::run_r("latin1", page, "", &[("all", script.as_str())])
        .pdfs
        .remove(0);
    let (ours, ()) = r::ours(page, |dev| {
        dev.par().mar([0, 0, 0, 0]).draw();
        dev.plot_new();
        dev.plot_window().xlim([0.0, 10.0]).ylim([0.0, 16.0]).draw();
        for (i, label) in LABELS.iter().enumerate() {
            for (j, adj) in [0.0, 0.5, 1.0].iter().enumerate() {
                dev.text()
                    .x([5.0])
                    .y([(3 * i + j + 1) as f64])
                    .labels([label])
                    .adj(*adj)
                    .vadj(0.0)
                    .font(1 + (i % 2) as i32)
                    .draw();
            }
        }
    });
    let (ours, r) = (pdf::parse(&ours), pdf::parse(&r_pdf));
    assert_eq!(r.pages[0].texts.len(), 3 * LABELS.len());
    pdf::assert_same_pdf("latin1", ours, r, &pdf::Compare::default());
}
