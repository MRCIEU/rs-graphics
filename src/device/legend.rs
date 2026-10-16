//! `legend()`: a port of the layout algorithm in R's
//! `graphics/R/legend.R`, including keyword positions (`"topright"`, ...),
//! multi-column / horizontal legends, titles, filled boxes and merged
//! point+line keys.
//!
//! R computes everything in *user* coordinates; since the user -> device
//! mapping is linear (also for log axes, where user coordinates are
//! `log10`-transformed), this port works directly in device inches (y up),
//! which gives identical results for non-reversed axes. Reversed axes
//! (`xlim = c(hi, lo)`) are not handled.
//!
//! Labels and the title may be plotmath expressions (`Label::math`).
//!
//! Not implemented: `density`/`angle` hatched fills,
//! per-entry `cex`/`x.intersp`/`seg.len` recycling (these are scalars on
//! [`LegendOptions`]), `text.width = NA`, and `adj`.
//!
//! `par(cex)` scales everything as in `legend.R`: the character cell
//! (`xc`/`yc`) and the measured and drawn text (R's `strwidth(cex = )` and
//! `text(cex = )` both multiply by `par("cex")`, i.e. `cexbase`).

use crate::backend::Backend;
use crate::color::ColorSpec;
use crate::geometry::{Point, Rect};
use crate::lty::LineType;

use crate::plotmath::Label;

use super::{Clip, Device};

/// Where to put a legend: R's `legend(x, y)` forms.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LegendPos {
    /// A single point in user coordinates; anchored by `xjust`/`yjust`
    /// (default: the top-left corner of the legend box).
    Xy(f64, f64),
    /// Two opposite corners of the legend box, in user coordinates (R's
    /// `nx == 2` case): the box is stretched to fit exactly.
    Corners(f64, f64, f64, f64),
    /// One of R's nine keyword positions, relative to the plot region.
    Keyword(LegendKeyword),
}

/// R's `"bottomright"`, `"bottom"`, ... keyword positions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LegendKeyword {
    BottomRight,
    Bottom,
    BottomLeft,
    Left,
    TopLeft,
    Top,
    TopRight,
    Right,
    Center,
}

impl LegendKeyword {
    const ALL: [(&'static str, LegendKeyword); 9] = [
        ("bottomright", LegendKeyword::BottomRight),
        ("bottom", LegendKeyword::Bottom),
        ("bottomleft", LegendKeyword::BottomLeft),
        ("left", LegendKeyword::Left),
        ("topleft", LegendKeyword::TopLeft),
        ("top", LegendKeyword::Top),
        ("topright", LegendKeyword::TopRight),
        ("right", LegendKeyword::Right),
        ("center", LegendKeyword::Center),
    ];

    /// Parse like R's `match.arg()`: an exact match, or else a unique
    /// prefix (so `"topr"` is `TopRight`, but `"to"` is ambiguous).
    pub fn parse(s: &str) -> Option<LegendKeyword> {
        if let Some(&(_, k)) = Self::ALL.iter().find(|(name, _)| *name == s) {
            return Some(k);
        }
        let mut it = Self::ALL
            .iter()
            .filter(|(name, _)| !s.is_empty() && name.starts_with(s));
        match (it.next(), it.next()) {
            (Some(&(_, k)), None) => Some(k),
            _ => None,
        }
    }
}

crate::error::from_str_via_parse!(LegendKeyword, "legend position");

/// One row (key + label) of a legend.
#[derive(Clone, Debug)]
pub struct LegendEntry {
    /// Text or a plotmath expression (`Label::math(...)`).
    pub label: Label,
    /// Colour of the point/line symbol.
    pub col: ColorSpec,
    pub pch: Option<i32>,
    /// `None` = no line segment in the key. A blank `lty` also draws none.
    pub lty: Option<LineType>,
    /// Line width of the segment; `None` = `par(lwd)`.
    pub lwd: Option<f64>,
    /// Fill colour of a filled box key (R's `fill=`); `None` = no box for
    /// this row (but if any other row has a box, this row gets an empty one,
    /// as in R).
    pub fill: Option<ColorSpec>,
    /// `pt.bg`: background for `pch` 21..25.
    pub pt_bg: ColorSpec,
    /// `pt.cex`; `None` = the legend's `cex`.
    pub pt_cex: Option<f64>,
    /// `pt.lwd`; `None` = this row's line width.
    pub pt_lwd: Option<f64>,
}

impl LegendEntry {
    fn base(label: impl Into<Label>, col: ColorSpec) -> Self {
        LegendEntry {
            label: label.into(),
            col,
            pch: None,
            lty: None,
            lwd: None,
            fill: None,
            pt_bg: ColorSpec::None,
            pt_cex: None,
            pt_lwd: None,
        }
    }

