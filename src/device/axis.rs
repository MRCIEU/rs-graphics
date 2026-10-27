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
use crate::backend::{Backend, LineEnd};
use crate::color::ColorSpec;
use crate::error::Error;
use crate::font::FontFace;
use crate::geometry::Point;
use crate::lty::LineType;
use crate::par::AxisType;
use crate::plotmath::Label;

use super::{Device, Side, Xpd, format_axis_labels};

/// `FLT_EPSILON`, which `GetAxisLimits()` widens the axis range by.
const FLT_EPSILON: f64 = f32::EPSILON as f64;

crate::setters::options! {
    /// The arguments of R's `axis()` other than `side`, plus the inline
    /// graphical parameters it uses, with R's defaults. `None` is R's `NA` (or
    /// `NULL`), which falls back to the matching `par()` value unless the
    /// field says otherwise.
    #[derive(Clone, Debug)]
    pub struct AxisOptions => [AxisCall] {
        /// Tick positions, in user coordinates (data units, also on a log
        /// axis). `None`: the default ticks from `axp` (else
        /// `par(xaxp/yaxp)`), as [`Device::ax_ticks`].
        pub at: Option<Vec<f64>> => option_values f64,
        /// Labels for the ticks in `at` (requires `at`): text or plotmath
        /// expressions (`Some("a".into())`, `Some(Label::math("10^5"))`).
        /// `None`: `at` formatted like R's `formatReal()`. An inner `None` (a
        /// character `NA`) draws the tick but not its label, and doesn't count
        /// for `gap_axis`.
        pub labels: Option<Vec<Option<Label>>> => labels Label,
        /// Whether to draw the tick labels. Default `true`. `false` is R's
        /// `labels = FALSE`.
        pub draw_labels: bool => value bool,
        /// Whether to draw the axis line and tick marks. Default `true`.
        pub tick: bool => value bool,
        /// Distance of the axis line from the plot region (inner region for
        /// `outer`), in margin lines; the labels sit `mgp[1]` lines beyond it.
        /// `None`: the axis line at `mgp[2]` and the labels at `mgp[1]` lines.
        pub line: Option<f64> => option f64,
        /// Where to draw the axis line, as a user coordinate on the other
        /// axis; overrides `line`. The labels sit `mgp[1]` lines beyond it.
        /// `None`: use `line`.
        pub pos: Option<f64> => option f64,
        /// Whether to measure `line` from the inner region (inside
        /// `par(oma)`) rather than the plot region. Default `false`.
        pub outer: bool => value bool,
        /// Font of the labels. `None`: `par(font.axis)`.
        pub font: Option<FontFace> => option FontFace,
        /// Line type of the axis line and ticks. `None`: solid (R's default,
        /// not `par(lty)`).
        pub lty: Option<LineType> => option LineType,
        /// Width of the axis line; 0 or less draws no axis line. `None`: 1
        /// (R's default, not `par(lwd)`).
        pub lwd: Option<f64> => option f64,
        /// Width of the tick marks; 0 or less draws no ticks. `None`: `lwd`.
        /// R's `lwd.ticks`.
        pub lwd_ticks: Option<f64> => option f64,
        /// Colour of the axis line. `None`: `par(fg)`.
        pub col: Option<ColorSpec> => option ColorSpec,
        /// Colour of the tick marks. `None`: `col`. R's `col.ticks`.
        pub col_ticks: Option<ColorSpec> => option ColorSpec,
        /// Justification of every label along its reading direction. `None`:
        /// centred for labels parallel to the axis, otherwise right-justified
        /// on sides 1 and 2 and left-justified on 3 and 4.
        pub hadj: Option<f64> => option f64,
        /// Justification across each label, recycled over the ticks in
        /// increasing order of `at`. `NaN` entries and an empty vector mean
        /// the default, which depends on `side` and `las` (0, i.e. baseline
        /// on the line, for `las = 0`).
        pub padj: Vec<f64> => values f64,
        /// Minimum gap between labels, in "m" widths (labels parallel to the
        /// axis) or "M" heights (perpendicular); a label closer than that to
        /// the previous one is dropped. `None`: 1 or 0.25. R's `gap.axis`.
        pub gap_axis: Option<f64> => option f64,
        /// Size of the labels. `None`: `par(cex.axis)`; overrides it for this
        /// call only.
        pub cex_axis: Option<f64> => option f64,
        /// Colour of the labels. `None`: `par(col.axis)`; overrides it for
        /// this call only.
        pub col_axis: Option<ColorSpec> => option ColorSpec,
        /// Orientation of the labels. `None`: `par(las)`; overrides it for
        /// this call only.
        pub las: Option<i32> => option i32,
        /// Range and number of the default ticks, used when `at` is `None`.
        /// `None`: `par(xaxp)` (sides 1, 3) or `par(yaxp)` (sides 2, 4);
        /// overrides it for this call only.
        pub axp: Option<[f64; 3]> => array [f64; 3],
        /// Margin lines for the axis title, labels and line (only the labels
        /// and line are used here). `None`: `par(mgp)`; overrides it for this
        /// call only.
        pub mgp: Option<[f64; 3]> => array [f64; 3],
        /// Whether to draw the axis at all: `AxisType::None` draws nothing.
        /// `None`: `par(xaxt)` (sides 1, 3) or `par(yaxt)` (sides 2, 4).
        pub axt: Option<AxisType> => option AxisType,
        /// Tick length as a fraction of the plot region (of its height for an
        /// x axis if above 0.5, else of its smaller side); positive points
        /// inwards. Overrides `tcl`. `None`: `par(tck)`, which is `NA` by
        /// default, meaning `tcl` is used.
        pub tck: Option<f64> => option f64,
        /// Tick length in margin lines; negative points outwards. `None`:
        /// `par(tcl)`.
        pub tcl: Option<f64> => option f64,
        /// How far ticks and labels may reach along the axis: the plot
        /// region, figure region or device, as for `par(xpd)`. The axis is
        /// always clipped to the device, as R does. `None`: `par(xpd)`.
        pub xpd: Option<Xpd> => option Xpd,
        /// Ends of the axis line and ticks (`"round"`, `"butt"`,
        /// `"square"`). `None`: `par(lend)`.
        pub lend: Option<LineEnd> => option LineEnd,
    }
}

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
            mgp: None,
            axt: None,
            tck: None,
            tcl: None,
            xpd: None,
            lend: None,
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
    pub(crate) fn axis_impl(
        &mut self,
        side: impl Into<Side>,
        opts: &AxisOptions,
    ) -> Result<Vec<f64>, Error> {
        check_axis(opts)?;
        let side = side.into();
        if opts.at.is_none() {
            self.check_axis_axp(side, opts.axp)?;
        }
        let mut dev = self.line_pars(opts.lend, None, None)?;
        Ok(dev.axis_draw(side, opts))
    }

    fn axis_draw(&mut self, side: Side, opts: &AxisOptions) -> Vec<f64> {
        let x_axis = side.is_horizontal();
        let las = opts.las.unwrap_or(self.par.las);
        let mgp = opts.mgp.unwrap_or(self.par.mgp);
        let perpendicular = if x_axis {
            las == 2 || las == 3
        } else {
            las == 1 || las == 2
        };

        // `line` = NA: the axis line sits at `mgp[2]`, and the labels'
        // `mgp[1]` is not taken relative to it.
        let (line, mut lineoff) = match opts.line {
            Some(l) if l.is_finite() => (l, 0.0),
            _ => (mgp[2], mgp[2]),
        };
        let pos = opts.pos.filter(|p| p.is_finite());
        if pos.is_some() {
            lineoff = 0.0;
        }

        let at = opts.at.clone().unwrap_or_else(|| match opts.axp {
            Some(axp) => self.ticks_from_axp(side, axp),
            None => self.ax_ticks(side),
        });
        let labels = if opts.draw_labels {
            match &opts.labels {
                None => format_axis_labels(&at)
                    .into_iter()
                    .map(|l| Some(Label::Text(l)))
                    .collect(),
                // `check_axis` has made sure `at` is given, and as long.
                Some(l) => l.clone(),
            }
        } else {
            Vec::new()
        };
        // `xaxt`/`yaxt = "n"`: nothing drawn (checked after the argument
        // errors above, as in R).
        let axt = if x_axis { self.par.xaxt } else { self.par.yaxt };
        if opts.axt.unwrap_or(axt) == AxisType::None {
            return Vec::new();
        }
        // R's return value: `at` sorted, `NA`s last.
        let mut used = at.clone();
        used.sort_by(|a, b| a.is_nan().cmp(&b.is_nan()).then(a.total_cmp(b)));
        // Sort by `at`, dropping non-finite positions (which sort last).
        let mut ticks: Vec<(f64, Option<&Label>)> = at
            .iter()
            .enumerate()
            .map(|(i, &v)| (v, labels.get(i).and_then(Option::as_ref)))
            .collect();
        ticks.sort_by(|a, b| a.0.total_cmp(&b.0));
        ticks.retain(|t| t.0.is_finite());
        if ticks.is_empty() {
            return used;
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
        let (left, right) = self.xpd_limits(x_axis, opts.xpd.unwrap_or(self.par.xpd));
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
                    self.data_to_inches(1.0, p).y
                } else {
                    self.data_to_inches(p, 1.0).x
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
            let len = match opts.tck.or(self.par.tck).filter(|t| t.is_finite()) {
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
            self.push_clip_region(Xpd::Device);
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
            return used;
        }
        let font = opts.font.unwrap_or(self.par.font_axis);
        let cex = opts.cex_axis.unwrap_or(self.par.cex_axis);
        let col_axis = opts.col_axis.unwrap_or(ColorSpec::Color(self.par.col_axis));
        let face = font;
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
        let angle = if x_axis == perpendicular { 90.0 } else { 0.0 };
        let bias = self.line_bias(side, !perpendicular);
        let label_perp = axis_base + outward * (mgp[1] - lineoff + bias) * lh;

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
                        self.math_text(anchor, expr, hadj, padj, angle, col, cex, Xpd::Device);
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
                    Xpd::Device,
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
        used
    }
}

/// R's checks of `axis()`'s `labels`: given only with `at`, and as many.
pub(crate) fn check_axis(opts: &AxisOptions) -> Result<(), Error> {
    if let (true, Some(labels)) = (opts.draw_labels, &opts.labels) {
        match &opts.at {
            None => return Err(Error::invalid("'labels' is supplied and not 'at'")),
            Some(at) if at.len() != labels.len() => {
                return Err(Error::invalid(format!(
                    "'at' and 'labels' lengths differ, {} != {}",
                    at.len(),
                    labels.len()
                )));
            }
            Some(_) => {}
        }
    }
    Ok(())
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
