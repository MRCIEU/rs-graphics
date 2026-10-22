// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Integers wherever R takes numbers: data vectors (`&[1, 2, 3]`, other
//! integer types, ranges), recycled and single-value setters, limits,
//! arrays (`mar`, `mgp`, `xaxp`), pairs (`plot_window`, legend positions,
//! `inset`), legend `lwd`/`pt_cex`, `Layout` sizes, `fig()` and the
//! backend constructors draw byte for byte what the same `f64` values do,
//! on a device and in a recorded `Figure`. No R needed.

use rs_graphics::prelude::*;

/// Every changed kind of argument, with integers.
fn with_integers<G: Graphics>(g: &mut G) {
    g.par().mar([5, 4, 4, 2]).mgp([3, 1, 0]).draw();
    g.plot()
        .x([1, 2, 3, 4])
        .y(vec![2u32, 4, 3, 5])
        .pch(21)
        .cex(2)
        .lwd(2)
        .bg("red")
        .xlim(0, 5)
        .ylim(0, 6)
        .xaxp([0, 5, 5])
        .draw();
    g.lines().x(1..=4).y([1i64, 2, 2, 3]).lwd(3).draw();
    g.points().x([2, 3]).y([5, 1]).cex([1, 2]).draw();
    g.segments().x0([0]).y0([0]).x1([5]).y1([6]).lwd(2).draw();
    g.arrows()
        .x0([1])
        .y0([1])
        .x1([2])
        .y1([2])
        .length(0.1)
        .draw();
    g.rect()
        .xleft([1])
        .ybottom([1])
        .xright([2])
        .ytop([2])
        .lwd(2)
        .draw();
    g.polygon().x([3, 4, 4]).y([3, 3, 4]).col("grey").draw();
    g.text()
        .x([1, 2])
        .y([5, 5])
        .labels(["a", "b"])
        .cex(2)
        .draw();
    g.abline().h(3).v([1, 2]).a(0).b(1).lwd(2).draw();
    g.axis().side(3).at([1, 2, 3]).lwd(2).draw();
    let keys = LegendOptions::default()
        .col(["blue", "black"])
        .lty([Some("dashed"), None])
        .lwd(2)
        .pch([None, Some(21)])
        .pt_bg("red")
        .pt_cex(2);
    g.legend()
        .pos((1, 6))
        .labels(["line", "point"])
        .with(&keys)
        .inset((0, 0))
        .draw();
    g.legend()
        .pos(((3, 5), (4, 6)))
        .labels(["line", "point"])
        .with(&keys)
        .draw();
    g.plot_window().xlim(0, 10).ylim(0, 5).draw();
    g.hist().x([1, 2, 2, 3, 3, 3]).breaks([0, 1, 2, 3]).draw();
    g.barplot()
        .height([3, 5, 2])
        .width([1, 2, 1])
        .space(1)
        .draw();
    g.barplot().height(vec![vec![1, 2], vec![3, 4]]).draw();
    g.boxplot()
        .x(&[vec![1, 2, 3, 9], vec![4, 5, 6]])
        .at([1, 3])
        .draw();
    g.pie().x([1, 2, 3]).radius(1).draw();
}

