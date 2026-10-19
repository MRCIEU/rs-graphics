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
//! return. (Not an R cross-check: the direct calls are checked against R
//! elsewhere.)

use rs_graphics::prelude::*;

// ---- scenes, written once against the trait ----

/// `par()`, `plot()` and most low-level calls, with plotmath and default
/// text labels.
fn scatter<G: Graphics>(g: &mut G) {
    g.par().mar([4.0, 4.0, 3.0, 2.0]).las(1).draw();
    let x = [1.0, 2.0, 3.0, 4.0, 5.0];
    let y = [2.0, 4.5, 3.0, 6.0, 5.5];
    g.plot(&x, &y)
        .pch(19)
        .col(["red", "blue"])
        .main(Label::math("y == alpha^2"))
        .xlab("x")
        .draw();
    g.lines(&x, &y).lty(LineType::DASHED).draw();
    g.points(&[2.5], &[5.0]).pch(4).cex(2.0).draw();
    g.segments(&[1.0], &[2.0], &[5.0], &[6.0])
        .col("grey")
        .draw();
    g.arrows(&[1.5, 4.5], &[5.5, 5.5], &[1.5, 4.5], &[3.0, 3.0])
        .angle(90.0)
        .code(3)
        .length(0.05)
        .draw();
    g.rect(&[3.2], &[2.2], &[3.8], &[2.8])
        .col("lightblue")
        .draw();
    g.polygon(
        &[1.2, 1.8, 1.5, f64::NAN, 4.2, 4.8, 4.5],
        &[3.0, 3.0, 3.6, f64::NAN, 2.0, 2.0, 2.6],
    )
    .col(["pink", "yellow"])
    .draw();
    g.text(&[2.0, 4.0], &[2.5, 4.5], &[] as &[&str]).draw();
    g.text(&[3.0], &[5.8], &[Label::math("beta[i]")])
        .cex(1.3)
        .draw();
    g.axis(3).at([1.0, 3.0, 5.0]).labels(["a", "b", "c"]).draw();
    g.abline().coef(1.0, 1.0).h([3.5]).col("darkgreen").draw();
    let entries = [
        LegendEntry::point("points", "red", 19),
        LegendEntry::line("fit", "darkgreen", 1.0, LineType::SOLID),
    ];
    g.legend(LegendPos::Keyword(LegendKeyword::BottomRight), &entries)
        .title("Key")
        .draw();
    g.mtext("right margin", 4).line(0.5).draw();
    g.title().sub("a subtitle").draw();
}

/// `hist()`, and a second histogram added to it.
fn histograms<G: Graphics>(g: &mut G) {
    let x = [1.2, 2.5, 2.7, 3.1, 3.3, 3.9, 4.4, 5.0, 5.2, 7.5];
    g.hist(&x).col("lightgrey").draw();
    g.hist(&[2.1, 3.2, 3.4, 4.1])
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
    g.barplot(m)
        .names(["a", "b", "c"])
        .legend_text(["r1", "r2", "r3"])
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
    g.boxplot(&groups)
        .notch(true)
        .horizontal(true)
        .names(["A", "B"])
        .draw();
}

/// `pie()`.
fn pie<G: Graphics>(g: &mut G) {
    g.pie(&[1.0, 2.0, 3.0])
        .labels(["one", "two", "three"])
        .main("Pie")
        .draw();
}

