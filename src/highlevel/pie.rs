// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! `pie(x, ...)`, ported from `r-source/src/library/graphics/R/pie.R`.

use std::f64::consts::PI;

use crate::backend::Backend;
use crate::color::{Color, ColorSpec};
use crate::device::{
    Device, PlotWindowOptions, PolygonOptions, TextOptions, TitleOptions, XyOptions,
};
use crate::font::FontFace;
use crate::par::Xpd;
use crate::plotmath::Label;

/// R's default pie slice color cycle (used when `col` isn't given).
const DEFAULT_COLORS: [&str; 6] = [
    "white",
    "lightblue",
    "mistyrose",
    "lightcyan",
    "lavender",
    "cornsilk",
];

/// Options for [`Graphics::pie`](crate::graphics::Graphics::pie), covering the subset of `pie()`'s arguments this
/// crate implements. Unset fields are R's defaults, given for each.
#[derive(Clone, Debug)]
pub struct PieOptions {
    /// Slice labels (text or plotmath expressions); `None` gives `1, 2,
    /// ...`. An empty text label, or a missing one (fewer labels than
    /// slices, R's `NA`), draws no label or tick for its slice.
    pub labels: Option<Vec<Label>>,
    /// Rotation of the labels in degrees, counterclockwise (R's `srt`,
    /// passed to `text()`); `None`: `par(srt)`.
    pub srt: Option<f64>,
    /// Size of the labels (R's inline `cex`, passed to `text()`; `main`
    /// keeps `cex.main`); `None`: 1.
    pub cex: Option<f64>,
    /// Font of the labels (R's inline `font`, passed to `text()`); `None`:
    /// `par(font)`.
    pub font: Option<FontFace>,
    /// Pie radius in user units, where the plot region's shorter side
    /// spans `[-1, 1]` before the usual 4% axis extension; `None`: 0.8
    /// (R's default).
    pub radius: Option<f64>,
    /// Slice fill colours, recycled over the slices; empty: R's default
    /// cycle of 6 pastel colours.
    pub col: Vec<ColorSpec>,
    /// Slice borders; `None`: `par(fg)` (R's default).
    pub border: Option<ColorSpec>,
    /// Title, as for `plot()` (text or a plotmath expression); `None`:
    /// none. Drawn whatever `par(ann)` is, as in R.
    pub main: Option<Label>,
    /// Size of `main`, relative to `par(cex)` (R's `cex.main`, passed
    /// to `title()`); `None`: `par(cex.main)`.
    pub cex_main: Option<f64>,
    /// Colour of `main` (R's `col.main`, passed to `title()`);
    /// `None`: `par(col.main)`.
    pub col_main: Option<ColorSpec>,
    /// Font of `main` (a [`FontFace`]; R's
    /// `font.main`, passed to `title()`); `None`: `par(font.main)`.
    pub font_main: Option<FontFace>,
    /// Approximate number of polygon edges for a full circle; `None`: 200
    /// (R's default).
    pub edges: Option<usize>,
    /// Slices go clockwise; default `false` (counterclockwise).
    pub clockwise: bool,
    /// Starting angle in degrees (R's `init.angle`). `None`: R's default,
    /// 90 (12 o'clock) when `clockwise`, else 0 (3 o'clock).
    pub init_angle: Option<f64>,
}

crate::setters::setters!(PieOptions => [PieCall] {
    labels: list Label,
    srt: option f64,
    cex: option f64,
    font: option FontFace,
    radius: option f64,
    col: values ColorSpec,
    border: option ColorSpec,
    main: option Label,
    cex_main: option f64,
    col_main: option ColorSpec,
    font_main: option FontFace,
    edges: option usize,
    clockwise: value bool,
    init_angle: option f64,
});

impl Default for PieOptions {
    fn default() -> Self {
        PieOptions {
            labels: None,
            srt: None,
            cex: None,
            font: None,
            radius: None,
            col: Vec::new(),
            border: None,
            main: None,
            cex_main: None,
            col_main: None,
            font_main: None,
            edges: None,
            clockwise: false,
            init_angle: None,
        }
    }
}

