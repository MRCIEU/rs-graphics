// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! A recorded [`Figure`] drawn with `add_figure` gives exactly the output of
//! making the same calls on the device: every kind of call, on all three
//! backends, alone and mixed with direct calls in multi-panel pages, plus
//! `add_figure_at`, `par` being restored, and the results recorded figures
//! return. Also that `legend()`'s R-style positions (`"topright"`,
//! `(x, y)`, corners) draw the same as the `LegendPos` variants, and that
//! `barplot()` shading on a log axis panics on both. (Not an R
//! cross-check: the direct calls are checked against R elsewhere.)

use rs_graphics::prelude::*;

// ---- scenes, written once against the trait ----

/// `par()`, `plot()` and most low-level calls, with plotmath and default
/// text labels.
fn scatter<G: Graphics>(g: &mut G) {
    g.par().mar([4.0, 4.0, 3.0, 2.0]).las(1).draw();
    let x = [1.0, 2.0, 3.0, 4.0, 5.0];
    let y = [2.0, 4.5, 3.0, 6.0, 5.5];
    g.plot()
        .x(x)
        .y(y)
        .pch(19)
        .col(["red", "blue"])
        .main(Label::math("y == alpha^2"))
        .xlab("x")
        .draw();
    g.lines().x(x).y(y).lty(LineType::DASHED).draw();
    g.points().x([2.5]).y([5.0]).pch(4).cex(2.0).draw();
    g.segments()
        .x0([1.0])
        .y0([2.0])
        .x1([5.0])
        .y1([6.0])
        .col("grey")
        .draw();
    g.arrows()
        .x0([1.5, 4.5])
        .y0([5.5, 5.5])
        .x1([1.5, 4.5])
        .y1([3.0, 3.0])
        .angle(90.0)
        .code(3)
        .length(0.05)
        .draw();
    g.rect()
        .xleft([3.2])
        .ybottom([2.2])
        .xright([3.8])
        .ytop([2.8])
        .col("lightblue")
        .draw();
    g.rect()
        .xleft([2.2])
        .ybottom([4.2])
        .xright([2.8])
        .ytop([4.8])
        .density(20)
        .angle(30)
        .draw();
    g.polygon()
        .x([1.2, 1.8, 1.5, f64::NAN, 4.2, 4.8, 4.5])
        .y([3.0, 3.0, 3.6, f64::NAN, 2.0, 2.0, 2.6])
        .col(["pink", "yellow"])
        .density([None, Some(15.0)])
        .draw();
    g.text().x([2.0, 4.0]).y([2.5, 4.5]).draw();
    g.text()
        .x([3.0])
        .y([5.8])
        .labels(&[Label::math("beta[i]")])
        .cex(1.3)
        .draw();
    g.axis()
        .side(3)
        .at([1.0, 3.0, 5.0])
        .labels(["a", "b", "c"])
        .draw();
    g.abline().a(1.0).b(1.0).h([3.5]).col("darkgreen").draw();
    g.legend()
        .pos(LegendPos::Keyword(LegendKeyword::BottomRight))
        .labels(["points", "fit"])
        .col(["red", "darkgreen"])
        .pch([Some(19), None])
        .lty([None, Some(LineType::SOLID)])
        .title("Key")
        .draw();
    g.mtext().text("right margin").side(4).line(0.5).draw();
    g.title().sub("a subtitle").draw();
}

/// `hist()`, and a second histogram added to it.
fn histograms<G: Graphics>(g: &mut G) {
    let x = [1.2, 2.5, 2.7, 3.1, 3.3, 3.9, 4.4, 5.0, 5.2, 7.5];
    g.hist().x(x).col("lightgrey").draw();
    g.hist()
        .x([2.1, 3.2, 3.4, 4.1])
        .add(true)
        .col("steelblue")
        .draw();
}

/// A stacked barplot matrix with names and a legend, then a box.
fn bars<G: Graphics>(g: &mut G) {
    let m = vec![
        vec![1.0, 2.0, 3.0],
        vec![2.0, 1.0, 1.0],
        vec![3.0, 2.0, 0.5],
    ];
    g.barplot()
        .height(m)
        .names(["a", "b", "c"])
        .legend_text(["r1", "r2", "r3"])
        .density([10, 20, 30])
        .main("Bars")
        .draw();
    g.box_().draw();
}

