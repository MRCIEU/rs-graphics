// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks default axis tick labels for ranges that straddle zero
//! against a real R `pdf()` device: R snaps a tick within `range / (100 *
//! n)` of zero to exactly 0 (`CreateAtVector()`), so e.g. -0.2..0.4 is
//! labelled `-0.2 -0.1 0.0 0.1 ...`, not `... 2.775558e-17 ...` (which also
//! turned every label scientific and made them overlap). Compares every
//! painted path and text item (see `common::pdf::assert_same`), except the
//! origins of negative labels, which are laid out slightly differently in
//! PDF output because R's `pdf()` draws `-` as the wider `/minus` glyph
//! (a quirk of R's PDF device we deliberately don't copy); the page is wide enough that this doesn't
//! change which labels `gap.axis` drops. Skipped if `Rscript` isn't
//! available.

use crate::common;
use crate::common::{pdf, r};


use rs_graphics::prelude::*;

type Range = (f64, f64);

/// `(xlim, ylim)` of each panel.
const RANGES: &[(Range, Range)] = &[
    ((-0.2347, 0.4427), (-0.3, 0.9)),
    ((-0.7, 0.3), (-3.0, 1.2)),
    ((-1e-3, 3e-3), (-0.15, 0.06)),
];

#[test]
fn zero_ticks_match_r() {
    if !common::r_available() {
        return;
    }
    let page = (15.0, 5.0);
    let mut script = String::from("par(mfrow = c(1, 3))\n");
    for ((x0, x1), (y0, y1)) in RANGES {
        script.push_str(&format!(
            "plot.new(); plot.window(c({x0}, {x1}), c({y0}, {y1})); axis(1); axis(2)\n"
        ));
    }
    let r_pdf = r::run_r("zero_tick", page, "", &[("all", &script)])
        .pdfs
        .remove(0);
    let (ours, ()) = r::ours(page, |dev| {
        dev.par().mfrow([1, 3]).draw();
        for &(xlim, ylim) in RANGES {
            dev.plot_new();
            dev.plot_window()
                .xlim((xlim.0, xlim.1))
                .ylim((ylim.0, ylim.1))
                .draw();
            dev.axis().side(1).draw();
            dev.axis().side(2).draw();
        }
    });
    let r = pdf::parse(&r_pdf);
    assert!(
        r.pages[0].texts.iter().any(|t| t.string == "0.0"),
        "R labels: {:?}",
        r.pages[0].texts
    );
    let how = pdf::Compare {
        skip_origin: |t| t.string.contains('-'),
        ..Default::default()
    };
    pdf::assert_same_pdf("zero_tick", pdf::parse(&ours), r, &how);
}