    /// `legend(..., pch = pch, col = col)`.
    pub fn point(label: impl Into<Label>, col: ColorSpec, pch: i32) -> Self {
        LegendEntry {
            pch: Some(pch),
            ..Self::base(label, col)
        }
    }

    /// `legend(..., lty = lty, lwd = lwd, col = col)`.
    pub fn line(label: impl Into<Label>, col: ColorSpec, lwd: f64, lty: LineType) -> Self {
        LegendEntry {
            lty: Some(lty),
            lwd: Some(lwd),
            ..Self::base(label, col)
        }
    }

    /// `legend(..., fill = fill)`: a filled box key.
    pub fn filled(label: impl Into<Label>, fill: ColorSpec) -> Self {
        LegendEntry {
            fill: Some(fill),
            ..Self::base(label, ColorSpec::None)
        }
    }

    /// Adds a line segment to a point entry (R's `pch` + `lty`, drawn merged
    /// by default).
    pub fn with_line(mut self, lwd: f64, lty: LineType) -> Self {
        self.lty = Some(lty);
        self.lwd = Some(lwd);
        self
    }

    pub fn with_pt_bg(mut self, bg: ColorSpec) -> Self {
        self.pt_bg = bg;
        self
    }

    pub fn with_pt_cex(mut self, cex: f64) -> Self {
        self.pt_cex = Some(cex);
        self
    }

    fn has_line(&self) -> bool {
        matches!(self.lty, Some(l) if l != LineType::Blank)
    }