/// The same with `f64`s.
fn with_floats<G: Graphics>(g: &mut G) {
    g.par()
        .mar([5.0, 4.0, 4.0, 2.0])
        .mgp([3.0, 1.0, 0.0])
        .draw();
    g.plot()
        .x([1.0, 2.0, 3.0, 4.0])
        .y([2.0, 4.0, 3.0, 5.0])
        .pch(21)
        .cex(2.0)
        .lwd(2.0)
        .bg("red")
        .xlim(0.0, 5.0)
        .ylim(0.0, 6.0)
        .xaxp([0.0, 5.0, 5.0])
        .draw();
    g.lines()
        .x([1.0, 2.0, 3.0, 4.0])
        .y([1.0, 2.0, 2.0, 3.0])
        .lwd(3.0)
        .draw();
    g.points()
        .x([2.0, 3.0])
        .y([5.0, 1.0])
        .cex([1.0, 2.0])
        .draw();
    g.segments()
        .x0([0.0])
        .y0([0.0])
        .x1([5.0])
        .y1([6.0])
        .lwd(2.0)
        .draw();
    g.arrows()
        .x0([1.0])
        .y0([1.0])
        .x1([2.0])
        .y1([2.0])
        .length(0.1)
        .draw();
    g.rect()
        .xleft([1.0])
        .ybottom([1.0])
        .xright([2.0])
        .ytop([2.0])
        .lwd(2.0)
        .draw();
    g.polygon()
        .x([3.0, 4.0, 4.0])
        .y([3.0, 3.0, 4.0])
        .col("grey")
        .draw();
    g.text()
        .x([1.0, 2.0])
        .y([5.0, 5.0])
        .labels(["a", "b"])
        .cex(2.0)
        .draw();
    g.abline()
        .h(3.0)
        .v([1.0, 2.0])
        .a(0.0)
        .b(1.0)
        .lwd(2.0)
        .draw();
    g.axis().side(3).at([1.0, 2.0, 3.0]).lwd(2.0).draw();
    let keys = LegendOptions::default()
        .col(["blue", "black"])
        .lty([Some("dashed"), None])
        .lwd(2.0)
        .pch([None, Some(21)])
        .pt_bg("red")
        .pt_cex(2.0);
    g.legend()
        .pos((1.0, 6.0))
        .labels(["line", "point"])
        .with(&keys)
        .inset((0.0, 0.0))
        .draw();
    g.legend()
        .pos(((3.0, 5.0), (4.0, 6.0)))
        .labels(["line", "point"])
        .with(&keys)
        .draw();
    g.plot_window().xlim(0.0, 10.0).ylim(0.0, 5.0).draw();
    g.hist()
        .x([1.0, 2.0, 2.0, 3.0, 3.0, 3.0])
        .breaks([0.0, 1.0, 2.0, 3.0])
        .draw();
    g.barplot()
        .height([3.0, 5.0, 2.0])
        .width([1.0, 2.0, 1.0])
        .space(1.0)
        .draw();
    g.barplot()
        .height(vec![vec![1.0, 2.0], vec![3.0, 4.0]])
        .draw();
    g.boxplot()
        .x(&[vec![1.0, 2.0, 3.0, 9.0], vec![4.0, 5.0, 6.0]])
        .at([1.0, 3.0])
        .draw();
    g.pie().x([1.0, 2.0, 3.0]).radius(1.0).draw();
}

fn device(w: f64, h: f64) -> Device<PdfBackend> {
    Device::new(
        PdfBackend::new(w, h).with_compression(false),
        Par::default(),
    )
}

#[test]
fn integers_draw_the_same_as_floats() {
    let mut a = device(7.0, 6.0);
    with_integers(&mut a);
    let mut b = device(7.0, 6.0);
    with_floats(&mut b);
    assert!(a.finish() == b.finish(), "device output differs");

    let (mut fa, mut fb) = (Figure::new(), Figure::new());
    with_integers(&mut fa);
    with_floats(&mut fb);
    let (mut a, mut b) = (device(7.0, 6.0), device(7.0, 6.0));
    a.add_figure(&fa);
    b.add_figure(&fb);
    assert!(a.finish() == b.finish(), "figure output differs");
}

#[test]
fn layout_fig_and_backends_take_integers() {
    let render = |ints: bool| {
        let backend = if ints {
            PdfBackend::new(8, 6)
        } else {
            PdfBackend::new(8.0, 6.0)
        };
        let mut dev = Device::new(backend.with_compression(false), Par::default());
        let layout = Layout::new(&[vec![1, 2]]).unwrap();
        let layout = if ints {
            layout.widths(&[2, 1])
        } else {
            layout.widths(&[LayoutSize::Relative(2.0), LayoutSize::Relative(1.0)])
        };
        dev.layout(layout);
        dev.plot().x([1.0, 2.0]).y([1.0, 2.0]).draw();
        if ints {
            dev.par().fig([0, 1, 0, 1]).draw();
        } else {
            dev.par().fig([0.0, 1.0, 0.0, 1.0]).draw();
        }
        dev.par().new(true).draw();
        dev.plot().x([1.0, 2.0]).y([2.0, 1.0]).draw();
        dev.finish()
    };
    assert!(render(true) == render(false));
    // Mixed integers and floats in one array literal are floats.
    let mut dev = Device::new(SvgBackend::new(5, 5), Par::default());
    dev.par().fig([0.0, 0.5, 0.0, 1.0]).draw();
    let raster = RasterBackend::new(2, 2, 72, Color::WHITE);
    assert!(!Device::new(raster, Par::default()).finish().is_empty());
}

#[test]
fn boxplot_stats_takes_integers() {
    let s = |b: BoxStats| format!("{b:?}");
    assert_eq!(
        s(boxplot_stats([1, 2, 3, 4, 100], 1.5)),
        s(boxplot_stats([1.0, 2.0, 3.0, 4.0, 100.0], 1.5))
    );
    assert_eq!(
        s(boxplot_stats([1, 2, 3, 4, 100], 1)),
        s(boxplot_stats([1.0, 2.0, 3.0, 4.0, 100.0], 1.0))
    );
}
