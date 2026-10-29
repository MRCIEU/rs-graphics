// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! A `Figure` holding a whole plot: page layout (`mfrow`, `layout()`,
//! `fig`, `fin`, `mfg`) and several pages, recorded once and drawn on any
//! device with the same output as making the calls there (R's
//! `recordPlot()`/`replayPlot()`). (No R needed: drawing the calls directly
//! is what the R cross-checks test.)

use rs_graphics::prelude::*;

/// A two-page plot: a 2 x 2 grid with an outer title over five panels
/// (so a second page), then a `layout()` page and a `fig` inset.
fn whole_plot<G: Graphics>(g: &mut G) {
    let x = [1.0, 2.0, 2.5, 3.0, 4.5, 5.0, 6.5];
    g.par().oma([0, 0, 2, 0]).mfrow(2, 2).draw();
    g.plot().x(x).main("one").draw();
    g.hist().x(x).draw();
    g.boxplot().x(x).draw();
    g.barplot().height([3, 5, 2]).names(["a", "b", "c"]).draw();
    g.mtext().text("Page one").outer(true).line(0.5).draw();
    g.plot().x(x).y(x).main("five").draw();
    g.layout().mat(&[[1, 1], [2, 3]]).heights([2, 1]).draw();
    g.plot().x(x).type_("l").draw();
    g.pie().x([1, 2, 3]).draw();
    g.par().mfg(2, 1).draw();
    g.plot().x([3, 1, 2]).main("back in panel 2").draw();
    g.par().fig([0.6, 0.95, 0.6, 0.95]).new(true).draw();
    g.plot().x([1, 2]).draw();
}

fn bytes(format: &str, recorded: bool) -> Vec<u8> {
    let mut dev = Device::memory(format)
        .width(8)
        .height(7)
        .compress(false)
        .open()
        .unwrap();
    if recorded {
        let mut fig = Figure::new();
        whole_plot(&mut fig);
        dev.add_figure(&fig);
    } else {
        whole_plot(&mut dev);
    }
    dev.finish()
}

#[test]
fn a_recorded_whole_plot_draws_as_the_calls_do() {
    let (pdf, svg, png) = ("pdf", "svg", "png");
    assert_eq!(bytes(pdf, true), bytes(pdf, false), "pdf");
    assert_eq!(bytes(svg, true), bytes(svg, false), "svg");
    assert_eq!(bytes(png, true), bytes(png, false), "png");
    // The whole plot is there: two grid pages and the layout page.
    let pdf_out = String::from_utf8_lossy(&bytes(pdf, true)).into_owned();
    assert!(pdf_out.contains("/Count 3"), "three pages");
}

#[test]
fn layout_errors_are_found_when_recording() {
    let mut fig = Figure::new();
    let message = |e: rs_graphics::Error| (e.call().to_string(), e.message().to_string());
    assert_eq!(
        message(fig.par().mfrow(0, 2).try_draw().unwrap_err()),
        (
            "par".into(),
            "invalid value specified for graphical parameter \"mfrow\"".into()
        )
    );
    assert_eq!(
        message(fig.par().fig([0.5, 0.2, 0.0, 1.0]).try_draw().unwrap_err()),
        (
            "par".into(),
            "invalid value specified for graphical parameter \"fig\"".into()
        )
    );
    assert_eq!(
        message(fig.par().fin([0, 3]).try_draw().unwrap_err()),
        (
            "par".into(),
            "invalid value specified for graphical parameter \"fin\"".into()
        )
    );
    let e = fig.layout().mat(&[[1, 3]]).try_draw().unwrap_err();
    assert_eq!(e.call(), "layout");
    assert!(fig.is_empty(), "nothing recorded after an error");
    // `layout()` returns the number of figures in a `Figure` too.
    assert_eq!(fig.layout().mat(&[[1, 2], [3, 3]]).draw(), 3);
}

#[test]
fn mfg_outside_the_grid_is_found_when_added() {
    let mut fig = Figure::new();
    fig.par().mfrow(2, 2).draw();
    fig.par().mfg(3, 1).draw(); // recorded: the grid is the device's
    fig.plot().x([1, 2]).draw();
    let mut dev = Device::null().open().unwrap();
    let e = dev.try_add_figure(&fig).unwrap_err();
    assert_eq!(
        e.to_string(),
        "par(): parameter \"i\" in \"mfg\" is out of range"
    );
}

#[test]
fn a_whole_plot_cant_go_into_a_panel() {
    let mut fig = Figure::new();
    fig.par().mfrow(1, 2).draw();
    fig.plot().x([1, 2]).draw();
    let mut dev = Device::null().open().unwrap();
    dev.par().mfrow(2, 2).draw();
    let e = dev.try_add_figure_at(1, 2, &fig).unwrap_err();
    assert_eq!(e.kind(), ErrorKind::InvalidArgument);
    assert!(e.message().contains("can't be added into a panel"), "{e}");
    // `layout()` makes a whole plot too.
    let mut fig = Figure::new();
    fig.layout().mat(&[[1, 2]]).draw();
    assert!(dev.try_add_figure_at(1, 1, &fig).is_err());
}

#[test]
fn par_is_restored_after_a_panel_but_kept_after_a_whole_plot() {
    let mut panel = Figure::new();
    panel.par().las(1).cex(0.5).draw();
    panel.plot().x([1, 2]).draw();
    let mut dev = Device::null().open().unwrap();
    dev.add_figure(&panel);
    assert_eq!((dev.current_par().las, dev.current_par().cex), (0, 1.0));

    let mut whole = Figure::new();
    whole.par().mfrow(2, 2).las(1).draw();
    whole.plot().x([1, 2]).draw();
    let mut dev = Device::null().open().unwrap();
    dev.add_figure(&whole);
    // As R's replayPlot(): the 2 x 2 grid's cex (0.83) and the las stay,
    // so the next plot goes to panel 2 at the grid's size.
    assert_eq!((dev.current_par().las, dev.current_par().cex), (1, 0.83));
    dev.plot().x([1, 2]).draw();
    assert_eq!(dev.current_figure(), 2);
}
