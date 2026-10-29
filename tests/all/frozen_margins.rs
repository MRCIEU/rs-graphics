// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! `par(mar)`/`par(oma)` take effect at the next `plot_new()`, for the
//! data and for margin drawing alike. (R places `title()`, `mtext()` and
//! `axis(outer = TRUE)` with the new margins straight away while keeping
//! the data where it was; we don't copy that, so this isn't an R
//! cross-check.)

use rs_graphics::prelude::*;

fn draw(change_margins: bool) -> Vec<u8> {
    let mut d = Device::memory("pdf")
        .width(6)
        .height(5)
        .compress(false)
        .open()
        .unwrap();
    d.par().oma(Sides::from([1.0, 1.0, 2.0, 1.0])).draw();
    d.plot_new();
    d.plot_window().xlim(0.0, 10.0).ylim(0.0, 10.0).draw();
    if change_margins {
        d.par().mar(Sides::from([2.0, 2.0, 8.0, 1.0])).draw();
        d.par().oma(Sides::from([3.0, 0.0, 0.0, 2.0])).draw();
    }
    d.points().x([5.0]).y([5.0]).draw();
    d.title().main("Main").xlab("x").draw();
    d.mtext().text("outer").side(3).outer(true).draw();
    d.mtext().text("inner").side(4).line(1.0).draw();
    d.axis().side(1).outer(true).draw();
    d.box_().which(BoxWhich::Inner).draw();
    d.finish()
}

#[test]
fn margins_changed_after_plot_new_apply_to_the_next_plot() {
    assert_eq!(draw(false), draw(true));
}
