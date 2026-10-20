// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! `legend()`: a port of the layout algorithm in R's
//! `graphics/R/legend.R`, including keyword positions (`"topright"`, ...),
//! multi-column / horizontal legends, titles, filled boxes and merged
//! point+line keys.
//!
//! R computes everything in *user* coordinates; since the user -> device
//! mapping is linear (also for log axes, where user coordinates are
//! `log10`-transformed), this port works directly in device inches (y up),
//! and converts what it returns ([`LegendLayout`]) back to user
//! coordinates. That gives R's results on reversed axes (`xlim = c(hi,
//! lo)`) too, where R's widths and heights turn negative together, with
//! one exception: for [`LegendPos::Corners`] on a reversed axis R sorts the
//! corners but then steps by its negative character size, so it draws the
//! entries outside its own box. That is not copied: here they go inside,
//! as on other axes.
//!
//! Labels and the title may be plotmath expressions (`Label::math`). As
//! R draws them with `text()`, they are rotated by `par(srt)`.
//!
//! Not implemented: `density`/`angle` hatched fills,
//! per-entry `cex`/`x.intersp`/`seg.len` recycling (these are scalars on
//! [`LegendOptions`]), `text.width = NA`, `adj` and `trace`.
//!
//! `par(cex)` scales everything as in `legend.R`: the character cell
//! (`xc`/`yc`) and the measured and drawn text (R's `strwidth(cex = )` and
//! `text(cex = )` both multiply by `par("cex")`, i.e. `cexbase`).

use crate::backend::Backend;
use crate::color::ColorSpec;
use crate::geometry::{Point, Rect};
use crate::lty::LineType;
use crate::numbers::Pair;

use crate::plotmath::Label;

use super::{Device, Xpd};

/// Where to put a legend: R's `legend(x, y)` forms.
///
/// [`legend()`](crate::graphics::Graphics::legend) takes anything that
/// converts into one, so positions can be written as in R:
///
/// | R | Rust shorthand | Variant |
/// |---|---|---|
/// | `legend("topright", ...)` | `"topright"` (or `LegendKeyword::TopRight`) | [`Keyword`](LegendPos::Keyword) |
/// | `legend(5, 20, ...)` | `(5.0, 20.0)` | [`Xy`](LegendPos::Xy) |
/// | `legend(c(5, 15), c(10, 20), ...)` | `((5.0, 15.0), (10.0, 20.0))` | [`Corners`](LegendPos::Corners) |
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LegendPos {
    /// A single point `(x, y)` in user coordinates; anchored by
    /// `xjust`/`yjust` (default: the top-left corner of the legend box).
    Xy(f64, f64),
    /// Two opposite corners of the legend box, `(xa, ya, xb, yb)` in user
    /// coordinates (R's `nx == 2` case): the box is stretched to fit
    /// exactly.
    Corners(f64, f64, f64, f64),
    /// One of R's nine keyword positions, relative to the plot region.
    Keyword(LegendKeyword),
}

impl From<LegendKeyword> for LegendPos {
    fn from(k: LegendKeyword) -> Self {
        LegendPos::Keyword(k)
    }
}

impl From<&str> for LegendPos {
    /// One of R's keywords, matched like R's `match.arg()`
    /// ([`LegendKeyword::parse`]: the full name, or a unique prefix such
    /// as `"topr"`).
    ///
    /// # Panics
    /// If `s` is not a keyword or is an ambiguous prefix (R errors too);
    /// use [`LegendKeyword::parse`] or `str::parse` to handle that
    /// yourself.
    fn from(s: &str) -> Self {
        match LegendKeyword::parse(s) {
            Some(k) => LegendPos::Keyword(k),
            None => panic!(
                "legend: {s:?} is not a legend position; use one of \
                 \"bottomright\", \"bottom\", \"bottomleft\", \"left\", \
                 \"topleft\", \"top\", \"topright\", \"right\" or \"center\""
            ),
        }
    }
}

impl From<String> for LegendPos {
    /// As `From<&str>`.
    fn from(s: String) -> Self {
        LegendPos::from(s.as_str())
    }
}