/// A notched horizontal boxplot.
fn boxes<G: Graphics>(g: &mut G) {
    let groups = vec![
        vec![1.0, 2.0, 2.5, 3.0, 3.5, 4.0, 9.0],
        vec![2.0, 3.0, 3.5, 4.0, 4.2, 5.0],
    ];
    g.boxplot()
        .x(&groups)
        .notch(true)
        .horizontal(true)
        .names(["A", "B"])
        .draw();
}

/// `pie()`.
fn pie<G: Graphics>(g: &mut G) {
    g.pie()
        .x([1.0, 2.0, 3.0])
        .labels(["one", "two", "three"])
        .main("Pie")
        .draw();
}

/// `plot_new()`, a log window, outer `mtext()`, and `par(new = TRUE)` for
/// an overlay.
fn layered<G: Graphics>(g: &mut G) {
    g.par().oma([0.0, 0.0, 2.0, 0.0]).draw();
    g.plot_new();
    g.plot_window()
        .xlim([1.0, 1000.0])
        .ylim([0.0, 1.0])
        .xlog(true)
        .draw();
    g.points()
        .x([1.0, 10.0, 100.0, 1000.0])
        .y([0.2, 0.4, 0.6, 0.8])
        .draw();
    g.axis().side(1).draw();
    g.mtext().text("outer title").side(3).outer(true).draw();
    g.par().new(true).draw();
    g.plot()
        .x([0.0, 1.0])
        .y([1.0, 0.0])
        .type_(PlotType::Lines)
        .axes(false)
        .draw();
    g.axis().side(4).draw();
}

type Scene = fn(&mut Figure);
type Direct = fn(&mut Device<AnyBackend>);

fn scenes() -> Vec<(&'static str, Scene, Direct)> {
    vec![
        ("scatter", scatter::<Figure>, scatter::<Device<AnyBackend>>),
        (
            "histograms",
            histograms::<Figure>,
            histograms::<Device<AnyBackend>>,
        ),
        ("bars", bars::<Figure>, bars::<Device<AnyBackend>>),
        ("boxes", boxes::<Figure>, boxes::<Device<AnyBackend>>),
        ("pie", pie::<Figure>, pie::<Device<AnyBackend>>),
        ("layered", layered::<Figure>, layered::<Device<AnyBackend>>),
    ]
}

fn device(format: Format) -> Device<AnyBackend> {
    // `compress(false)`: an uncompressed PDF, a plain SVG; ignored for PNG.
    Device::memory(format)
        .width(7.0)
        .height(6.0)
        .compress(false)
        .open()
        .unwrap()
}

const FORMATS: [Format; 3] = [Format::Pdf, Format::Svg, Format::Png];

fn recorded(scene: Scene) -> Figure {
    let mut fig = Figure::new();
    scene(&mut fig);
    fig
}

#[test]
fn every_scene_replays_identically_on_every_backend() {
    for format in FORMATS {
        for (name, record, direct) in scenes() {
            let mut a = device(format);
            direct(&mut a);
            let mut b = device(format);
            b.add_figure(&recorded(record));
            assert!(a.finish() == b.finish(), "{name} differs on {format:?}");
        }
    }
}

#[test]
fn a_plot_can_be_added_many_times_and_mixed_with_direct_calls() {
    let fig = recorded(scatter::<Figure>);
    let bars_fig = recorded(bars::<Figure>);
    for format in FORMATS {
        // mfrow(2, 2): direct, recorded, recorded again, then a recorded
        // plot placed with add_figure_at - against all direct, with mfg.
        let mut a = device(format);
        a.par().mfrow([2, 2]).draw();
        scatter(&mut a);
        bars(&mut a);
        scatter(&mut a);
        a.par().mfg([1, 2]).draw();
        bars(&mut a);

        let mut b = device(format);
        b.par().mfrow([2, 2]).draw();
        scatter(&mut b);
        b.add_figure(&bars_fig);
        b.add_figure(&fig);
        b.add_figure_at(1, 2, &bars_fig);
        assert!(a.finish() == b.finish(), "mixed page differs on {format:?}");
    }
}

