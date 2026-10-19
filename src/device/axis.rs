// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! `axis()`: a port of `C_axis` (`plot.c`, incl. `GetAxisLimits`,
//! `getxlimits`/`getylimits` and `ComputePAdjValue`), with the labels placed
//! by `GMtext()` (`graphics.c`).
//!
//! The axis line sits `line` margin lines out from the plot region (or the
//! inner region, for `outer`), or at the user coordinate `pos`. Labels sit
//! `mgp[1]` lines beyond the axis line; when neither `line` nor `pos` is
//! given, the axis line sits at `mgp[2]` and the labels at `mgp[1]`, both
//! measured from the plot region. Lines and labels are clipped to the
//! device, as R forces `xpd = NA`. Expression labels ([`Label::Math`]) are
//! drawn by `GMMathText()`'s rules and, as in R, never dropped for
//! overlapping (`gap.axis` applies to text labels only).

use crate::afm;
use crate::backend::Backend;
use crate::color::ColorSpec;
use crate::geometry::Point;
use crate::lty::LineType;
use crate::par::AxisType;
use crate::plotmath::Label;

use super::{Clip, Device, Side, Y_LINE_BIAS, format_axis_labels};

/// `FLT_EPSILON`, which `GetAxisLimits()` widens the axis range by.
const FLT_EPSILON: f64 = f32::EPSILON as f64;

/// The arguments of R's `axis()` other than `side`, plus the inline
/// graphical parameters it uses, with R's defaults. `None` is R's `NA` (or
/// `NULL`), which falls back to the matching `par()` value unless the
/// field says otherwise.
#[derive(Clone, Debug)]
pub struct AxisOptions {
    /// Tick positions, in user coordinates (data units, also on a log
    /// axis).
    /// `None`: the default ticks from `axp` (else `par(xaxp/yaxp)`), as
    /// [`Device::default_ticks`].
    pub at: Option<Vec<f64>>,
    /// Labels for `at` (requires `at`): text or plotmath expressions
    /// (`Some("a".into())`, `Some(Label::math("10^5"))`). `None`: `at`
    /// formatted like R's `formatReal()`. An inner `None` is a character
    /// `NA`: its tick is drawn but not its label, which also doesn't count
    /// for `gap_axis`.
    pub labels: Option<Vec<Option<Label>>>,
    /// `labels = FALSE` when false: no labels at all. Default `true`.
    pub draw_labels: bool,
    /// `tick`: draw the axis line and tick marks. Default `true`.
    pub tick: bool,
    /// Margin lines from the plot region (inner region for `outer`) to the
    /// axis line. Labels then sit `mgp[1]` lines beyond it. `None` (R's
    /// `NA`): the axis line at `mgp[2]` and the labels at `mgp[1]` lines.
    pub line: Option<f64>,
    /// User coordinate (on the other axis) at which to draw the axis line;
    /// overrides `line`. Labels sit `mgp[1]` lines beyond it. `None` (R's
    /// `NA`): use `line`.
    pub pos: Option<f64>,
    /// Measure `line` from the inner region (inside `par(oma)`) rather
    /// than the plot region. Default `false`.
    pub outer: bool,
    /// Label font; `None`: `par(font.axis)`.
    pub font: Option<i32>,
    /// Line type of the axis line and ticks; `None`: solid (R's default,
    /// not `par(lty)`).
    pub lty: Option<LineType>,
    /// Width of the axis line; `None`: 1 (R's default, not `par(lwd)`). 0
    /// or less draws no axis line.
    pub lwd: Option<f64>,
    /// Width of the tick marks; `None`: `lwd`. 0 or less draws no ticks.
    pub lwd_ticks: Option<f64>,
    /// Colour of the axis line; `None`: `par(fg)`.
    pub col: Option<ColorSpec>,
    /// Colour of the ticks; `None`: `col`.
    pub col_ticks: Option<ColorSpec>,
    /// Justification of every label along its reading direction; `None`:
    /// centred for labels parallel to the axis, otherwise right-justified
    /// on sides 1 and 2 and left-justified on 3 and 4.
    pub hadj: Option<f64>,
    /// Justification across each label, recycled over the ticks in
    /// increasing order of `at`; `NaN` entries and an empty vector are R's
    /// `NA`, which depends on `side` and `las` (0, i.e. baseline on the
    /// line, for `las = 0`).
    pub padj: Vec<f64>,
    /// Minimum gap between labels, in "m" widths (labels parallel to the
    /// axis) or "M" heights (perpendicular); a label closer than that to
    /// the previous one is dropped. `None`: 1 or 0.25.
    pub gap_axis: Option<f64>,
    /// Label size (R's inline `cex.axis`); `None`: `par(cex.axis)`.
    pub cex_axis: Option<f64>,
    /// Label colour (R's inline `col.axis`); `None`: `par(col.axis)`.
    pub col_axis: Option<ColorSpec>,
    /// Label orientation (R's inline `las`); `None`: `par(las)`.
    pub las: Option<i32>,
    /// Range and number of the default ticks when `at` is `None` (R's
    /// inline `xaxp`/`yaxp`); `None`: `par(xaxp)` (sides 1, 3) or
    /// `par(yaxp)` (sides 2, 4).
    pub axp: Option<[f64; 3]>,
    /// `AxisType::None` draws nothing (R's inline `xaxt`/`yaxt`); `None`:
    /// `par(xaxt)` (sides 1, 3) or `par(yaxt)` (sides 2, 4).
    pub axt: Option<AxisType>,
    /// Tick length as a fraction of the plot region (of its height for an
    /// x axis if above 0.5, else of its smaller side); positive points
    /// inwards. Overrides `tcl`. `None` (R's `par(tck)` default `NA`): use
    /// `tcl`.
    pub tck: Option<f64>,
    /// Tick length in margin lines; negative points outwards. `None`:
    /// `par(tcl)`.
    pub tcl: Option<f64>,
}

