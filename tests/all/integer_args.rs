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
//! `inset`), legend `lwd`/`pt_cex`, `layout()` sizes, `fig()` and device
//! sizes and `res` draw byte for byte what the same `f64` values do,
//! on a device and in a recorded `Figure`, and option setters take every
//! number type (`i64`, `u8`, `usize`, ...) and ranges as data setters do.
//! No R needed.

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
        .xlim([0, 5])
        .ylim([0, 6])
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
    g.plot_window().xlim([0, 10]).ylim([0, 5]).draw();
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
        .xlim([0.0, 5.0])
        .ylim([0.0, 6.0])
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
    g.plot_window().xlim([0.0, 10.0]).ylim([0.0, 5.0]).draw();
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

fn device(w: f64, h: f64) -> Device<AnyBackend> {
    Device::memory("pdf")
        .width(w)
        .height(h)
        .compress(false)
        .open()
        .unwrap()
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

/// Option setters take every number type and ranges, as data does:
/// `i64`, `u8`, `usize`, ... (not only `i32`/`u32`/`f32`).
fn with_other_number_types<G: Graphics>(g: &mut G) {
    g.par().cex(1_u8).mgp([3_i64, 1, 0]).draw();
    g.plot()
        .x(1..=4)
        .y(vec![2_i64, 4, 3, 5])
        .cex(2_u8)
        .lwd([Some(2_u16), None])
        .xlim((0_u64, 5_usize))
        .ylim((0_i8, 6_i16))
        .xaxp([0_u64, 5, 5])
        .draw();
    g.points().x([2, 3]).y([5, 1]).cex(1..3).draw();
    g.abline().h(3_i64).v(vec![1_usize, 2]).lwd(2_u8).draw();
    g.axis().side(3).at(1..=3).tcl(-1_i64).draw();
    g.legend()
        .pos((1_u8, 6_i64))
        .labels(["a", "b"])
        .pch(1)
        .inset((0_u8, 0_usize))
        .draw();
    g.plot_window()
        .xlim((0_u8, 10_u64))
        .ylim((0_i64, 5_i8))
        .draw();
}

fn with_other_number_types_as_floats<G: Graphics>(g: &mut G) {
    g.par().cex(1.0).mgp([3.0, 1.0, 0.0]).draw();
    g.plot()
        .x([1.0, 2.0, 3.0, 4.0])
        .y([2.0, 4.0, 3.0, 5.0])
        .cex(2.0)
        .lwd([Some(2.0), None])
        .xlim([0.0, 5.0])
        .ylim([0.0, 6.0])
        .xaxp([0.0, 5.0, 5.0])
        .draw();
    g.points()
        .x([2.0, 3.0])
        .y([5.0, 1.0])
        .cex([1.0, 2.0])
        .draw();
    g.abline().h(3.0).v([1.0, 2.0]).lwd(2.0).draw();
    g.axis().side(3).at([1.0, 2.0, 3.0]).tcl(-1.0).draw();
    g.legend()
        .pos((1.0, 6.0))
        .labels(["a", "b"])
        .pch(1)
        .inset((0.0, 0.0))
        .draw();
    g.plot_window().xlim([0.0, 10.0]).ylim([0.0, 5.0]).draw();
}

#[test]
fn option_setters_take_every_number_type() {
    let mut a = device(7.0, 6.0);
    with_other_number_types(&mut a);
    let mut b = device(7.0, 6.0);
    with_other_number_types_as_floats(&mut b);
    assert!(a.finish() == b.finish(), "device output differs");

    let (mut fa, mut fb) = (Figure::new(), Figure::new());
    with_other_number_types(&mut fa);
    with_other_number_types_as_floats(&mut fb);
    let (mut a, mut b) = (device(7.0, 6.0), device(7.0, 6.0));
    a.add_figure(&fa);
    b.add_figure(&fb);
    assert!(a.finish() == b.finish(), "figure output differs");
}

#[test]
fn layout_fig_and_backends_take_integers() {
    let render = |ints: bool| {
        let builder = if ints {
            Device::memory("pdf").width(8).height(6)
        } else {
            Device::memory("pdf").width(8.0).height(6.0)
        };
        let mut dev = builder.compress(false).open().unwrap();
        if ints {
            dev.layout().mat(&[[1, 2]]).widths([2, 1]).draw();
        } else {
            dev.layout()
                .mat(&[[1.0, 2.0]])
                .widths([LayoutSize::Relative(2.0), LayoutSize::Relative(1.0)])
                .draw();
        }
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
    let mut dev = Device::memory("svg").width(5).height(5).open().unwrap();
    dev.par().fig([0.0, 0.5, 0.0, 1.0]).draw();
    let png = Device::memory("png").width(2).height(2).res(72);
    assert!(!png.open().unwrap().finish().is_empty());
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

/// Every data argument given single numbers (R's length-one vectors),
/// integers and floats, by value and by reference.
#[allow(clippy::needless_borrows_for_generic_args)] // `&one` on purpose
fn with_single_numbers<G: Graphics>(g: &mut G) {
    let one = 1.0;
    g.plot().x(1).y(&one).xlim([0, 4]).ylim([0, 4]).draw();
    g.points().x(2.0).y(2).draw();
    g.lines().x(1).y(1).draw();
    g.text().x(3).y(3.0).labels("a").draw();
    g.segments().x0(0).y0(0).x1(4).y1(4).draw();
    g.arrows().x0(1).y0(3).x1(2).y1(3.5).draw();
    g.rect().xleft(1).ybottom(1).xright(2).ytop(2).draw();
    g.polygon().x(3).y(1).draw();
    g.hist().x(2).draw();
    g.barplot().height(5).draw();
    g.boxplot().x(3).draw();
    g.pie().x(1).draw();
}

/// The same, as one-element vectors.
fn with_one_element_vectors<G: Graphics>(g: &mut G) {
    g.plot().x([1.0]).y([1.0]).xlim([0, 4]).ylim([0, 4]).draw();
    g.points().x([2.0]).y([2.0]).draw();
    g.lines().x([1.0]).y([1.0]).draw();
    g.text().x([3.0]).y([3.0]).labels(["a"]).draw();
    g.segments().x0([0.0]).y0([0.0]).x1([4.0]).y1([4.0]).draw();
    g.arrows().x0([1.0]).y0([3.0]).x1([2.0]).y1([3.5]).draw();
    g.rect()
        .xleft([1.0])
        .ybottom([1.0])
        .xright([2.0])
        .ytop([2.0])
        .draw();
    g.polygon().x([3.0]).y([1.0]).draw();
    g.hist().x([2.0]).draw();
    g.barplot().height([5.0]).draw();
    g.boxplot().x([vec![3.0]]).draw();
    g.pie().x([1.0]).draw();
}

#[test]
fn single_numbers_are_vectors_of_one() {
    let mut a = device(7.0, 6.0);
    with_single_numbers(&mut a);
    let mut b = device(7.0, 6.0);
    with_one_element_vectors(&mut b);
    assert!(a.finish() == b.finish(), "device output differs");

    let (mut fa, mut fb) = (Figure::new(), Figure::new());
    with_single_numbers(&mut fa);
    with_one_element_vectors(&mut fb);
    let (mut a, mut b) = (device(7.0, 6.0), device(7.0, 6.0));
    a.add_figure(&fa);
    b.add_figure(&fb);
    assert!(a.finish() == b.finish(), "figure output differs");

    // One vector is one box, as R's `boxplot(x)`.
    let mut a = device(7.0, 6.0);
    a.boxplot().x([1, 2, 3, 4, 100]).draw();
    let mut b = device(7.0, 6.0);
    b.boxplot().x([vec![1.0, 2.0, 3.0, 4.0, 100.0]]).draw();
    assert!(a.finish() == b.finish(), "boxplot of one vector differs");
    assert_eq!(
        format!("{:?}", boxplot_stats(5, 1.5)),
        format!("{:?}", boxplot_stats([5.0], 1.5))
    );
}

/// Every two-number setting as arrays: limits, page grid and panel, figure
/// and plot sizes, legend position, corners and inset.
fn pairs_as_arrays<G: Graphics>(g: &mut G) {
    g.par().mfrow([2, 2]).pin([3, 2]).draw();
    g.plot().x([1, 2, 3]).xlim([0, 4]).ylim([0, 5]).draw();
    g.legend().pos([1, 4]).labels("a").inset([0.0, 0.0]).draw();
    g.legend()
        .pos("topright")
        .labels("b")
        .inset([0.05, 0.05])
        .draw();
    g.legend().pos([[2, 3], [1, 2]]).labels("c").draw();
    g.hist().x([1, 2, 2, 3]).xlim([0, 4]).ylim([0, 3]).draw();
    g.barplot().height([1, 2]).xlim([0, 3]).ylim([0, 3]).draw();
    g.boxplot()
        .x([1, 2, 3, 9])
        .xlim([0, 2])
        .ylim([0, 10])
        .draw();
    g.par().mfg([1, 2]).draw();
    g.plot_new();
    g.plot_window().xlim([0, 1]).ylim([0, 1]).draw();
    g.par().fin([3, 2]).draw();
    g.plot_new();
}

/// The same settings as tuples, mixing integers and floats, with R's
/// truncation of fractional grid counts and one number for `inset`.
fn pairs_as_tuples<G: Graphics>(g: &mut G) {
    g.par().mfrow((2.7, 2)).pin((3.0, 2)).draw();
    g.plot().x([1, 2, 3]).xlim((0, 4.0)).ylim((0.0, 5)).draw();
    g.legend().pos((1, 4.0)).labels("a").inset(0).draw();
    g.legend().pos("topright").labels("b").inset(0.05).draw();
    g.legend().pos(((2, 3.0), (1.0, 2))).labels("c").draw();
    g.hist()
        .x([1, 2, 2, 3])
        .xlim((0.0, 4))
        .ylim((0, 3.0))
        .draw();
    g.barplot()
        .height([1, 2])
        .xlim((0, 3.0))
        .ylim((0.0, 3))
        .draw();
    g.boxplot()
        .x([1, 2, 3, 9])
        .xlim((0, 2.0))
        .ylim((0.0, 10))
        .draw();
    g.par().mfg((1.9, 2)).draw();
    g.plot_new();
    g.plot_window().xlim((0, 1.0)).ylim((0.0, 1)).draw();
    g.par().fin((3.0, 2)).draw();
    g.plot_new();
}

/// Two-number settings take an array or a tuple (mixed types), `inset`
/// one number too, and grid counts are truncated as R's `as.integer()`.
#[test]
fn pair_settings_take_arrays_or_tuples() {
    let mut a = device(7.0, 6.0);
    pairs_as_arrays(&mut a);
    let mut b = device(7.0, 6.0);
    pairs_as_tuples(&mut b);
    assert!(a.finish() == b.finish(), "device output differs");

    let (mut fa, mut fb) = (Figure::new(), Figure::new());
    pairs_as_arrays(&mut fa);
    pairs_as_tuples(&mut fb);
    let (mut a, mut b) = (device(7.0, 6.0), device(7.0, 6.0));
    a.add_figure(&fa);
    b.add_figure(&fb);
    assert!(a.finish() == b.finish(), "figure output differs");
}