impl From<&String> for LegendPos {
    /// As `From<&str>`.
    fn from(s: &String) -> Self {
        LegendPos::from(s.as_str())
    }
}

impl<P: Pair> From<P> for LegendPos {
    /// `(x, y)`: R's `legend(x, y)`, a point in user coordinates (any
    /// [`Pair`], e.g. `(5, 20)`).
    fn from(p: P) -> Self {
        let (x, y) = p.into_pair();
        LegendPos::Xy(x, y)
    }
}

/// `From<((xa, xb), (ya, yb))>` for each mix of `f64` and `i32`.
macro_rules! corners {
    ($(($a:ty, $b:ty, $c:ty, $d:ty)),*) => {
        $(
            impl From<(($a, $b), ($c, $d))> for LegendPos {
                /// `((xa, xb), (ya, yb))`: R's `legend(x = c(xa, xb),
                /// y = c(ya, yb))`, two opposite corners of the box.
                fn from((x, y): (($a, $b), ($c, $d))) -> Self {
                    let ((xa, xb), (ya, yb)) = (x.into_pair(), y.into_pair());
                    LegendPos::Corners(xa, ya, xb, yb)
                }
            }
        )*
    };
}
corners!(
    (f64, f64, f64, f64),
    (f64, f64, f64, i32),
    (f64, f64, i32, f64),
    (f64, f64, i32, i32),
    (f64, i32, f64, f64),
    (f64, i32, f64, i32),
    (f64, i32, i32, f64),
    (f64, i32, i32, i32),
    (i32, f64, f64, f64),
    (i32, f64, f64, i32),
    (i32, f64, i32, f64),
    (i32, f64, i32, i32),
    (i32, i32, f64, f64),
    (i32, i32, f64, i32),
    (i32, i32, i32, f64),
    (i32, i32, i32, i32)
);

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

/// R's `legend(bty = )`: whether the legend has a box around it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LegendBty {
    /// `"o"` (the default): draw the box.
    #[default]
    Box,
    /// `"n"`: no box.
    None,
}

impl LegendBty {
    /// `"o"` or `"n"`.
    pub fn parse(s: &str) -> Option<LegendBty> {
        match s {
            "o" => Some(LegendBty::Box),
            "n" => Some(LegendBty::None),
            _ => None,
        }
    }
}

crate::error::from_str_via_parse!(LegendBty, "legend box type");
crate::error::from_r_string!(LegendBty, "legend box type", "\"o\" or \"n\"");

/// One row (key + label) of a legend.
#[derive(Clone, Debug)]
pub struct LegendEntry {
    /// Text or a plotmath expression (`Label::math(...)`). Required.
    pub label: Label,
    /// Colour of the point/line symbol. Required (set by the
    /// constructors).
    pub col: ColorSpec,
    /// Plotting symbol of the key; `None` = no point.
    pub pch: Option<i32>,
    /// `None` = no line segment in the key. A blank `lty` also draws none.
    pub lty: Option<LineType>,
    /// Line width of the segment; `None` = `par(lwd)`.
    pub lwd: Option<f64>,
    /// Fill colour of a filled box key (R's `fill=`); `None` = no box for
    /// this row (but if any other row has a box, this row gets an empty one,
    /// as in R).
    pub fill: Option<ColorSpec>,
    /// `pt.bg`: background for `pch` 21..25; `None`: none (R's `NA`).
    pub pt_bg: Option<ColorSpec>,
    /// `pt.cex`; `None` = the legend's `cex`.
    pub pt_cex: Option<f64>,
    /// `pt.lwd`; `None` = this row's line width.
    pub pt_lwd: Option<f64>,
}

crate::setters::setters!(LegendEntry {
    label: value Label,
    col: value ColorSpec,
    pch: option i32,
    lty: option LineType,
    lwd: option f64,
    fill: option ColorSpec,
    pt_bg: option ColorSpec,
    pt_cex: option f64,
    pt_lwd: option f64,
});