    fn has_pch(&self) -> bool {
        matches!(self.pch, Some(p) if p >= 0 || p <= -32)
    }
}

/// The scalar arguments of R's `legend()`, with R's defaults.
#[derive(Clone, Debug)]
pub struct LegendOptions {
    /// `bty != "n"`: draw the surrounding box.
    pub bty_box: bool,
    /// Box fill (`bg=`); `None` = `par("bg")` (transparent, so unfilled,
    /// on PDF by default: see [`Backend::default_bg`]).
    pub bg: Option<ColorSpec>,
    pub cex: f64,
    /// `xjust` / `yjust`; `None` = R's default (0 / 1, or 0.5 for
    /// [`LegendPos::Corners`]).
    pub xjust: Option<f64>,
    pub yjust: Option<f64>,
    pub x_intersp: f64,
    pub y_intersp: f64,
    /// Length of line keys, in character widths.
    pub seg_len: f64,
    /// `text.width`, in x *user* units; `None` = widest label.
    pub text_width: Option<f64>,
    /// `merge`; `None` = R's default (`do.lines && has.pch`).
    pub merge: Option<bool>,
    pub ncol: usize,
    pub horiz: bool,
    /// Text or a plotmath expression.
    pub title: Option<Label>,
    /// `title.cex`; `None` = `cex`.
    pub title_cex: Option<f64>,
    /// `inset` as fractions of the plot region, `(x, y)`, for keyword
    /// positions.
    pub inset: (f64, f64),
    /// `text.col`; `None` = `par("col")`.
    pub text_col: Option<ColorSpec>,
    /// `text.font`; `None` = `par("font")`.
    pub text_font: Option<i32>,
    /// Border colour of filled box keys (`border=`).
    pub border: ColorSpec,
    /// Temporarily override `par("xpd")` while drawing.
    pub xpd: Option<bool>,
}

impl Default for LegendOptions {
    fn default() -> Self {
        LegendOptions {
            bty_box: true,
            bg: None,
            cex: 1.0,
            xjust: None,
            yjust: None,
            x_intersp: 1.0,
            y_intersp: 1.0,
            seg_len: 2.0,
            text_width: None,
            merge: None,
            ncol: 1,
            horiz: false,
            title: None,
            title_cex: None,
            inset: (0.0, 0.0),
            text_col: None,
            text_font: None,
            border: ColorSpec::Color(crate::color::Color::BLACK),
            xpd: None,
        }
    }
}

/// The legend box that was drawn, in device inches (y up): what R returns as
/// `legend()$rect` (there in user units).
#[derive(Clone, Copy, Debug)]
pub struct LegendLayout {
    pub left: f64,
    pub top: f64,
    pub w: f64,
    pub h: f64,
}

impl LegendLayout {
    pub fn rect(&self) -> Rect {
        Rect::new(self.left, self.top - self.h, self.left + self.w, self.top)
    }
}

impl<B: Backend> Device<B> {
    /// `legend(pos, ...)`: see the module docs. Returns the box geometry, or
    /// `None` if `entries` is empty (R errors there).
    pub fn legend(
        &mut self,
        pos: LegendPos,
        entries: &[LegendEntry],
        opts: &LegendOptions,
    ) -> Option<LegendLayout> {
        if entries.is_empty() {
            return None;
        }
        let saved_xpd = self.par.xpd;
        if let Some(xpd) = opts.xpd {
            self.par.xpd = xpd;
        }
        let layout = self.legend_impl(pos, entries, opts);
        self.par.xpd = saved_xpd;
        Some(layout)
    }

