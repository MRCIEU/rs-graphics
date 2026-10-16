//! `axis()`: a port of `C_axis` (`plot.c`, incl. `GetAxisLimits`,
//! `getxlimits`/`getylimits` and `ComputePAdjValue`), with the labels placed
//! by `GMtext()` (`graphics.c`).
//!
//! The axis line sits `line` margin lines out from the plot region (or the
//! inner region, for `outer`), or at the user coordinate `pos`. Labels sit
//! `mgp[1]` lines beyond the axis line; when neither `line` nor `pos` is
//! given, the axis line sits at `mgp[2]` and the labels at `mgp[1]`, both
//! measured from the plot region. Lines and labels are clipped to the
//! device, as R forces `xpd = NA`.

use crate::afm;
use crate::backend::Backend;
use crate::color::ColorSpec;
use crate::geometry::Point;
use crate::lty::LineType;
use crate::par::AxisType;

use super::{Clip, Device, Side, Y_LINE_BIAS, format_axis_labels};

/// `FLT_EPSILON`, which `GetAxisLimits()` widens the axis range by.
const FLT_EPSILON: f64 = f32::EPSILON as f64;

/// The arguments of R's `axis()` other than `side`, plus the inline
/// graphical parameters it uses, with R's defaults. `None` is R's `NA` (or
/// `NULL`), which falls back to the matching `par()` value.
#[derive(Clone, Debug)]
pub struct AxisOptions {
    /// Tick positions, in user coordinates (data units, also on a log
    /// axis).
    /// `None`: `pretty()` on the axis range.
    pub at: Option<Vec<f64>>,
    /// Labels for `at` (requires `at`). `None`: `at` formatted like R's
    /// `formatReal()`.
    pub labels: Option<Vec<String>>,
    /// `labels = FALSE` when false: no labels at all.
    pub draw_labels: bool,
    /// `tick`: draw the axis line and tick marks.
    pub tick: bool,
    /// Margin lines from the plot region (inner region for `outer`) to the
    /// axis line. Labels then sit `mgp[1]` lines beyond it.
    pub line: Option<f64>,
    /// User coordinate (on the other axis) at which to draw the axis line;
    /// overrides `line`. Labels sit `mgp[1]` lines beyond it.
    pub pos: Option<f64>,
    /// Measure `line` from the inner region (inside `par(oma)`) rather
    /// than the plot region.
    pub outer: bool,
    /// Label font; `None`: `par(font.axis)`.
    pub font: Option<i32>,
    /// Line type of the axis line and ticks (R: `"solid"`).
    pub lty: LineType,
    /// Width of the axis line (R: 1, whatever `par(lwd)` is). 0 or less
    /// draws no axis line.
    pub lwd: f64,
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
    /// `par(cex.axis)` for the labels.
    pub cex_axis: Option<f64>,
    /// `par(col.axis)`: label colour.
    pub col_axis: Option<ColorSpec>,
    /// `par(las)`: label orientation.
    pub las: Option<i32>,
    /// `par(xaxp)` (sides 1, 3) or `par(yaxp)` (sides 2, 4) for the
    /// default ticks when `at` is `None`.
    pub axp: Option<[f64; 3]>,
    /// `par(xaxt)` (sides 1, 3) or `par(yaxt)` (sides 2, 4):
    /// `AxisType::None` draws nothing.
    pub axt: Option<AxisType>,
    /// `par(tck)`: tick length as a fraction of the plot region (of its
    /// height for an x axis if above 0.5, else of its smaller side);
    /// positive points inwards. Overrides `tcl`.
    pub tck: Option<f64>,
    /// `par(tcl)`: tick length in margin lines; negative points outwards.
    pub tcl: Option<f64>,
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
            lty: LineType::Pattern(&[]),
            lwd: 1.0,
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
    let x_axis = side == 1 || side == 3;
    match las {
        1 if !x_axis => 0.5,
        2 => 0.5,
        3 if x_axis => 0.5,
        _ => 0.0,
    }
}

impl<B: Backend> Device<B> {
    /// `axis(side, ...)` with all of R's arguments; see [`AxisOptions`].
    ///
    /// # Panics
    /// If `labels` is given without `at`, or their lengths differ (R's
    /// errors).
    pub fn axis_with(&mut self, side: Side, opts: &AxisOptions) {
        let x_axis = side == 1 || side == 3;
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
                None => format_axis_labels(&at),
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
        let mut ticks: Vec<(f64, Option<&str>)> = at
            .iter()
            .enumerate()
            .map(|(i, &v)| (v, labels.get(i).map(String::as_str)))
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
        let lh = self.line_height();
        let outward = if side <= 2 { -1.0 } else { 1.0 };
        let axis_base = match pos {
            Some(p) => {
                if x_axis {
                    self.user_to_inches(1.0, p).y
                } else {
                    self.user_to_inches(p, 1.0).x
                }
            }
            None => {
                let edge = if opts.outer {
                    let omi = self.par.omi();
                    match side {
                        1 => omi.bottom,
                        2 => omi.left,
                        3 => self.height_in - omi.top,
                        _ => self.width_in - omi.right,
                    }
                } else {
                    match side {
                        1 => self.plt.y0,
                        2 => self.plt.x0,
                        3 => self.plt.y1,
                        _ => self.plt.x1,
                    }
                };
                edge + outward * line * lh
            }
        };

        if opts.tick {
            let col = opts.col.unwrap_or(ColorSpec::Color(self.par.fg));
            let col_ticks = opts.col_ticks.unwrap_or(col);
            let lwd_ticks = opts.lwd_ticks.unwrap_or(opts.lwd);
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
            if opts.lwd > 0.0 {
                if let Some(c) = self.resolve(col) {
                    let stroke = self.stroke_of(c, opts.lwd, opts.lty);
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
                    let stroke = self.stroke_of(c, lwd_ticks, opts.lty);
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
                if side <= 2 { 1.0 } else { 0.0 }
            } else {
                0.5
            });
        // `GMtext()`'s rotation and `yLineBias` nudge.
        let (angle, bias) = match (side, perpendicular) {
            (1 | 3, true) => (90.0, 0.0),
            (_, true) => (0.0, 0.0),
            (1, false) => (0.0, 1.0 - Y_LINE_BIAS),
            (2, false) => (90.0, Y_LINE_BIAS),
            (3, false) => (0.0, Y_LINE_BIAS),
            (_, false) => (90.0, 1.0 - Y_LINE_BIAS),
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
        for side in 1..=4 {
            assert_eq!(default_padj(side, 0), 0.0);
            assert_eq!(default_padj(side, 2), 0.5);
        }
        assert_eq!(default_padj(1, 1), 0.0);
        assert_eq!(default_padj(2, 1), 0.5);
        assert_eq!(default_padj(3, 3), 0.5);
        assert_eq!(default_padj(4, 3), 0.0);
    }
}