crate::setters::setters!(AxisOptions => [AxisCall] {
    at: option_values f64,
    labels: labels Label,
    draw_labels: value bool,
    tick: value bool,
    line: option f64,
    pos: option f64,
    outer: value bool,
    font: option i32,
    lty: option LineType,
    lwd: option f64,
    lwd_ticks: option f64,
    col: option ColorSpec,
    col_ticks: option ColorSpec,
    hadj: option f64,
    padj: values f64,
    gap_axis: option f64,
    cex_axis: option f64,
    col_axis: option ColorSpec,
    las: option i32,
    axp: option [f64; 3],
    axt: option AxisType,
    tck: option f64,
    tcl: option f64,
});

impl Default for AxisOptions {
    fn default() -> Self {
        AxisOptions {
            at: None,
            labels: None,
            draw_labels: true,
            tick: true,
            line: None,
            pos: None,
            outer: false,
            font: None,
            lty: None,
            lwd: None,
            lwd_ticks: None,
            col: None,
            col_ticks: None,
            hadj: None,
            padj: Vec::new(),
            gap_axis: None,
            cex_axis: None,
            col_axis: None,
            las: None,
            axp: None,
            axt: None,
            tck: None,
            tcl: None,
        }
    }
}

/// `ComputePAdjValue()`: the default `padj` for `side` and `las`.
fn default_padj(side: Side, las: i32) -> f64 {
    let x_axis = side.is_horizontal();
    match las {
        1 if !x_axis => 0.5,
        2 => 0.5,
        3 if x_axis => 0.5,
        _ => 0.0,
    }
}