    fn legend_impl(
        &mut self,
        pos: LegendPos,
        entries: &[LegendEntry],
        opts: &LegendOptions,
    ) -> LegendLayout {
        let n = entries.len();
        let plt = self.plt;
        let cex = opts.cex;

        // Number of columns / rows ("legends per column").
        let (ncol, n_per_col) = if opts.horiz {
            (n, 1)
        } else {
            let ncol = opts.ncol.max(1);
            (ncol, n.div_ceil(ncol))
        };

        // One character cell: `xyinch(par("cin"))` scaled by the effective
        // cex. For R's pdf device `cin = (0.9, 1.2) * startps / 72`.
        let cex_eff = cex * self.par.cex;
        let cin_x = 0.9 * self.start_ps / 72.0;
        let cin_y = 1.2 * self.start_ps / 72.0;
        let xc = cex_eff * cin_x;
        let yc = cex_eff * cin_y;

        let font = opts.text_font.unwrap_or(self.par.font);
        let (ux0, ux1, _, _) = self.usr;
        let inches_per_user_x = plt.width() / (ux1 - ux0).abs();
        let text_w = match opts.text_width {
            Some(tw) => tw.abs() * inches_per_user_x,
            None => entries
                .iter()
                .map(|e| self.label_width(&e.label, font, cex))
                .fold(0.0, f64::max),
        };

        // Row heights: at least a character cell, or the tallest label
        // (an expression can be taller).
        let yextra = yc * (opts.y_intersp - 1.0);
        let ymax = entries
            .iter()
            .map(|e| self.label_height(&e.label, font, cex))
            .fold(yc, f64::max);
        let ychar = yextra + ymax;

        // Title.
        let title = opts
            .title
            .as_ref()
            .filter(|t| !matches!(t, Label::Text(s) if s.is_empty()));
        let tcex = opts.title_cex.unwrap_or(cex);
        let title_unit_h = tcex * self.par.cex * cin_y;
        let ymaxtitle = match title {
            Some(t) => title_unit_h.max(self.label_height(t, font, tcex)),
            None => title_unit_h,
        };
        let ychartitle = yextra + ymaxtitle;
        let title_extra_h = if title.is_some() { ychartitle } else { 0.0 };

        let has_fill = entries.iter().any(|e| e.fill.is_some());
        let xbox = xc * 0.8;
        let ybox = yc * 0.5;
        let dx_fill = xbox;
        let do_lines = entries.iter().any(|e| e.has_line());
        let has_pch = entries.iter().any(|e| e.has_pch());
        let merge = opts.merge.unwrap_or(do_lines && has_pch);
        let x_off = if do_lines && merge { -0.7 } else { 0.0 };

        let mut w0;
        let (left, top, w, h, xextra);
        match pos {
            LegendPos::Corners(xa, ya, xb, yb) => {
                let pa = self.user_to_inches(xa, ya);
                let pb = self.user_to_inches(xb, yb);
                left = pa.x.min(pb.x);
                top = pa.y.max(pb.y);
                w = (pa.x - pb.x).abs();
                h = (pa.y - pb.y).abs();
                w0 = w / ncol as f64;
                xextra = 0.0;
            }
            _ => {
                let h_ = n_per_col as f64 * ychar + yc + title_extra_h;

                w0 = text_w + (opts.x_intersp + 1.0) * xc;
                if has_fill {
                    w0 += dx_fill;
                }
                if do_lines {
                    w0 += (opts.seg_len + x_off) * xc;
                }
                let mut w_ = ncol as f64 * w0 + 0.5 * xc;
                let mut xextra_ = 0.0;
                if let Some(t) = title {
                    let tw = self.label_width(t, font, tcex) + 0.5 * tcex * self.par.cex * cin_x;
                    if tw > w_ {
                        xextra_ = (tw - w_) / 2.0;
                        w_ = tw;
                    }
                }

                let (l, t) = match pos {
                    LegendPos::Xy(x, y) => {
                        let p = self.user_to_inches(x, y);
                        let xjust = opts.xjust.unwrap_or(0.0);
                        let yjust = opts.yjust.unwrap_or(1.0);
                        (p.x - xjust * w_, p.y + (1.0 - yjust) * h_)
                    }
                    LegendPos::Keyword(k) => {
                        use LegendKeyword::*;
                        let insetx = opts.inset.0 * plt.width();
                        let insety = opts.inset.1 * plt.height();
                        let l = match k {
                            BottomRight | TopRight | Right => plt.x1 - w_ - insetx,
                            BottomLeft | Left | TopLeft => plt.x0 + insetx,
                            Bottom | Top | Center => (plt.x0 + plt.x1 - w_) / 2.0,
                        };
                        let t = match k {
                            BottomRight | Bottom | BottomLeft => plt.y0 + h_ + insety,
                            TopLeft | Top | TopRight => plt.y1 - insety,
                            Left | Right | Center => (plt.y0 + plt.y1 + h_) / 2.0,
                        };
                        (l, t)
                    }
                    LegendPos::Corners(..) => unreachable!(),
                };
                left = l;
                top = t;
                w = w_;
                h = h_;
                xextra = xextra_;
            }
        }

        // The legend itself. Graphics are clipped according to `xpd`, like
        // the `rect()`/`segments()`/`points()` calls R makes.
        let lwd_box = self.par.lwd;
        self.push_clip_for(self.par.xpd);
        if opts.bty_box {
            let bg = opts.bg.unwrap_or(ColorSpec::Color(self.par.bg));
            let fill = self.resolve(bg).filter(|c| c.a > 0);
            let stroke = self.stroke_of(self.par.fg, lwd_box, self.par.lty);
            self.backend
                .draw_rect(Rect::new(left, top - h, left + w, top), fill, Some(&stroke));
        }

        let topspace = 0.5 * ymax + title_extra_h;
        let mut xt: Vec<f64> = (0..n)
            .map(|i| left + xc + xextra + (i / n_per_col) as f64 * w0)
            .collect();
        let yt: Vec<f64> = (0..n)
            .map(|i| top - topspace - ychar * ((i % n_per_col) as f64 + 0.5))
            .collect();

        if has_fill {
            let border = self.resolve(opts.border);
            let stroke = border.map(|c| self.stroke_of(c, self.par.lwd, self.par.lty));
            for i in 0..n {
                let fill = entries[i].fill.and_then(|f| self.resolve(f));
                self.backend.draw_rect(
                    Rect::new(xt[i], yt[i] - ybox / 2.0, xt[i] + xbox, yt[i] + ybox / 2.0),
                    fill,
                    stroke.as_ref(),
                );
                xt[i] += dx_fill;
            }
        }

        if do_lines {
            for i in 0..n {
                let e = &entries[i];
                if let (true, Some(col), Some(lty)) = (e.has_line(), self.resolve(e.col), e.lty) {
                    let stroke = self.stroke_of(col, e.lwd.unwrap_or(self.par.lwd), lty);
                    let x0 = xt[i] + x_off * xc;
                    self.backend.draw_line(
                        Point::new(x0, yt[i]),
                        Point::new(x0 + opts.seg_len * xc, yt[i]),
                        &stroke,
                    );
                }
                xt[i] += (opts.seg_len + x_off) * xc;
            }
        }

        if has_pch {
            for i in 0..n {
                let e = &entries[i];
                if !e.has_pch() {
                    continue;
                }
                let Some(col) = self.resolve(e.col) else {
                    continue;
                };
                let x1 = if merge && do_lines {
                    xt[i] - (opts.seg_len / 2.0) * xc
                } else {
                    xt[i]
                };
                let pt_cex = e.pt_cex.unwrap_or(cex);
                let bg = self.resolve(e.pt_bg);
                let lwd = e.pt_lwd.or(e.lwd).unwrap_or(self.par.lwd);
                self.draw_pch(
                    e.pch.unwrap_or(0),
                    Point::new(x1, yt[i]),
                    pt_cex,
                    col,
                    bg,
                    lwd,
                );
            }
        }
        self.pop_clip();

        let text_col = opts.text_col.unwrap_or(ColorSpec::Color(self.par.col));
        // `text(adj = c(title.adj, 0))` and `text(adj = c(0, 0.5))`.
        let clip = if self.par.xpd {
            Clip::Figure
        } else {
            Clip::Plot
        };
        if let Some(t) = title {
            let anchor = Point::new(left + w * 0.5, top - ymaxtitle);
            self.draw_label(clip, anchor, t, 0.5, Some(0.0), 0.0, text_col, tcex, font);
        }
        for i in 0..n {
            let anchor = Point::new(xt[i] + opts.x_intersp * xc, yt[i]);
            let label = &entries[i].label;
            self.draw_label(
                clip,
                anchor,
                label,
                0.0,
                Some(0.5),
                0.0,
                text_col,
                cex,
                font,
            );
        }

        LegendLayout { left, top, w, h }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyword_parsing_is_like_match_arg() {
        assert_eq!(
            LegendKeyword::parse("topright"),
            Some(LegendKeyword::TopRight)
        );
        assert_eq!(LegendKeyword::parse("top"), Some(LegendKeyword::Top));
        assert_eq!(LegendKeyword::parse("topr"), Some(LegendKeyword::TopRight));
        assert_eq!(LegendKeyword::parse("cen"), Some(LegendKeyword::Center));
        assert_eq!(LegendKeyword::parse("to"), None);
        assert_eq!(LegendKeyword::parse("nowhere"), None);
        assert_eq!(LegendKeyword::parse(""), None);
    }
}