impl<B: Backend> Device<B> {
    /// Draws R's `pie()`; the public entry is
    /// [`Graphics::pie`](crate::graphics::Graphics::pie), whose docs
    /// describe it.
    pub(crate) fn pie_impl(&mut self, x: &[f64], opts: &PieOptions) {
        let nx = x.len();
        assert!(
            nx > 0 && x.iter().all(|&v| v.is_finite() && v >= 0.0),
            "pie: 'x' values must be positive"
        );
        let total: f64 = x.iter().sum();

        let labels: Vec<Label> = opts
            .labels
            .clone()
            .unwrap_or_else(|| (1..=nx).map(|i| Label::from(i.to_string())).collect());

        self.plot_new_impl();

        // Pre-scale xlim/ylim by the plot region's aspect ratio, then
        // `plot.window(xlim, ylim, asp = 1)` (with the current `xaxs`/`yaxs`,
        // i.e. R's default 4% extension), exactly as `pie.R` does.
        let plt = self.plot_region();
        let (w, h) = (plt.width(), plt.height());
        let (xlim, ylim) = if w > h {
            ((-(w / h), w / h), (-1.0, 1.0))
        } else {
            ((-1.0, 1.0), (-(h / w), h / w))
        };
        let window = PlotWindowOptions {
            asp: Some(1.0),
            ..PlotWindowOptions::default()
        };
        self.plot_window_impl(xlim, ylim, &window);

        let twopi = if opts.clockwise { -2.0 * PI } else { 2.0 * PI };
        let init = opts
            .init_angle
            .unwrap_or(if opts.clockwise { 90.0 } else { 0.0 })
            .to_radians();
        let radius = opts.radius.unwrap_or(0.8);
        let edges = opts.edges.unwrap_or(200);
        let t2xy = |t: f64| -> (f64, f64) {
            let tp = twopi * t + init;
            (radius * tp.cos(), radius * tp.sin())
        };

        let mut cum = Vec::with_capacity(nx + 1);
        cum.push(0.0);
        // `c(0, cumsum(x) / sum(x))`: divide the running sum (not each term),
        // so slice fractions - and hence edge counts - round exactly as in R.
        let mut acc = 0.0;
        for &v in x {
            acc += v;
            cum.push(acc / total);
        }

        for i in 0..nx {
            let dxi = cum[i + 1] - cum[i];
            let n = (2usize).max((edges as f64 * dxi).floor() as usize);
            let mut xs = Vec::with_capacity(n + 1);
            let mut ys = Vec::with_capacity(n + 1);
            for k in 0..n {
                let t = if n > 1 {
                    cum[i] + dxi * (k as f64) / (n as f64 - 1.0)
                } else {
                    cum[i]
                };
                let (px, py) = t2xy(t);
                xs.push(px);
                ys.push(py);
            }
            xs.push(0.0);
            ys.push(0.0);

            let col = if opts.col.is_empty() {
                ColorSpec::Color(Color::parse(DEFAULT_COLORS[i % DEFAULT_COLORS.len()]).unwrap())
            } else {
                opts.col[i % opts.col.len()]
            };
            // `border = NULL`: polygon()'s default, par(fg).
            let slice = PolygonOptions {
                col: vec![col],
                border: opts.border.into_iter().collect(),
                ..PolygonOptions::default()
            };
            self.polygon_impl(&xs, &ys, &slice);

            let mid = 0.5 * (cum[i] + cum[i + 1]);
            let (px, py) = t2xy(mid);
            // `labels[i]` is NA past the end of `labels`: no label.
            let Some(label) = labels.get(i) else {
                continue;
            };
            if !matches!(label, Label::Text(t) if t.is_empty()) {
                self.lines_impl(&[px, 1.05 * px], &[py, 1.05 * py], &XyOptions::default());
                // `text(1.1 * P$x, 1.1 * P$y, labels[i], xpd = TRUE, adj =
                // ifelse(P$x < 0, 1, 0))`.
                let at = TextOptions {
                    adj: Some(if px < 0.0 { 1.0 } else { 0.0 }),
                    xpd: Some(Xpd::Figure),
                    srt: opts.srt,
                    cex: opts.cex.into_iter().map(Some).collect(),
                    font: opts.font.into_iter().map(Some).collect(),
                    ..TextOptions::default()
                };
                self.text_impl(&[1.1 * px], &[1.1 * py], std::slice::from_ref(label), &at);
            }
        }

        if let Some(main) = &opts.main {
            self.title_impl(&TitleOptions {
                main: Some(main.clone()),
                cex_main: opts.cex_main,
                col_main: opts.col_main,
                font_main: opts.font_main,
                ..TitleOptions::default()
            });
        }
    }
}