impl<B: Backend> Device<B> {
    /// Draws R's `axis()`; the public entry is
    /// [`Graphics::axis`](crate::graphics::Graphics::axis), whose docs
    /// describe it.
    pub(crate) fn axis_impl(&mut self, side: impl Into<Side>, opts: &AxisOptions) {
        let side = side.into();
        let x_axis = side.is_horizontal();
        let las = opts.las.unwrap_or(self.par.las);
        let perpendicular = if x_axis {
            las == 2 || las == 3
        } else {
            las == 1 || las == 2
        };

        // `line` = NA: the axis line sits at `mgp[2]`, and the labels'
        // `mgp[1]` is not taken relative to it.
        let (line, mut lineoff) = match opts.line {
            Some(l) if l.is_finite() => (l, 0.0),
            _ => (self.par.mgp[2], self.par.mgp[2]),
        };
        let pos = opts.pos.filter(|p| p.is_finite());
        if pos.is_some() {
            lineoff = 0.0;
        }

        let at = opts.at.clone().unwrap_or_else(|| match opts.axp {
            Some(axp) => self.ticks_from_axp(side, axp),
            None => self.default_ticks(side),
        });
        let labels = if opts.draw_labels {
            let labels = match &opts.labels {
                None => format_axis_labels(&at)
                    .into_iter()
                    .map(|l| Some(Label::Text(l)))
                    .collect(),
                Some(l) => {
                    assert!(opts.at.is_some(), "axis: 'labels' is supplied and not 'at'");
                    l.clone()
                }
            };
            assert_eq!(
                at.len(),
                labels.len(),
                "axis: 'at' and 'labels' lengths differ"
            );
            labels
        } else {
            Vec::new()
        };
        // `xaxt`/`yaxt = "n"`: nothing drawn (checked after the argument
        // errors above, as in R).
        let axt = if x_axis { self.par.xaxt } else { self.par.yaxt };
        if opts.axt.unwrap_or(axt) == AxisType::None {
            return;
        }
        // Sort by `at`, dropping non-finite positions (which sort last).
        let mut ticks: Vec<(f64, Option<&Label>)> = at
            .iter()
            .enumerate()
            .map(|(i, &v)| (v, labels.get(i).and_then(Option::as_ref)))
            .collect();
        ticks.sort_by(|a, b| a.0.total_cmp(&b.0));
        ticks.retain(|t| t.0.is_finite());
        if ticks.is_empty() {
            return;
        }
        // From here on, positions are in `usr` units (log10 on a log axis;
        // values <= 0 become -inf or NaN, outside every limit below).
        if (x_axis && self.xlog) || (!x_axis && self.ylog) {
            for t in &mut ticks {
                t.0 = t.0.log10();
            }
        }

        // `getxlimits`/`getylimits` (from the caller's `xpd`, before
        // `C_axis` forces it to NA) widened by `GetAxisLimits`, in `usr`
        // units: ticks outside are skipped.
        let (u0, u1) = if x_axis {
            (self.usr.0, self.usr.1)
        } else {
            (self.usr.2, self.usr.3)
        };
        let (left, right) = self.xpd_limits(x_axis);
        let (left, right) = (left.min(right), left.max(right));
        let eps = if right == left {
            0.5 * FLT_EPSILON
        } else {
            (right - left) * FLT_EPSILON
        };
        let (low, high) = (left - eps, right + eps);

        // Device position (inches) along the axis of a `usr` value.
        let (d0, d1) = if x_axis {
            (self.plt.x0, self.plt.x1)
        } else {
            (self.plt.y0, self.plt.y1)
        };
        let along = move |v: f64| d0 + (v - u0) / (u1 - u0) * (d1 - d0);
        let point = |perp: f64, along: f64| {
            if x_axis {
                Point::new(along, perp)
            } else {
                Point::new(perp, along)
            }
        };

        // The axis line's position across the axis, and which way is
        // "outward" (away from the plot region).
        let lh = self.line_height_inches();
        let outward = match side {
            Side::Bottom | Side::Left => -1.0,
            Side::Top | Side::Right => 1.0,
        };
        let axis_base = match pos {
            Some(p) => {
                if x_axis {
                    self.user_to_inches(1.0, p).y
                } else {
                    self.user_to_inches(p, 1.0).x
                }
            }
            None => {
                let region = if opts.outer { self.inner } else { self.plt };
                let edge = match side {
                    Side::Bottom => region.y0,
                    Side::Left => region.x0,
                    Side::Top => region.y1,
                    Side::Right => region.x1,
                };
                edge + outward * line * lh
            }
        };

        if opts.tick {
            let col = opts.col.unwrap_or(ColorSpec::Color(self.par.fg));
            let col_ticks = opts.col_ticks.unwrap_or(col);
            let lwd = opts.lwd.unwrap_or(1.0);
            let lty = opts.lty.unwrap_or(LineType::SOLID);
            let lwd_ticks = opts.lwd_ticks.unwrap_or(lwd);
            // Signed tick length in inches, positive inwards.
            let len = match opts.tck.filter(|t| t.is_finite()) {
                Some(tck) => {
                    let (w, h) = (self.plt.width(), self.plt.height());
                    if tck > 0.5 {
                        tck * if x_axis { h } else { w }
                    } else {
                        let (xu, yu) = (tck * w, tck * h);
                        if xu.abs() < yu.abs() { xu } else { yu }
                    }
                }
                None => opts.tcl.unwrap_or(self.par.tcl) * lh,
            };
            let axis_tick = axis_base - outward * len;
            self.push_clip_region(Clip::Device);
            if lwd > 0.0 {
                if let Some(c) = self.resolve(col) {
                    let stroke = self.stroke_of(c, lwd, lty);
                    let clamp = |v: f64| v.max(low).min(high);
                    let (a, b) = (
                        along(clamp(ticks[0].0)),
                        along(clamp(ticks[ticks.len() - 1].0)),
                    );
                    self.backend
                        .draw_line(point(axis_base, a), point(axis_base, b), &stroke);
                }
            }
            if lwd_ticks > 0.0 {
                if let Some(c) = self.resolve(col_ticks) {
                    let stroke = self.stroke_of(c, lwd_ticks, lty);
                    for &(v, _) in &ticks {
                        if low <= v && v <= high {
                            let a = along(v);
                            self.backend.draw_line(
                                point(axis_base, a),
                                point(axis_tick, a),
                                &stroke,
                            );
                        }
                    }
                }
            }
            self.pop_clip();
        }

        if !opts.draw_labels {
            return;
        }
        let font = opts.font.unwrap_or(self.par.font_axis);
        let cex = opts.cex_axis.unwrap_or(self.par.cex_axis);
        let col_axis = opts.col_axis.unwrap_or(ColorSpec::Color(self.par.col_axis));
        let face = self.par.font_face(font);
        let size_pt = self.font_size_pt(cex);
        let m_h = afm::m_height_inches(face, size_pt, 1.0);
        let gap = opts
            .gap_axis
            .unwrap_or(if perpendicular { 0.25 } else { 1.0 })
            * if perpendicular {
                m_h
            } else {
                afm::str_width_inches("m", face, size_pt, 1.0)
            };
        let hadj = opts
            .hadj
            .filter(|h| h.is_finite())
            .unwrap_or(if perpendicular {
                if outward < 0.0 { 1.0 } else { 0.0 }
            } else {
                0.5
            });
        // `GMtext()`'s rotation and `yLineBias` nudge.
        let (angle, bias) = match (side, perpendicular) {
            (Side::Bottom | Side::Top, true) => (90.0, 0.0),
            (Side::Left | Side::Right, true) => (0.0, 0.0),
            (Side::Bottom, false) => (0.0, 1.0 - Y_LINE_BIAS),
            (Side::Left, false) => (90.0, Y_LINE_BIAS),
            (Side::Top, false) => (0.0, Y_LINE_BIAS),
            (Side::Right, false) => (90.0, 1.0 - Y_LINE_BIAS),
        };
        let label_perp = axis_base + outward * (self.par.mgp[1] - lineoff + bias) * lh;

        // Labels left to right (bottom to top), skipping those within `gap`
        // of the previous one.
        let mut order: Vec<usize> = (0..ticks.len()).collect();
        if u0 > u1 {
            order.reverse();
        }
        let mut tlast = f64::NEG_INFINITY;
        for i in order {
            let (v, Some(label)) = ticks[i] else {
                continue;
            };
            if !(low < v && v < high) {
                continue;
            }
            let padj = match opts.padj.len() {
                0 => f64::NAN,
                n => opts.padj[i % n],
            };
            let padj = if padj.is_finite() {
                padj
            } else {
                default_padj(side, las)
            };
            let temp = along(v);
            let label = match label {
                Label::Text(t) => t.as_str(),
                Label::Math(expr) => {
                    // `GMMathText()`: aligned by `par(adj)` (= `hadj`) and
                    // `padj`, in the plain font at `cex.axis`.
                    if let Some(col) = self.resolve(col_axis) {
                        let cex = self.par.cex * cex;
                        let anchor = point(label_perp, temp);
                        self.math_text(anchor, expr, hadj, padj, angle, col, cex, Clip::Device);
                    }
                    continue;
                }
            };
            let labw = if perpendicular {
                m_h
            } else {
                afm::str_width_inches(label, face, size_pt, 1.0)
            };
            let tnew = temp - 0.5 * labw;
            if tnew - tlast >= gap {
                self.text_at_clipped(
                    Clip::Device,
                    point(label_perp, temp),
                    label,
                    hadj,
                    Some(padj),
                    angle,
                    col_axis,
                    cex,
                    font,
                );
                tlast = temp + 0.5 * labw;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_padj_follows_compute_padj_value() {
        for side in Side::ALL {
            assert_eq!(default_padj(side, 0), 0.0);
            assert_eq!(default_padj(side, 2), 0.5);
        }
        assert_eq!(default_padj(Side::Bottom, 1), 0.0);
        assert_eq!(default_padj(Side::Left, 1), 0.5);
        assert_eq!(default_padj(Side::Top, 3), 0.5);
        assert_eq!(default_padj(Side::Right, 3), 0.0);
    }
}