impl LegendEntry {
    fn base(label: impl Into<Label>, col: ColorSpec) -> Self {
        LegendEntry {
            label: label.into(),
            col,
            pch: None,
            lty: None,
            lwd: None,
            fill: None,
            pt_bg: None,
            pt_cex: None,
            pt_lwd: None,
        }
    }

    /// `legend(..., pch = pch, col = col)`.
    pub fn point(label: impl Into<Label>, col: impl Into<ColorSpec>, pch: i32) -> Self {
        LegendEntry {
            pch: Some(pch),
            ..Self::base(label, col.into())
        }
    }

    /// `legend(..., lty = lty, lwd = lwd, col = col)`; `lty` is a
    /// [`LineType`] or R's string (`"dashed"`).
    pub fn line(
        label: impl Into<Label>,
        col: impl Into<ColorSpec>,
        lwd: impl Into<f64>,
        lty: impl Into<LineType>,
    ) -> Self {
        LegendEntry {
            lty: Some(lty.into()),
            lwd: Some(lwd.into()),
            ..Self::base(label, col.into())
        }
    }

    /// `legend(..., fill = fill)`: a filled box key.
    pub fn filled(label: impl Into<Label>, fill: impl Into<ColorSpec>) -> Self {
        LegendEntry {
            fill: Some(fill.into()),
            ..Self::base(label, ColorSpec::None)
        }
    }

    fn has_line(&self) -> bool {
        matches!(self.lty, Some(l) if l != LineType::Blank)
    }

    fn has_pch(&self) -> bool {
        matches!(self.pch, Some(p) if p >= 0 || p <= -32)
    }
}

/// The arguments of R's `legend()` other than the per-entry keys (which
/// are on [`LegendEntry`]). `None` (or an empty `Vec`) is R's default,
/// given for each field.
#[derive(Clone, Debug, Default)]
pub struct LegendOptions {
    /// `bty`: whether to draw the surrounding box; `None`: a box (R's
    /// `"o"`).
    pub bty: Option<LegendBty>,
    /// Box fill (`bg=`); `None` = `par("bg")` (transparent, so unfilled,
    /// on PDF by default: see [`Backend::default_bg`]).
    pub bg: Option<ColorSpec>,
    /// Size of the text and keys, times `par(cex)`; `None`: 1.
    pub cex: Option<f64>,
    /// Horizontal justification of the box relative to `x`; `None`: R's
    /// default (0, or 0.5 for [`LegendPos::Corners`]).
    pub xjust: Option<f64>,
    /// Vertical justification of the box relative to `y`; `None`: R's
    /// default (1, or 0.5 for [`LegendPos::Corners`]).
    pub yjust: Option<f64>,
    /// Space between key and label, in character widths; `None`: 1.
    pub x_intersp: Option<f64>,
    /// Space between rows, in character heights; `None`: 1.
    pub y_intersp: Option<f64>,
    /// Length of line keys, in character widths; `None`: 2.
    pub seg_len: Option<f64>,
    /// `text.width`, in x *user* units; `None` = widest label.
    pub text_width: Option<f64>,
    /// `merge`; `None` = R's default (`do.lines && has.pch`).
    pub merge: Option<bool>,
    /// Number of columns; `None`: 1.
    pub ncol: Option<usize>,
    /// Lay the entries out in one row. Default `false`.
    pub horiz: bool,
    /// Text or a plotmath expression above the entries; `None`: no title.
    pub title: Option<Label>,
    /// `title.cex`; `None` = `cex`.
    pub title_cex: Option<f64>,
    /// `title.col`; `None` = the first `text_col`.
    pub title_col: Option<ColorSpec>,
    /// `title.adj`: where the title sits across the box (0 left, 1
    /// right) and its justification there; `None`: 0.5 (centred).
    pub title_adj: Option<f64>,
    /// `title.font`; `None` = the first `text_font`.
    pub title_font: Option<i32>,
    /// `inset` as fractions of the plot region, `(x, y)`, for keyword
    /// positions; `None`: `(0, 0)`.
    pub inset: Option<(f64, f64)>,
    /// `text.col`: label colours, recycled over the entries; empty:
    /// `par("col")`.
    pub text_col: Vec<ColorSpec>,
    /// `text.font`: label fonts, recycled over the entries (also for
    /// measuring them); empty: `par("font")`.
    pub text_font: Vec<i32>,
    /// `box.lwd`: line width of the legend box; `None`: `par("lwd")`.
    pub box_lwd: Option<f64>,
    /// `box.lty`: line type of the legend box; `None`: `par("lty")`.
    pub box_lty: Option<LineType>,
    /// `box.col`: colour of the legend box's border (`ColorSpec::None`:
    /// no border); `None`: `par("fg")`.
    pub box_col: Option<ColorSpec>,
    /// Border colour of filled box keys (`border=`); `None`: black (R's
    /// default).
    pub border: Option<ColorSpec>,
    /// R's inline `xpd` while drawing ([`Xpd`]; the setter also takes
    /// `false`/`true`); `None`: `par(xpd)`.
    pub xpd: Option<Xpd>,
    /// R's `plot`: `false` works out the layout (returned as usual) but
    /// draws nothing; `None`: `true`.
    pub plot: Option<bool>,
}