/// `plot_new()`, a log window, outer `mtext()`, and `par(new = TRUE)` for
/// an overlay.
fn layered<G: Graphics>(g: &mut G) {
    g.par().oma([0.0, 0.0, 2.0, 0.0]).draw();
    g.plot_new();
    g.plot_window((1.0, 1000.0), (0.0, 1.0)).xlog(true).draw();
    g.points(&[1.0, 10.0, 100.0, 1000.0], &[0.2, 0.4, 0.6, 0.8])
        .draw();
    g.axis(1).draw();
    g.mtext("outer title", 3).outer(true).draw();
    g.par().new(true).draw();
    g.plot(&[0.0, 1.0], &[1.0, 0.0])
        .type_(PlotType::Lines)
        .axes(false)
        .draw();
    g.axis(4).draw();
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
    let backend = match format {
        Format::Pdf => AnyBackend::Pdf(PdfBackend::new(7.0, 6.0).with_compression(false)),
        Format::Svg => AnyBackend::Svg(SvgBackend::new(7.0, 6.0)),
        Format::Png => AnyBackend::Png(RasterBackend::new(7.0, 6.0, 72.0, Color::WHITE)),
    };
    Device::new(backend, Par::default())
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
        a.mfrow(2, 2);
        scatter(&mut a);
        bars(&mut a);
        scatter(&mut a);
        a.mfg(1, 2);
        bars(&mut a);

        let mut b = device(format);
        b.mfrow(2, 2);
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
    a.mfrow(2, 2);
    a.plot_new();
    a.mfg(2, 1);
    pie(&mut a);

    let mut b = device(Format::Pdf);
    b.mfrow(2, 2);
    b.add_figure_at(2, 1, &recorded(pie::<Figure>));
    assert!(a.finish() == b.finish());
}

#[test]
fn add_figure_restores_par() {
    let mut dev = device(Format::Pdf);
    let before = format!("{:?}", dev.par);
    dev.add_figure(&recorded(scatter::<Figure>)); // sets mar and las
    assert_eq!(format!("{:?}", dev.par), before);

    // Drawn directly, the same calls leave par changed.
    let mut direct = device(Format::Pdf);
    scatter(&mut direct);
    assert_eq!(direct.par.las, 1);
}

#[test]
fn recorded_results_match_the_device() {
    let x = [1.2, 2.5, 2.7, 3.1, 3.3, 3.9, 4.4, 5.0, 5.2, 7.5];
    let m = vec![vec![1.0, 2.0], vec![3.0, 1.0]];
    let groups = vec![vec![1.0, 2.0, 3.0, 9.0], vec![2.0, 3.0]];
    let mut fig = Figure::new();
    let mut dev = device(Format::Svg);

    let (h_fig, h_dev) = (fig.hist(&x).draw(), dev.hist(&x).draw());
    assert_eq!(
        (h_fig.breaks, h_fig.counts, h_fig.density, h_fig.mids),
        (h_dev.breaks, h_dev.counts, h_dev.density, h_dev.mids)
    );
    assert_eq!(
        fig.barplot(m.clone()).beside(true).draw(),
        dev.barplot(m).beside(true).draw()
    );
    let (b_fig, b_dev) = (fig.boxplot(&groups).draw(), dev.boxplot(&groups).draw());
    assert_eq!(format!("{b_fig:?}"), format!("{b_dev:?}"));

    // Legend layouts come back from add_figure, one per legend() call.
    let entries = [LegendEntry::point("a", "red", 1)];
    let mut legends = Figure::new();
    legends.plot(&x, &x).draw();
    legends
        .legend(LegendPos::Keyword(LegendKeyword::TopLeft), &entries)
        .draw();
    legends
        .legend(LegendPos::Xy(5.0, 3.0), &entries)
        .cex(0.8)
        .draw();
    let mut d1 = device(Format::Pdf);
    let replayed = d1.add_figure(&legends);
    let mut d2 = device(Format::Pdf);
    d2.plot(&x, &x).draw();
    let direct = vec![
        d2.legend(LegendPos::Keyword(LegendKeyword::TopLeft), &entries)
            .draw(),
        d2.legend(LegendPos::Xy(5.0, 3.0), &entries).cex(0.8).draw(),
    ];
    assert_eq!(replayed.legends, direct);
}

#[test]
fn recording_checks_arguments_like_the_device() {
    let r = std::panic::catch_unwind(|| {
        Figure::new()
            .barplot([1.0, 2.0])
            .names(["a", "b", "c"])
            .draw();
    });
    assert!(
        r.is_err(),
        "wrong number of names should panic when recorded"
    );
}
