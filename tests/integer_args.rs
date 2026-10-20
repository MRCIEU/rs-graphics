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
//! `inset`), `LegendEntry::line`'s `lwd`, `Layout` sizes, `fig()` and the
//! backend constructors draw byte for byte what the same `f64` values do,
//! on a device and in a recorded `Figure`. No R needed.

use rs_graphics::prelude::*;

/// Every changed kind of argument, with integers.
fn with_integers<G: Graphics>(g: &mut G) {
    g.par().mar([5, 4, 4, 2]).mgp([3, 1, 0]).draw();
    g.plot(&[1, 2, 3, 4], &vec![2u32, 4, 3, 5])
        .pch(21)
        .cex(2)
        .lwd(2)
        .bg("red")
        .xlim(0, 5)
        .ylim(0, 6)
        .xaxp([0, 5, 5])
        .draw();
    g.lines(1..=4, &[1i64, 2, 2, 3]).lwd(3).draw();
    g.points(&[2, 3], &[5, 1]).cex([1, 2]).draw();
    g.segments(&[0], &[0], &[5], &[6]).lwd(2).draw();
    g.arrows(&[1], &[1], &[2], &[2]).length(0.1).draw();
    g.rect(&[1], &[1], &[2], &[2]).lwd(2).draw();
    g.polygon(&[3, 4, 4], &[3, 3, 4]).col("grey").draw();
    g.text(&[1, 2], &[5, 5], &["a", "b"]).cex(2).draw();
    g.abline().h(3).v([1, 2]).coef(0, 1).lwd(2).draw();
    g.axis(3).at([1, 2, 3]).lwd(2).draw();
    let keys = [
        LegendEntry::line("line", "blue", 2, "dashed"),
        LegendEntry::point("point", "black", 21)
            .pt_bg("red")
            .pt_cex(2),
    ];
    g.legend((1, 6), &keys).inset((0, 0)).draw();
    g.legend(((3, 5), (4, 6)), &keys).draw();
    g.plot_window((0, 10), (0, 5)).draw();
    g.hist(&[1, 2, 2, 3, 3, 3]).breaks([0, 1, 2, 3]).draw();
    g.barplot([3, 5, 2]).width([1, 2, 1]).space(1).draw();
    g.barplot(vec![vec![1, 2], vec![3, 4]]).draw();
    g.boxplot(&[vec![1, 2, 3, 9], vec![4, 5, 6]])
        .at([1, 3])
        .draw();
    g.pie(&[1, 2, 3]).radius(1).draw();
}

/// The same with `f64`s.
fn with_floats<G: Graphics>(g: &mut G) {
    g.par()
        .mar([5.0, 4.0, 4.0, 2.0])
        .mgp([3.0, 1.0, 0.0])
        .draw();
    g.plot(&[1.0, 2.0, 3.0, 4.0], &[2.0, 4.0, 3.0, 5.0])
        .pch(21)
        .cex(2.0)
        .lwd(2.0)
        .bg("red")
        .xlim(0.0, 5.0)
        .ylim(0.0, 6.0)
        .xaxp([0.0, 5.0, 5.0])
        .draw();
    g.lines(&[1.0, 2.0, 3.0, 4.0], &[1.0, 2.0, 2.0, 3.0])
        .lwd(3.0)
        .draw();
    g.points(&[2.0, 3.0], &[5.0, 1.0]).cex([1.0, 2.0]).draw();
    g.segments(&[0.0], &[0.0], &[5.0], &[6.0]).lwd(2.0).draw();
    g.arrows(&[1.0], &[1.0], &[2.0], &[2.0]).length(0.1).draw();
    g.rect(&[1.0], &[1.0], &[2.0], &[2.0]).lwd(2.0).draw();
    g.polygon(&[3.0, 4.0, 4.0], &[3.0, 3.0, 4.0])
        .col("grey")
        .draw();
    g.text(&[1.0, 2.0], &[5.0, 5.0], &["a", "b"])
        .cex(2.0)
        .draw();
    g.abline()
        .h(3.0)
        .v([1.0, 2.0])
        .coef(0.0, 1.0)
        .lwd(2.0)
        .draw();
    g.axis(3).at([1.0, 2.0, 3.0]).lwd(2.0).draw();
    let keys = [
        LegendEntry::line("line", "blue", 2.0, "dashed"),
        LegendEntry::point("point", "black", 21)
            .pt_bg("red")
            .pt_cex(2.0),
    ];
    g.legend((1.0, 6.0), &keys).inset((0.0, 0.0)).draw();
    g.legend(((3.0, 5.0), (4.0, 6.0)), &keys).draw();
    g.plot_window((0.0, 10.0), (0.0, 5.0)).draw();
    g.hist(&[1.0, 2.0, 2.0, 3.0, 3.0, 3.0])
        .breaks([0.0, 1.0, 2.0, 3.0])
        .draw();
    g.barplot([3.0, 5.0, 2.0])
        .width([1.0, 2.0, 1.0])
        .space(1.0)
        .draw();
    g.barplot(vec![vec![1.0, 2.0], vec![3.0, 4.0]]).draw();
    g.boxplot(&[vec![1.0, 2.0, 3.0, 9.0], vec![4.0, 5.0, 6.0]])
        .at([1.0, 3.0])
        .draw();
    g.pie(&[1.0, 2.0, 3.0]).radius(1.0).draw();
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
        dev.plot(&[1.0, 2.0], &[1.0, 2.0]).draw();
        if ints {
            dev.par().fig([0, 1, 0, 1]).draw();
        } else {
            dev.par().fig([0.0, 1.0, 0.0, 1.0]).draw();
        }
        dev.par().new(true).draw();
        dev.plot(&[1.0, 2.0], &[2.0, 1.0]).draw();
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
        s(boxplot_stats(&[1, 2, 3, 4, 100], 1.5)),
        s(boxplot_stats(&[1.0, 2.0, 3.0, 4.0, 100.0], 1.5))
    );
    assert_eq!(
        s(boxplot_stats(&[1, 2, 3, 4, 100], 1)),
        s(boxplot_stats(&[1.0, 2.0, 3.0, 4.0, 100.0], 1.0))
    );
}