crate::setters::setters!(LegendOptions => [LegendCall] {
    bty: option LegendBty,
    bg: option ColorSpec,
    cex: option f64,
    xjust: option f64,
    yjust: option f64,
    x_intersp: option f64,
    y_intersp: option f64,
    seg_len: option f64,
    text_width: option f64,
    merge: option bool,
    ncol: option usize,
    horiz: value bool,
    title: option Label,
    title_cex: option f64,
    title_col: option ColorSpec,
    title_adj: option f64,
    title_font: option i32,
    inset: option_pair (f64, f64),
    text_col: values ColorSpec,
    text_font: values i32,
    box_lwd: option f64,
    box_lty: option LineType,
    box_col: option ColorSpec,
    border: option ColorSpec,
    xpd: option Xpd,
    plot: option bool,
});

/// Where `legend()` drew, as R's (invisible) return value `list(rect =
/// list(w, h, left, top), text = list(x, y))`. All in `usr` units, as in
/// R: log10 values on a log axis (see [`Device::usr`]).
#[derive(Clone, Debug, PartialEq)]
pub struct LegendLayout {
    /// `rect$left`, `rect$top`: the box's top-left corner.
    pub left: f64,
    pub top: f64,
    /// `rect$w`, `rect$h`: the box's width and height.
    pub w: f64,
    pub h: f64,
    /// `text$x`, `text$y`: each label's anchor (its left end and vertical
    /// centre).
    pub text_x: Vec<f64>,
    pub text_y: Vec<f64>,
}

impl<B: Backend> Device<B> {
    /// Draws R's `legend()`; the public entry is
    /// [`Graphics::legend`](crate::graphics::Graphics::legend), whose docs
    /// describe it.
    pub(crate) fn legend_impl(
        &mut self,
        pos: LegendPos,
        entries: &[LegendEntry],
        opts: &LegendOptions,
    ) -> Option<LegendLayout> {
        if entries.is_empty() {
            return None;
        }
        // R's `legend(xpd = )` sets `par(xpd)` for the call and restores it
        // `on.exit`.
        let mut dev = self.par_scope(|now, saved| now.xpd = saved.xpd);
        if let Some(xpd) = opts.xpd {
            dev.par.xpd = xpd;
        }
        Some(dev.draw_legend(pos, entries, opts))
    }