#[test]
fn add_figure_at_on_a_fresh_device_starts_the_page_first() {
    // R ignores par(mfg) until something is drawn, so add_figure_at opens
    // the page as plot_new() would, then picks the panel.
    let mut a = device(Format::Pdf);
    a.par().mfrow([2, 2]).draw();
    a.plot_new();
    a.par().mfg([2, 1]).draw();
    pie(&mut a);

    let mut b = device(Format::Pdf);
    b.par().mfrow([2, 2]).draw();
    b.add_figure_at(2, 1, &recorded(pie::<Figure>));
    assert!(a.finish() == b.finish());
}

#[test]
fn add_figure_restores_par() {
    let mut dev = device(Format::Pdf);
    let before = format!("{:?}", dev.current_par());
    dev.add_figure(&recorded(scatter::<Figure>)); // sets mar and las
    assert_eq!(format!("{:?}", dev.current_par()), before);

    // Drawn directly, the same calls leave par changed.
    let mut direct = device(Format::Pdf);
    scatter(&mut direct);
    assert_eq!(direct.current_par().las, 1);
}

#[test]
fn recorded_results_match_the_device() {
    let x = [1.2, 2.5, 2.7, 3.1, 3.3, 3.9, 4.4, 5.0, 5.2, 7.5];
    let m = vec![vec![1.0, 2.0], vec![3.0, 1.0]];
    let groups = vec![vec![1.0, 2.0, 3.0, 9.0], vec![2.0, 3.0]];
    let mut fig = Figure::new();
    let mut dev = device(Format::Svg);

    let (h_fig, h_dev) = (fig.hist().x(x).draw(), dev.hist().x(x).draw());
    assert_eq!(
        (h_fig.breaks, h_fig.counts, h_fig.density, h_fig.mids),
        (h_dev.breaks, h_dev.counts, h_dev.density, h_dev.mids)
    );
    assert_eq!(
        fig.barplot().height(m.clone()).beside(true).draw(),
        dev.barplot().height(m).beside(true).draw()
    );
    let (b_fig, b_dev) = (
        fig.boxplot().x(&groups).draw(),
        dev.boxplot().x(&groups).draw(),
    );
    assert_eq!(format!("{b_fig:?}"), format!("{b_dev:?}"));

    // Legend layouts come back from add_figure, one per legend() call.
    let key = LegendOptions::default().col("red").pch(1);
    let mut legends = Figure::new();
    legends.plot().x(x).y(x).draw();
    legends
        .legend()
        .pos(LegendPos::Keyword(LegendKeyword::TopLeft))
        .labels("a")
        .with(&key)
        .draw();
    legends
        .legend()
        .pos(LegendPos::Xy(5.0, 3.0))
        .labels("a")
        .with(&key)
        .cex(0.8)
        .draw();
    let mut d1 = device(Format::Pdf);
    let replayed = d1.add_figure(&legends);
    let mut d2 = device(Format::Pdf);
    d2.plot().x(x).y(x).draw();
    let direct = vec![
        d2.legend()
            .pos(LegendPos::Keyword(LegendKeyword::TopLeft))
            .labels("a")
            .with(&key)
            .draw(),
        d2.legend()
            .pos(LegendPos::Xy(5.0, 3.0))
            .labels("a")
            .with(&key)
            .cex(0.8)
            .draw(),
    ];
    assert_eq!(replayed.legends, direct);
}

#[test]
fn recording_checks_arguments_like_the_device() {
    let r = std::panic::catch_unwind(|| {
        Figure::new()
            .barplot()
            .height([1.0, 2.0])
            .names(["a", "b", "c"])
            .draw();
    });
    assert!(
        r.is_err(),
        "wrong number of names should panic when recorded"
    );
}

#[test]
fn legend_errors_panic_when_recorded() {
    let no_labels = std::panic::catch_unwind(|| {
        Figure::new()
            .legend()
            .pos("topleft")
            .labels([] as [&str; 0])
            .draw();
    });
    assert!(no_labels.is_err(), "a legend without labels should panic");
    let bad_width = std::panic::catch_unwind(|| {
        Figure::new()
            .legend()
            .pos("topleft")
            .labels("a")
            .text_width(-1)
            .draw();
    });
    assert!(bad_width.is_err(), "a negative text.width should panic");
}

/// The legends drawn with R-style positions or with `LegendPos` variants.
fn legends<G: Graphics>(g: &mut G, short: bool) {
    let x = [1.0, 10.0];
    g.plot().x(x).y(x).draw();
    if short {
        g.legend().pos("bottomr").labels("a").pch(19).draw();
        g.legend().pos((5.0, 3.0)).labels("a").pch(19).draw();
        g.legend()
            .pos(((2.0, 4.0), (6.0, 9.0)))
            .labels("a")
            .pch(19)
            .draw();
    } else {
        g.legend()
            .pos(LegendPos::Keyword(LegendKeyword::BottomRight))
            .labels("a")
            .pch(19)
            .draw();
        g.legend()
            .pos(LegendPos::Xy(5.0, 3.0))
            .labels("a")
            .pch(19)
            .draw();
        g.legend()
            .pos(LegendPos::Corners(2.0, 6.0, 4.0, 9.0))
            .labels("a")
            .pch(19)
            .draw();
    }
}

#[test]
fn legend_shorthand_draws_the_same() {
    let draw = |short: bool, record: bool| {
        let mut dev = device(Format::Pdf);
        if record {
            let mut fig = Figure::new();
            legends(&mut fig, short);
            dev.add_figure(&fig);
        } else {
            legends(&mut dev, short);
        }
        dev.finish()
    };
    let long = draw(false, false);
    assert_eq!(draw(true, false), long, "on a device");
    assert_eq!(draw(true, true), long, "in a figure");
}

/// `par` comes back after `add_figure` even when a recorded call fails
/// (here `points()` with a negative colour number, an error R finds only
/// when drawing): returned by `try_add_figure`, or a caught panic from
/// `add_figure`.
#[test]
fn add_figure_restores_par_after_an_error() {
    let mut fig = Figure::new();
    fig.par().cex(2).las(1).draw();
    fig.plot().x([1, 2]).y([1, 2]).draw();
    fig.points().x(1).y(1).col(-1).draw();
    fig.abline().h(1.5).draw();

    let mut dev = device(Format::Pdf);
    let before = format!("{:?}", dev.current_par());
    let e = dev.try_add_figure(&fig).unwrap_err();
    assert_eq!(
        e.to_string(),
        "points(): numerical color values must be >= 0, found -1"
    );
    assert_eq!(format!("{:?}", dev.current_par()), before);

    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        dev.add_figure(&fig);
    }));
    std::panic::set_hook(hook);
    assert!(caught.is_err());
    assert_eq!(format!("{:?}", dev.current_par()), before);
}

/// `barplot()` and `legend(xpd = )` change `par` only for the call, as R's
/// `on.exit(par(opar))`.
#[test]
fn barplot_and_legend_restore_their_par_changes() {
    let mut dev = device(Format::Pdf);
    dev.barplot().height([3, 5, 2]).draw();
    dev.barplot().height([3, 5, 2]).horiz(true).draw();
    let p = dev.current_par();
    assert_eq!(
        (p.xaxs, p.yaxs, p.xpd),
        (AxisStyle::Regular, AxisStyle::Regular, Xpd::Plot)
    );
    dev.legend()
        .pos("topright")
        .labels("a")
        .pch(1)
        .xpd(Xpd::Device)
        .draw();
    assert_eq!(dev.current_par().xpd, Xpd::Plot);
}

/// `barplot()` can't shade on a log axis (R's error), on a device and
/// when a `Figure` records the call.
#[test]
fn barplot_shading_on_a_log_axis_panics() {
    for figure in [false, true] {
        let r = std::panic::catch_unwind(|| {
            if figure {
                let mut fig = Figure::new();
                fig.barplot().height([1, 2]).ylog(true).density(10).draw();
            } else {
                let mut dev = Device::memory("pdf").width(5).height(5).open().unwrap();
                dev.barplot().height([1, 2]).ylog(true).density(10).draw();
            }
        });
        let msg = r.unwrap_err();
        let msg = msg
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| msg.downcast_ref::<&str>().copied())
            .unwrap_or_default();
        assert!(
            msg.contains("Cannot use shading lines in bars when log scale is used"),
            "{msg}"
        );
    }
}