    fn draw_legend(
        &mut self,
        pos: LegendPos,
        entries: &[LegendEntry],
        opts: &LegendOptions,
    ) -> LegendLayout {
        let n = entries.len();
        let plt = self.plt;
        // R's defaults for the arguments left unset.
        let cex = opts.cex.unwrap_or(1.0);
        let x_intersp = opts.x_intersp.unwrap_or(1.0);
        let y_intersp = opts.y_intersp.unwrap_or(1.0);
        let seg_len = opts.seg_len.unwrap_or(2.0);
        let inset = opts.inset.unwrap_or((0.0, 0.0));
        let border = opts
            .border
            .unwrap_or(ColorSpec::Color(crate::color::Color::BLACK));

        // Number of columns / rows ("legends per column").
        let (ncol, n_per_col) = if opts.horiz {
            (n, 1)
        } else {
            let ncol = opts.ncol.unwrap_or(1).max(1);
            (ncol, n.div_ceil(ncol))
        };

        // One character cell: `xyinch(par("cin"))` scaled by the effective
        // cex. For R's pdf device `cin = (0.9, 1.2) * startps / 72`.
        let cex_eff = cex * self.par.cex;
        let cin_x = 0.9 * self.start_ps / 72.0;
        let cin_y = 1.2 * self.start_ps / 72.0;
        let xc = cex_eff * cin_x;
        let yc = cex_eff * cin_y;

        // `text.font` recycled over the entries, as R's `mapply()`; the
        // title's font defaults to the first.
        let fonts: Vec<i32> = (0..n)
            .map(|i| match opts.text_font.len() {
                0 => self.par.font,
                l => opts.text_font[i % l],
            })
            .collect();
        let title_font = opts.title_font.unwrap_or(fonts[0]);
        let (ux0, ux1, _, _) = self.usr;
        let inches_per_user_x = plt.width() / (ux1 - ux0).abs();
        let text_w = match opts.text_width {
            Some(tw) => tw.abs() * inches_per_user_x,
            None => entries
                .iter()
                .zip(&fonts)
                .map(|(e, &f)| self.label_width(&e.label, f, cex))
                .fold(0.0, f64::max),
        };

        // Row heights: at least a character cell, or the tallest label
        // (an expression can be taller).
        let yextra = yc * (y_intersp - 1.0);
        let ymax = entries
            .iter()
            .zip(&fonts)
            .map(|(e, &f)| self.label_height(&e.label, f, cex))
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
            Some(t) => title_unit_h.max(self.label_height(t, title_font, tcex)),
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

                w0 = text_w + (x_intersp + 1.0) * xc;
                if has_fill {
                    w0 += dx_fill;
                }
                if do_lines {
                    w0 += (seg_len + x_off) * xc;
                }
                let mut w_ = ncol as f64 * w0 + 0.5 * xc;
                let mut xextra_ = 0.0;
                if let Some(t) = title {
                    let tw =
                        self.label_width(t, title_font, tcex) + 0.5 * tcex * self.par.cex * cin_x;
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
                        let insetx = inset.0 * plt.width();
                        let insety = inset.1 * plt.height();
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
        // the `rect()`/`segments()`/`points()` calls R makes. With `plot =
        // FALSE` only the positions are worked out.
        let plot = opts.plot.unwrap_or(true);
        if plot {
            self.push_clip_for(self.par.xpd);
        }
        if plot && opts.bty.unwrap_or_default() == LegendBty::Box {
            let bg = opts.bg.unwrap_or(ColorSpec::Color(self.par.bg));
            let fill = self.resolve(bg).filter(|c| c.a > 0);
            let box_col = opts.box_col.unwrap_or(ColorSpec::Color(self.par.fg));
            let stroke = self.resolve(box_col).map(|c| {
                self.stroke_of(
                    c,
                    opts.box_lwd.unwrap_or(self.par.lwd),
                    opts.box_lty.unwrap_or(self.par.lty),
                )
            });
            self.backend.draw_rect(
                Rect::new(left, top - h, left + w, top),
                fill,
                stroke.as_ref(),
            );
        }

        let topspace = 0.5 * ymax + title_extra_h;
        let mut xt: Vec<f64> = (0..n)
            .map(|i| left + xc + xextra + (i / n_per_col) as f64 * w0)
            .collect();
        let yt: Vec<f64> = (0..n)
            .map(|i| top - topspace - ychar * ((i % n_per_col) as f64 + 0.5))
            .collect();

        if has_fill {
            let border = self.resolve(border);
            let stroke = border.map(|c| self.stroke_of(c, self.par.lwd, self.par.lty));
            for i in 0..n {
                let fill = entries[i].fill.and_then(|f| self.resolve(f));
                if !plot {
                    xt[i] += dx_fill;
                    continue;
                }
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
                let draw = plot && e.has_line();
                if let (true, Some(col), Some(lty)) = (draw, self.resolve(e.col), e.lty) {
                    let stroke = self.stroke_of(col, e.lwd.unwrap_or(self.par.lwd), lty);
                    let x0 = xt[i] + x_off * xc;
                    self.backend.draw_line(
                        Point::new(x0, yt[i]),
                        Point::new(x0 + seg_len * xc, yt[i]),
                        &stroke,
                    );
                }
                xt[i] += (seg_len + x_off) * xc;
            }
        }

        if plot && has_pch {
            for i in 0..n {
                let e = &entries[i];
                if !e.has_pch() {
                    continue;
                }
                let Some(col) = self.resolve(e.col) else {
                    continue;
                };
                let x1 = if merge && do_lines {
                    xt[i] - (seg_len / 2.0) * xc
                } else {
                    xt[i]
                };
                let pt_cex = e.pt_cex.unwrap_or(cex);
                let bg = self.resolve(e.pt_bg.unwrap_or(ColorSpec::None));
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
        if plot {
            self.pop_clip();
        }

        // `text.col` recycled over the entries; the title's colour defaults
        // to the first.
        let text_col = |dev: &Self, i: usize| match opts.text_col.len() {
            0 => ColorSpec::Color(dev.par.col),
            l => opts.text_col[i % l],
        };
        // `text(adj = c(title.adj, 0))` and `text(adj = c(0, 0.5))`.
        let clip = self.par.xpd;
        if let (true, Some(t)) = (plot, title) {
            let title_adj = opts.title_adj.unwrap_or(0.5);
            let anchor = Point::new(left + w * title_adj, top - ymaxtitle);
            self.draw_label(
                clip,
                anchor,
                t,
                title_adj,
                Some(0.0),
                self.par.srt,
                opts.title_col.unwrap_or(text_col(self, 0)),
                tcex,
                title_font,
            );
        }
        let mut text_x = Vec::with_capacity(n);
        let mut text_y = Vec::with_capacity(n);
        for i in 0..n {
            let anchor = Point::new(xt[i] + x_intersp * xc, yt[i]);
            let (tx, ty) = self.inches_to_usr(anchor);
            text_x.push(tx);
            text_y.push(ty);
            if !plot {
                continue;
            }
            let label = &entries[i].label;
            self.draw_label(
                clip,
                anchor,
                label,
                0.0,
                Some(0.5),
                self.par.srt,
                text_col(self, i),
                cex,
                fonts[i],
            );
        }

        let (left_u, top_u) = self.inches_to_usr(Point::new(left, top));
        let (right_u, bottom_u) = self.inches_to_usr(Point::new(left + w, top - h));
        LegendLayout {
            left: left_u,
            top: top_u,
            w: right_u - left_u,
            h: top_u - bottom_u,
            text_x,
            text_y,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positions_convert_like_r() {
        let p = |p: LegendPos| p;
        assert_eq!(
            p("topright".into()),
            LegendPos::Keyword(LegendKeyword::TopRight)
        );
        assert_eq!(
            p("topr".into()),
            LegendPos::Keyword(LegendKeyword::TopRight)
        );
        assert_eq!(
            p(String::from("center").into()),
            LegendPos::Keyword(LegendKeyword::Center)
        );
        assert_eq!(
            p(LegendKeyword::Left.into()),
            LegendPos::Keyword(LegendKeyword::Left)
        );
        assert_eq!(p((5.0, 20.0).into()), LegendPos::Xy(5.0, 20.0));
        assert_eq!(
            p(((5.0, 15.0), (10.0, 20.0)).into()),
            LegendPos::Corners(5.0, 10.0, 15.0, 20.0)
        );
    }

    #[test]
    #[should_panic(expected = "\"middle\" is not a legend position")]
    fn unknown_keyword_panics() {
        let _ = LegendPos::from("middle");
    }

    #[test]
    #[should_panic(expected = "\"to\" is not a legend position")]
    fn ambiguous_prefix_panics() {
        let _ = LegendPos::from("to");
    }

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
