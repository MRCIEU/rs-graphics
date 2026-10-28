// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Drawing plotmath expressions: a port of `r-source/src/main/plotmath.c`
//! for the constructs [`crate::plotmath`] accepts. Rules (fraction bars,
//! radicals, `bar`/`widehat`/`widetilde`, `underline`, `abs()` bars) are
//! drawn as lines, as R does. Each element is laid out twice, as in R:
//! once to measure its bounding box, once to draw.
//! Boxes and positions are in inches; text is drawn baseline-left in
//! pieces (whole strings, or one character at a time for names, as R
//! does), with sizes from the TeX-like styles (`STYLE_D` ... `STYLE_SS1`).

use crate::afm::{self, FontFace};
use crate::backend::{Backend, TextStyle};
use crate::color::{Color, ColorSpec};
use crate::geometry::Point;
use crate::lty::LineType;
use crate::plotmath::{Expr, Expression, Label};

use super::{Device, Xpd};

/// `BBOX`: height above and depth below the baseline, width, italic
/// correction, and whether the element was a simple atom.
#[derive(Clone, Copy, Debug, Default)]
struct BBox {
    height: f64,
    depth: f64,
    width: f64,
    italic: f64,
    simple: bool,
}

impl BBox {
    fn new(height: f64, depth: f64, width: f64) -> BBox {
        BBox {
            height,
            depth,
            width,
            ..BBox::default()
        }
    }

    /// `EnlargeBBox`.
    fn enlarge(mut self, height: f64, depth: f64, width: f64) -> BBox {
        self.height += height;
        self.depth += depth;
        self.width += width;
        self
    }

    /// `CenterShift`.
    fn center_shift(&self) -> f64 {
        0.5 * (self.height - self.depth)
    }

    /// `CombineOffsetBBoxes`: `b` placed at an offset over `self`.
    fn combine_offset(mut self, b: BBox, x: f64, y: f64) -> BBox {
        self.width = self.width.max(b.width + x);
        self.height = self.height.max(b.height + y);
        self.depth = self.depth.max(b.depth - y);
        self.italic = 0.0;
        self.simple = false;
        self
    }

    fn shift(mut self, v: f64) -> BBox {
        self.height += v;
        self.depth -= v;
        self
    }

    /// `CombineBBoxes`: side by side.
    fn combine(mut self, b: BBox) -> BBox {
        self.height = self.height.max(b.height);
        self.depth = self.depth.max(b.depth);
        self.width += b.width;
        self.italic = b.italic;
        self.simple = b.simple;
        self
    }

    /// `CombineAlignedBBoxes`: overlaid.
    fn combine_aligned(mut self, b: BBox) -> BBox {
        self.height = self.height.max(b.height);
        self.depth = self.depth.max(b.depth);
        self.width = self.width.max(b.width);
        self.italic = 0.0;
        self.simple = false;
        self
    }
}

/// TeX styles, in R's order (`STYLE_SS1 = 1` ... `STYLE_D = 8`); the
/// `1` styles are the cramped ones (subscripts, denominators, radicals).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Style {
    SS1 = 1,
    SS = 2,
    S1 = 3,
    S = 4,
    T1 = 5,
    T = 6,
    D1 = 7,
    D = 8,
}

const ITALIC_FACTOR: f64 = 0.15;
const DELIM_SYMBOL_MAG: f64 = 1.25;
const OPERATOR_SYMBOL_MAG: f64 = 1.25;
/// `RuleThickness()` (TeX's `xi8`), in inches.
const RULE_THICKNESS: f64 = 0.015;
const ACCENT_GAP: f64 = 0.2;
const HAT_HEIGHT: f64 = 0.3;
const NTILDE: usize = 8;
const TILDE_DELTA: f64 = 0.05;
const RADICAL_GAP: f64 = 0.4;
const RADICAL_SPACE: f64 = 0.2;
const PLAIN: FontFace = FontFace::Plain;
const BOLD: FontFace = FontFace::Bold;
const ITALIC: FontFace = FontFace::Italic;
const BOLD_ITALIC: FontFace = FontFace::BoldItalic;
const SYMBOL: FontFace = FontFace::Symbol;

/// `mathContext` plus the parts of the graphics context plotmath changes
/// (`gc->cex`, `gc->fontface`).
struct Math<'d, B: Backend> {
    dev: &'d mut Device<B>,
    base_cex: f64,
    style: Style,
    cex: f64,
    font: FontFace,
    reference: Point,
    current: Point,
    angle: f64,
    cos: f64,
    sin: f64,
    col: Color,
    clip: Xpd,
}

impl<B: Backend> Device<B> {
    /// `GEExpressionMetric`: `(ascent, descent, width)` in inches of `expr`
    /// at total text expansion `cex` (`ps * cex` points).
    pub(super) fn expression_metrics(&mut self, expr: &Expression, cex: f64) -> (f64, f64, f64) {
        let mut m = Math::new(self, cex, Color::BLACK, Xpd::Device);
        let bbox = m.element(&expr.root, false);
        (bbox.height, bbox.depth, bbox.width)
    }

    /// `strwidth(label, "inches", cex = cex, font = font)`, text (the
    /// widest line) or an expression; `cex` is multiplied by [`par(cex)`], as in
    /// [`Graphics::text`](crate::graphics::Graphics::text).
    ///
    /// [`par(cex)`]: crate::par::Par::cex
    pub(super) fn label_width(&mut self, label: &Label, font: FontFace, cex: f64) -> f64 {
        match label {
            Label::Text(t) => self.text_size_inches(t, font, cex).0,
            Label::Math(e) => self.expression_metrics(e, self.par.cex * cex).2,
        }
    }

    /// `strheight(label, "inches", cex = cex, font = font)`: for text the
    /// height of "M" plus a line height per newline, for an expression its
    /// height + depth.
    pub(super) fn label_height(&mut self, label: &Label, font: FontFace, cex: f64) -> f64 {
        match label {
            Label::Text(t) => self.text_size_inches(t, font, cex).1,
            Label::Math(e) => {
                let (h, d, _) = self.expression_metrics(e, self.par.cex * cex);
                h + d
            }
        }
    }

    /// Draws text (`GEText`) or an expression (`GEMathText`, in the plain
    /// font) at `anchor`. `xc`/`yc` align text as in `text_at_clipped`
    /// and an expression's bounding box; `yc = None` is R's `NA`. `cex` is
    /// multiplied by [`par(cex)`].
    ///
    /// [`par(cex)`]: crate::par::Par::cex
    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_label(
        &mut self,
        clip: Xpd,
        anchor: Point,
        label: &Label,
        xc: f64,
        yc: Option<f64>,
        rotation_deg: f64,
        col: ColorSpec,
        cex: f64,
        font: FontFace,
    ) {
        match label {
            Label::Text(t) => {
                self.text_at_clipped(clip, anchor, t, xc, yc, rotation_deg, col, cex, font)
            }
            Label::Math(e) => {
                if let Some(col) = self.resolve(col) {
                    let yc = yc.unwrap_or(f64::NAN);
                    let cex = self.par.cex * cex;
                    self.math_text(anchor, e, xc, yc, rotation_deg, col, cex, clip);
                }
            }
        }
    }

    /// `GEMathText`: draws `expr` with its bounding box aligned to
    /// `anchor` by `xc` (horizontal, fraction of the width) and `yc`
    /// (vertical, fraction of height + depth from the bottom); `NaN` is
    /// R's `NA` (centred). `cex` is the total text expansion (`ps * cex`
    /// points); the expression starts in the plain font.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn math_text(
        &mut self,
        anchor: Point,
        expr: &Expression,
        xc: f64,
        yc: f64,
        rotation_deg: f64,
        col: Color,
        cex: f64,
        clip: Xpd,
    ) {
        let mut m = Math::new(self, cex, col, clip);
        let bbox = m.element(&expr.root, false);
        m.reference = anchor;
        let xc = if xc.is_finite() { xc } else { 0.5 };
        let yc = if yc.is_finite() { yc } else { 0.5 };
        m.current = Point::new(
            anchor.x - xc * bbox.width,
            anchor.y + bbox.depth - yc * (bbox.height + bbox.depth),
        );
        m.angle = rotation_deg;
        let theta = rotation_deg.to_radians();
        (m.cos, m.sin) = (theta.cos(), theta.sin());
        m.element(&expr.root, true);
    }
}

/// Symbol-font codes of names that plotmath draws as a symbol
/// (`TranslatedSymbol`): the Greek letters and a few others.
fn translated_symbol(name: &str) -> Option<char> {
    const GREEK_UPPER: [&str; 26] = [
        "Alpha", "Beta", "Chi", "Delta", "Epsilon", "Phi", "Gamma", "Eta", "Iota", "theta1",
        "Kappa", "Lambda", "Mu", "Nu", "Omicron", "Pi", "Theta", "Rho", "Sigma", "Tau", "Upsilon",
        "sigma1", "Omega", "Xi", "Psi", "Zeta",
    ];
    const GREEK_LOWER: [&str; 26] = [
        "alpha", "beta", "chi", "delta", "epsilon", "phi", "gamma", "eta", "iota", "phi1", "kappa",
        "lambda", "mu", "nu", "omicron", "pi", "theta", "rho", "sigma", "tau", "upsilon", "omega1",
        "omega", "xi", "psi", "zeta",
    ];
    if let Some(i) = GREEK_UPPER.iter().position(|&g| g == name) {
        return char::from_u32(65 + i as u32);
    }
    if let Some(i) = GREEK_LOWER.iter().position(|&g| g == name) {
        return char::from_u32(97 + i as u32);
    }
    let code = match name {
        "vartheta" => 74,
        "varsigma" | "stigma" => 86,
        "varphi" => 106,
        "Upsilon1" => 161,
        "minute" => 162,
        "infinity" => 165,
        "degree" => 176,
        "second" => 178,
        "partialdiff" => 182,
        "aleph" => 192,
        "nabla" => 209,
        _ => return None,
    };
    char::from_u32(code)
}

/// `BinTable`: binary/unary operators drawn with a Symbol character.
fn bin_code(name: &str) -> Option<char> {
    let code = match name {
        "!" => 0o41,
        "*" => 0o52,
        "+" => 0o53,
        "-" => 0o55,
        "/" => 0o57,
        ":" => 0o72,
        "%+-%" => 0o261,
        "%*%" => 0o264,
        "%/%" => 0o270,
        "%intersection%" => 0o307,
        "%union%" => 0o310,
        "%.%" => 0o327,
        _ => return None,
    };
    char::from_u32(code)
}

/// `RelTable`: relations, arrows and set relations.
fn rel_code(name: &str) -> Option<char> {
    let code = match name {
        "<" => 60,
        "==" => 61,
        ">" => 62,
        "%=~%" => 64,
        "!=" => 185,
        "<=" => 163,
        ">=" => 179,
        "%==%" => 186,
        "%~~%" => 187,
        "%prop%" => 181,
        "%~%" => 126,
        "%<->%" => 171,
        "%<-%" => 172,
        "%up%" => 173,
        "%->%" => 174,
        "%down%" => 175,
        "%<=>%" => 219,
        "%<=%" => 220,
        "%dblup%" => 221,
        "%=>%" => 222,
        "%dbldown%" => 223,
        "%supset%" => 201,
        "%supseteq%" => 202,
        "%notsubset%" => 203,
        "%subset%" => 204,
        "%subseteq%" => 205,
        "%in%" => 206,
        "%notin%" => 207,
        _ => return None,
    };
    char::from_u32(code)
}

const S_SPACE: char = ' ';
const S_PARENLEFT: char = '(';
const S_PARENRIGHT: char = ')';
const S_ASTERISKMATH: char = '*';
const S_COMMA: char = ',';
const S_SLASH: char = '/';
const S_ELLIPSIS: char = '\u{BC}';
const A_HAT: char = '^';
const A_TILDE: char = '~';

impl<'d, B: Backend> Math<'d, B> {
    /// A context in display style and the plain font, as `GEMathText`
    /// starts.
    fn new(dev: &'d mut Device<B>, cex: f64, col: Color, clip: Xpd) -> Self {
        Math {
            dev,
            base_cex: cex,
            style: Style::D,
            cex,
            font: PLAIN,
            reference: Point::new(0.0, 0.0),
            current: Point::new(0.0, 0.0),
            angle: 0.0,
            cos: 1.0,
            sin: 0.0,
            col,
            clip,
        }
    }

    // --- fonts, sizes and metrics -------------------------------------

    fn face(&self) -> FontFace {
        self.font
    }

    /// The point size text is set at (rounded on PDF, as R's `pdf()`).
    fn size_pt(&self) -> f64 {
        self.dev
            .backend
            .effective_font_size(self.dev.par.ps * self.cex)
    }

    /// `GEMetricInfo` in inches: (ascent, descent, width).
    fn metric(&self, c: char) -> (f64, f64, f64) {
        afm::glyph_metric_inches(c, self.face(), self.size_pt())
    }

    fn x_height(&self) -> f64 {
        self.metric('x').0
    }

    fn cap_x_height(&self) -> f64 {
        self.metric('X').0
    }

    fn axis_height(&self) -> f64 {
        0.5 * self.metric('+').0
    }

    fn quad(&self) -> f64 {
        self.metric('M').2
    }

    fn thin_space(&self) -> f64 {
        self.quad() / 6.0
    }

    fn medium_space(&self) -> f64 {
        self.quad() * 2.0 / 9.0
    }

    fn thick_space(&self) -> f64 {
        self.quad() * 5.0 / 18.0
    }

    fn mu_space(&self) -> f64 {
        self.quad() / 18.0
    }

    /// `FigHeight`: the height of digits.
    fn fig_height(&self) -> f64 {
        self.metric('0').0
    }

    /// `DescDepth`: the depth of descenders.
    fn desc_depth(&self) -> f64 {
        self.metric('g').1
    }

    fn set_style(&mut self, style: Style) {
        self.cex = self.base_cex
            * match style {
                Style::D | Style::T | Style::D1 | Style::T1 => 1.0,
                Style::S | Style::S1 => 0.7,
                Style::SS | Style::SS1 => 0.5,
            };
        self.style = style;
    }

    fn set_prime_style(&mut self, style: Style) {
        self.set_style(match style {
            Style::D | Style::D1 => Style::D1,
            Style::T | Style::T1 => Style::T1,
            Style::S | Style::S1 => Style::S1,
            Style::SS | Style::SS1 => Style::SS1,
        });
    }

    fn set_sup_style(&mut self, style: Style) {
        self.set_style(match style {
            Style::D | Style::T => Style::S,
            Style::D1 | Style::T1 => Style::S1,
            Style::S | Style::SS => Style::SS,
            Style::S1 | Style::SS1 => Style::SS1,
        });
    }

    fn set_sub_style(&mut self, style: Style) {
        self.set_style(match style {
            Style::D | Style::T | Style::D1 | Style::T1 => Style::S1,
            _ => Style::SS1,
        });
    }

    fn set_num_style(&mut self, style: Style) {
        match style {
            Style::D => self.set_style(Style::T),
            Style::D1 => self.set_style(Style::T1),
            _ => self.set_sup_style(style),
        }
    }

    fn set_denom_style(&mut self, style: Style) {
        if style > Style::T {
            self.set_style(Style::T1);
        } else {
            self.set_sub_style(style);
        }
    }

    fn is_compact(style: Style) -> bool {
        matches!(style, Style::D1 | Style::T1 | Style::S1 | Style::SS1)
    }

    fn using_italics(&self) -> bool {
        self.font == ITALIC || self.font == BOLD_ITALIC
    }

    fn set_font(&mut self, font: FontFace) -> FontFace {
        std::mem::replace(&mut self.font, font)
    }

    // --- drawing -------------------------------------------------------

    fn move_across(&mut self, dx: f64) {
        self.current.x += dx;
    }

    fn move_up(&mut self, dy: f64) {
        self.current.y += dy;
    }

    fn move_to(&mut self, x: f64, y: f64) {
        self.current = Point::new(x, y);
    }

    /// `GEPolyline` with `lty` solid and `lwd` at most 1, as plotmath
    /// draws its rules (fraction bars, radicals, `bar`/`widehat`/
    /// `widetilde`, `underline`, `abs()`).
    fn polyline(&mut self, pts: &[Point]) {
        let lwd = self.dev.par.lwd.min(1.0);
        let stroke = self.dev.stroke_of(self.col, lwd, LineType::SOLID);
        self.dev.push_clip_region(self.clip);
        self.dev.backend.draw_polyline(pts, &stroke);
        self.dev.pop_clip();
    }

    /// `ConvertedX`/`ConvertedY`: the current point rotated about the
    /// reference point.
    fn converted(&self) -> Point {
        let (dx, dy) = (
            self.current.x - self.reference.x,
            self.current.y - self.reference.y,
        );
        Point::new(
            self.reference.x + dx * self.cos - dy * self.sin,
            self.reference.y + dy * self.cos + dx * self.sin,
        )
    }

    /// `GEText(x, y, str, hadj = 0, yc = 0, rot)` at the current point.
    fn text(&mut self, s: &str) {
        let size = self.size_pt();
        if size <= 0.0 || s.is_empty() {
            return;
        }
        let style = TextStyle {
            rotation_deg: self.angle,
            ..TextStyle::new(self.col, self.face(), size)
        };
        let origin = self.converted();
        self.dev.push_clip_region(self.clip);
        self.dev.backend.draw_text(origin, s, &style);
        self.dev.pop_clip();
    }

    fn glyph_bbox(&self, c: char) -> BBox {
        let (h, d, w) = self.metric(c);
        BBox {
            simple: true,
            ..BBox::new(h, d, w)
        }
    }

    fn italic_corr(&mut self, mut bbox: BBox, draw: bool) -> BBox {
        if bbox.italic > 0.0 {
            if draw {
                self.move_across(bbox.italic);
            }
            bbox.width += bbox.italic;
            bbox.italic = 0.0;
        }
        bbox
    }

    fn gap(&mut self, gap: f64, draw: bool) -> BBox {
        if draw {
            self.move_across(gap);
        }
        BBox::new(0.0, 0.0, gap)
    }

    /// `RenderSymbolChar`: one character of the Symbol font (plain for
    /// `^` and `~`).
    fn symbol_char(&mut self, c: char, draw: bool) -> BBox {
        let prev = self.set_font(if c == A_HAT || c == A_TILDE {
            PLAIN
        } else {
            SYMBOL
        });
        let bbox = self.glyph_bbox(c);
        if draw {
            self.text(&c.to_string());
            self.move_across(bbox.width);
        }
        self.set_font(prev);
        bbox
    }

    /// `RenderChar`: one character in the current font.
    fn char_(&mut self, c: char, draw: bool) -> BBox {
        let bbox = self.glyph_bbox(c);
        if draw {
            self.text(&c.to_string());
            self.move_across(bbox.width);
        }
        bbox
    }

    /// `RenderSymbolStr`: a name drawn one character at a time, digits in
    /// the plain font, with italic corrections between characters. (R's
    /// font switch flips back to the name's font on every second
    /// consecutive digit, a bug not copied: all digits are plain.)
    fn symbol_str(&mut self, s: &str, draw: bool) -> BBox {
        let prev = self.font;
        let mut result = BBox::default();
        let mut last_italic = 0.0;
        for c in s.chars() {
            if c.is_ascii_digit() && self.font != PLAIN {
                self.font = PLAIN;
            } else if !c.is_ascii_digit() && self.font != prev {
                self.font = prev;
            }
            let mut glyph = self.glyph_bbox(c);
            glyph.italic = if self.using_italics() {
                ITALIC_FACTOR * glyph.height
            } else {
                0.0
            };
            if draw {
                self.move_across(last_italic);
                self.text(&c.to_string());
                self.move_across(glyph.width);
            }
            result.width += last_italic;
            result = result.combine(glyph);
            last_italic = glyph.italic;
        }
        self.font = prev;
        result.simple = true;
        result
    }

    /// `RenderStr`: a whole string (numbers, strings, function names).
    fn str(&mut self, s: &str, draw: bool) -> BBox {
        let mut glyph = BBox::default();
        let mut result = BBox::default();
        let mut n = 0;
        for c in s.chars() {
            glyph = self.glyph_bbox(c);
            result = result.combine(glyph);
            n += 1;
        }
        if n > 1 {
            // Kerning makes the sum of the glyphs' widths wrong.
            result.width = afm::str_width_inches(s, self.face(), self.size_pt());
        }
        if draw {
            self.text(s);
            self.move_across(result.width);
        }
        result.italic = if self.using_italics() {
            ITALIC_FACTOR * glyph.height
        } else {
            0.0
        };
        result.simple = true;
        result
    }

    // --- elements --------------------------------------------------------

    /// `RenderElement`.
    fn element(&mut self, e: &Expr, draw: bool) -> BBox {
        match e {
            Expr::Call(..) => self.formula(e, draw),
            Expr::Name(n) => {
                if n == "cdots" || n == "ldots" {
                    self.dots(n, draw)
                } else if let Some(c) = translated_symbol(n) {
                    self.symbol_char(c, draw)
                } else {
                    self.symbol_str(n, draw)
                }
            }
            Expr::Num(s) => {
                let prev = self.set_font(PLAIN);
                let bbox = self.str(s, draw);
                self.set_font(prev);
                bbox
            }
            Expr::Str(s) => self.str(s, draw),
        }
    }

    /// `RenderOffsetElement`.
    fn offset_element(&mut self, e: &Expr, x: f64, y: f64, draw: bool) -> BBox {
        let saved = self.current;
        if draw {
            self.current.x += x;
            self.current.y += y;
        }
        let mut bbox = self.element(e, draw);
        bbox.width += x;
        bbox.height += y;
        bbox.depth -= y;
        self.current = saved;
        bbox
    }

    /// `RenderDots`.
    fn dots(&mut self, name: &str, draw: bool) -> BBox {
        let bbox = self.symbol_char(S_ELLIPSIS, false);
        if name == "cdots" {
            let shift = self.axis_height() - 0.5 * bbox.height;
            if draw {
                self.move_up(shift);
                self.symbol_char(S_ELLIPSIS, true);
                self.move_up(-shift);
            }
            bbox.shift(shift)
        } else {
            if draw {
                self.symbol_char(S_ELLIPSIS, true);
            }
            bbox
        }
    }

    /// `RenderFormula`: dispatch on the call's function.
    fn formula(&mut self, e: &Expr, draw: bool) -> BBox {
        let Some((name, args)) = e.call_name() else {
            return self.generic_call(e, draw);
        };
        match (name, args.len()) {
            ("~", 1 | 2) => self.space(args, draw),
            (n, 1 | 2) if bin_code(n).is_some() => self.bin(n, args, draw),
            ("^", 2) => self.sup(args, draw),
            ("[", 2) => self.sub(args, draw),
            ("widetilde", 1) => self.wide_tilde(&args[0], draw),
            ("widehat", 1) => self.wide_hat(&args[0], draw),
            ("bar", 1) => self.bar(&args[0], draw),
            ("hat" | "ring" | "tilde" | "dot", 1) => self.accent(name, &args[0], draw),
            ("frac" | "over", 2) => self.fraction(args, true, draw),
            ("underline", 1) => self.underline(&args[0], draw),
            ("atop", 2) => self.fraction(args, false, draw),
            ("(", 1) => {
                let bbox = self.delimiter(S_PARENLEFT, draw);
                let bbox = bbox.combine(self.element(&args[0], draw));
                let bbox = self.italic_corr(bbox, draw);
                bbox.combine(self.delimiter(S_PARENRIGHT, draw))
            }
            ("bgroup", 3) => self.bgroup(args, draw),
            ("group", 3) => self.group(args, draw),
            ("integral", 1..=3) => self.integral(args, draw),
            (n, 1..=3) if is_op(n) => self.op(n, args, draw),
            ("sqrt" | "root", 1 | 2) => self.radical(&args[0], args.get(1), draw),
            ("abs", 1) => self.abs(&args[0], draw),
            ("{", 0) => BBox::default(),
            ("{", 1) => self.element(&args[0], draw),
            (n, 2) if rel_code(n).is_some() => self.rel(n, args, draw),
            ("bold", 1) => self.with_font(BOLD, &args[0], draw),
            ("italic" | "math", 1) => self.with_font(ITALIC, &args[0], draw),
            ("plain", 1) => self.with_font(PLAIN, &args[0], draw),
            ("symbol", 1) => self.with_font(SYMBOL, &args[0], draw),
            ("bolditalic" | "boldmath", 1) => self.with_font(BOLD_ITALIC, &args[0], draw),
            ("displaystyle" | "textstyle" | "scriptstyle" | "scriptscriptstyle", 1) => {
                let prev = self.style;
                self.set_style(match name {
                    "displaystyle" => Style::D,
                    "textstyle" => Style::T,
                    "scriptstyle" => Style::S,
                    _ => Style::SS,
                });
                let bbox = self.element(&args[0], draw);
                self.set_style(prev);
                bbox
            }
            ("phantom" | "vphantom", 1) => {
                let mut bbox = self.element(&args[0], false);
                if name == "vphantom" {
                    bbox.width = 0.0;
                    bbox.italic = 0.0;
                } else {
                    self.gap(bbox.width, draw);
                }
                bbox
            }
            ("paste", _) => {
                let mut bbox = BBox::default();
                for (i, a) in args.iter().enumerate() {
                    bbox = bbox.combine(self.element(a, draw));
                    if i + 1 != args.len() {
                        bbox = self.italic_corr(bbox, draw);
                    }
                }
                bbox
            }
            ("list", _) => self.comma_list(args, draw),
            _ => self.generic_call(e, draw),
        }
    }

    fn with_font(&mut self, font: FontFace, e: &Expr, draw: bool) -> BBox {
        let prev = self.set_font(font);
        let bbox = self.element(e, draw);
        self.set_font(prev);
        bbox
    }

    /// `RenderSpace`: `~x` and `x ~ y`.
    fn space(&mut self, args: &[Expr], draw: bool) -> BBox {
        if args.len() == 1 {
            let op = self.symbol_char(S_SPACE, draw);
            op.combine(self.element(&args[0], draw))
        } else {
            let a = self.element(&args[0], draw);
            let op = self.symbol_char(S_SPACE, draw);
            let b = self.element(&args[1], draw);
            a.combine(op).combine(b)
        }
    }

    /// `RenderSlash`: a line, not a glyph.
    fn slash(&mut self, draw: bool) -> BBox {
        let depth = 0.5 * self.axis_height();
        let height = self.cap_x_height() + 0.5 * self.axis_height();
        let width = 0.5 * self.x_height();
        if draw {
            self.move_across(0.5 * width);
            self.move_up(-depth);
            let p0 = self.converted();
            self.move_across(width);
            self.move_up(depth + height);
            let p1 = self.converted();
            self.move_up(-height);
            let lwd = self.dev.par.lwd.min(1.0);
            let stroke = self.dev.stroke_of(self.col, lwd, LineType::SOLID);
            self.dev.push_clip_region(self.clip);
            self.dev.backend.draw_line(p0, p1, &stroke);
            self.dev.pop_clip();
            self.move_across(0.5 * width);
        }
        BBox::new(height, depth, 2.0 * width)
    }

    /// `RenderBin`.
    fn bin(&mut self, name: &str, args: &[Expr], draw: bool) -> BBox {
        let op = bin_code(name).unwrap();
        if args.len() == 2 {
            if op == S_ASTERISKMATH {
                let bbox = self.element(&args[0], draw);
                let bbox = self.italic_corr(bbox, draw);
                return bbox.combine(self.element(&args[1], draw));
            }
            if op == S_SLASH {
                let bbox = self.element(&args[0], draw);
                let bbox = self.italic_corr(bbox, draw);
                let bbox = bbox.combine(self.gap(0.0, draw));
                let bbox = bbox.combine(self.slash(draw));
                let bbox = bbox.combine(self.gap(0.0, draw));
                return bbox.combine(self.element(&args[1], draw));
            }
            let gap = if self.style > Style::S {
                self.medium_space()
            } else {
                0.0
            };
            let bbox = self.element(&args[0], draw);
            let bbox = self.italic_corr(bbox, draw);
            let bbox = bbox.combine(self.gap(gap, draw));
            let bbox = bbox.combine(self.symbol_char(op, draw));
            let bbox = bbox.combine(self.gap(gap, draw));
            bbox.combine(self.element(&args[1], draw))
        } else {
            let gap = if self.style > Style::S {
                self.thin_space()
            } else {
                0.0
            };
            let bbox = self.symbol_char(op, draw);
            let bbox = bbox.combine(self.gap(gap, draw));
            bbox.combine(self.element(&args[0], draw))
        }
    }

    /// `RenderRel`.
    fn rel(&mut self, name: &str, args: &[Expr], draw: bool) -> BBox {
        let op = rel_code(name).unwrap();
        let gap = if self.style > Style::S {
            self.thick_space()
        } else {
            0.0
        };
        let bbox = self.element(&args[0], draw);
        let bbox = self.italic_corr(bbox, draw);
        let bbox = bbox.combine(self.gap(gap, draw));
        let bbox = bbox.combine(self.symbol_char(op, draw));
        let bbox = bbox.combine(self.gap(gap, draw));
        bbox.combine(self.element(&args[1], draw))
    }

    /// `RenderSub`: `body[sub]`.
    fn sub(&mut self, args: &[Expr], draw: bool) -> BBox {
        let (body, sub) = (&args[0], &args[1]);
        let style = self.style;
        let saved = self.current;
        let body_bbox = self.element(body, draw);
        let body_bbox = self.italic_corr(body_bbox, draw);
        let mut v = if body_bbox.simple {
            0.0
        } else {
            body_bbox.depth + 0.05 * self.cap_x_height() // sigma19
        };
        let s16 = 0.35 * self.x_height(); // sigma16
        self.set_sub_style(style);
        let sub_bbox = self.element(sub, false);
        v = v.max(s16).max(sub_bbox.height - 0.8 * self.x_height());
        let sub_bbox = self.offset_element(sub, 0.0, -v, draw);
        let bbox = body_bbox.combine(sub_bbox);
        self.set_style(style);
        if draw {
            self.current = Point::new(saved.x + bbox.width, saved.y);
        }
        bbox
    }

    /// `RenderSup`: `body^sup`, including `body[sub]^sup`.
    fn sup(&mut self, args: &[Expr], draw: bool) -> BBox {
        let (mut body, sup) = (&args[0], &args[1]);
        let mut sub = None;
        if let Some(("[", b)) = body.call_name() {
            if b.len() == 2 {
                sub = Some(&b[1]);
                body = &b[0];
            }
        }
        let style = self.style;
        let saved = self.current;
        let body_bbox = self.element(body, draw);
        let delta = body_bbox.italic;
        let mut body_bbox = self.italic_corr(body_bbox, draw);
        let width = body_bbox.width;
        let (mut u, mut v) = if body_bbox.simple {
            (0.0, 0.0)
        } else {
            (
                body_bbox.height - 0.3861111 * self.cap_x_height(), // sigma18
                body_bbox.depth + 0.05 * self.cap_x_height(),       // sigma19
            )
        };
        let theta = 0.015; // xi8
        let s5 = self.x_height();
        let s17 = 0.45 * self.cap_x_height();
        let p = if style == Style::D {
            0.95 * self.x_height() // sigma13
        } else if Self::is_compact(style) {
            0.7 * self.x_height() // sigma15
        } else {
            0.825 * self.x_height() // sigma14
        };
        self.set_sup_style(style);
        let sup_bbox = self.element(sup, false);
        u = u.max(p).max(sup_bbox.depth + 0.25 * s5);
        if let Some(sub) = sub {
            self.set_sub_style(style);
            let sub_bbox = self.element(sub, false);
            v = v.max(s17);
            if (u - sup_bbox.depth) - (sub_bbox.height - v) < 4.0 * theta {
                let psi = 0.8 * s5 - (u - sup_bbox.depth);
                if psi > 0.0 {
                    u += psi;
                    v -= psi;
                }
            }
            if draw {
                self.current = saved;
            }
            let sub_bbox = self.offset_element(sub, width, -v, draw);
            if draw {
                self.current = saved;
            }
            self.set_sup_style(style);
            let sup_bbox = self.offset_element(sup, width + delta, u, draw);
            body_bbox = body_bbox
                .combine_aligned(sub_bbox)
                .combine_aligned(sup_bbox);
        } else {
            let sup_bbox = self.offset_element(sup, 0.0, u, draw);
            body_bbox = body_bbox.combine(sup_bbox);
        }
        if draw {
            self.current = Point::new(saved.x + body_bbox.width, saved.y);
        }
        self.set_style(style);
        body_bbox
    }

    /// `RenderDelimiter`: a Symbol delimiter at 1.25 times the size.
    fn delimiter(&mut self, c: char, draw: bool) -> BBox {
        let saved = self.cex;
        self.cex *= DELIM_SYMBOL_MAG;
        let bbox = self.symbol_char(c, draw);
        self.cex = saved;
        bbox
    }

    /// `group(left, body, right)` (`RenderGroup`); delimiters are strings
    /// such as `"("`, `"["`, `"|"`, or `""`/`"."` for none.
    fn group(&mut self, args: &[Expr], draw: bool) -> BBox {
        let code = delim_code;
        let saved = self.cex;
        let mut bbox = BBox::default();
        let left = code(&args[0]);
        self.cex = DELIM_SYMBOL_MAG * saved;
        if left != '.' {
            bbox = self.symbol_char(left, draw);
        }
        self.cex = saved;
        bbox = bbox.combine(self.element(&args[1], draw));
        bbox = self.italic_corr(bbox, draw);
        let right = code(&args[2]);
        self.cex = DELIM_SYMBOL_MAG * saved;
        if right != '.' {
            bbox = bbox.combine(self.symbol_char(right, draw));
        }
        self.cex = saved;
        bbox
    }

    /// `RenderWideTilde`: a wavy line over the body.
    fn wide_tilde(&mut self, body: &Expr, draw: bool) -> BBox {
        let saved = self.current;
        let bbox = self.element(body, draw);
        let height = bbox.height;
        let total = bbox.width + bbox.italic;
        let delta = total * (1.0 - 2.0 * TILDE_DELTA) / NTILDE as f64;
        let start = TILDE_DELTA * total;
        let gap = ACCENT_GAP * self.cap_x_height();
        let hat_height = 0.5 * HAT_HEIGHT * self.cap_x_height();
        let c = 2.0 * std::f64::consts::PI / NTILDE as f64;
        if draw {
            let (base_x, base_y) = (saved.x, saved.y + height + gap);
            self.move_to(base_x, base_y);
            let mut pts = vec![self.converted()];
            for i in 0..=NTILDE {
                let x = start + i as f64 * delta;
                let y = 0.5 * hat_height * ((c * i as f64).sin() + 1.0);
                self.move_to(base_x + x, base_y + y);
                pts.push(self.converted());
            }
            self.move_to(base_x + total, base_y + hat_height);
            pts.push(self.converted());
            self.polyline(&pts);
            self.move_to(saved.x + total, saved.y);
        }
        BBox::new(height + gap + hat_height, bbox.depth, total)
    }

    /// `RenderWideHat`: a wide `^` over the body.
    fn wide_hat(&mut self, body: &Expr, draw: bool) -> BBox {
        let saved = self.current;
        let bbox = self.element(body, draw);
        let gap = ACCENT_GAP * self.cap_x_height();
        let hat_height = HAT_HEIGHT * self.cap_x_height();
        let total = bbox.width + bbox.italic;
        if draw {
            self.move_to(saved.x, saved.y + bbox.height + gap);
            let p0 = self.converted();
            self.move_across(0.5 * total);
            self.move_up(hat_height);
            let p1 = self.converted();
            self.move_across(0.5 * total);
            self.move_up(-hat_height);
            let p2 = self.converted();
            self.polyline(&[p0, p1, p2]);
            self.move_to(saved.x + bbox.width, saved.y);
        }
        bbox.enlarge(gap + hat_height, 0.0, 0.0)
    }

    /// `RenderBar`: a line over the body.
    fn bar(&mut self, body: &Expr, draw: bool) -> BBox {
        let saved = self.current;
        let bbox = self.element(body, draw);
        let gap = ACCENT_GAP * self.cap_x_height();
        if draw {
            self.move_to(saved.x + bbox.italic, saved.y + bbox.height + gap);
            let p0 = self.converted();
            self.move_across(bbox.width);
            let p1 = self.converted();
            self.polyline(&[p0, p1]);
            self.move_to(saved.x + bbox.width, saved.y);
        }
        bbox.enlarge(gap, 0.0, 0.0)
    }

    /// `RenderAccent`: `hat`, `tilde` (characters of the current font),
    /// `ring` and `dot` (Symbol characters) centred over the body.
    fn accent(&mut self, name: &str, body: &Expr, draw: bool) -> BBox {
        let code = match name {
            "hat" => A_HAT,
            "ring" => '\u{B0}',
            "tilde" => A_TILDE,
            _ => '\u{D7}', // dot
        };
        let symbol = code != A_HAT && code != A_TILDE;
        let saved = self.current;
        let body_bbox = self.element(body, false);
        let italic = body_bbox.italic;
        let accent_bbox = if symbol {
            self.symbol_char(code, false)
        } else {
            self.char_(code, false)
        };
        let width = (body_bbox.width + body_bbox.italic).max(accent_bbox.width);
        let xoffset = 0.5 * (width - body_bbox.width);
        let mut bbox = self.gap(xoffset, draw);
        bbox = bbox.combine(self.element(body, draw));
        bbox = bbox.combine(self.gap(xoffset, draw));
        self.current = saved;
        let xoffset = 0.5 * (width - accent_bbox.width) + 0.9 * italic;
        let yoffset = bbox.height + accent_bbox.depth + 0.1 * self.cap_x_height();
        if draw {
            self.move_to(saved.x + xoffset, saved.y + yoffset);
            if symbol {
                self.symbol_char(code, true);
            } else {
                self.char_(code, true);
            }
        }
        let bbox = bbox.combine_offset(accent_bbox, xoffset, yoffset);
        if draw {
            self.move_to(saved.x + width, saved.y);
        }
        bbox
    }

    /// `RenderFraction`: `frac`/`over` (with a rule) and `atop`.
    fn fraction(&mut self, args: &[Expr], rule: bool, draw: bool) -> BBox {
        let (num, denom) = (&args[0], &args[1]);
        let saved = self.current;
        let style = self.style;
        self.set_num_style(style);
        let num_bbox = self.element(num, false);
        let num_bbox = self.italic_corr(num_bbox, false);
        self.set_denom_style(style);
        let denom_bbox = self.element(denom, false);
        let denom_bbox = self.italic_corr(denom_bbox, false);
        self.set_style(style);

        let width = num_bbox.width.max(denom_bbox.width);
        // `NumDenomHShift`.
        let (n_h, d_h) = if num_bbox.width > denom_bbox.width {
            (0.0, (num_bbox.width - denom_bbox.width) / 2.0)
        } else {
            ((denom_bbox.width - num_bbox.width) / 2.0, 0.0)
        };
        // `NumDenomVShift` (TeX rules 15a-15d).
        let a = self.axis_height();
        let theta = RULE_THICKNESS;
        let x = self.cap_x_height();
        let (mut u, mut v, phi) = if self.style > Style::T {
            (
                a + 3.51 * theta + 0.15 * x + 0.7 * self.desc_depth(), // sigma8
                -a + 3.51 * theta + 0.7 * self.fig_height() + 0.344444 * x, // sigma11
                3.0 * theta,
            )
        } else {
            (
                a + 1.51 * theta + 0.08333333 * x, // sigma9
                -a + 1.51 * theta + 0.7 * self.fig_height() + 0.08333333 * x, // sigma12
                theta,
            )
        };
        let delta = (u - num_bbox.depth) - (a + 0.5 * theta);
        if delta < phi {
            u += phi - delta;
        }
        let delta = (a + 0.5 * theta) - (denom_bbox.height - v);
        if delta < phi {
            v += phi - delta;
        }

        self.current = saved;
        self.set_num_style(style);
        let num_bbox = self.offset_element(num, n_h, u, draw);
        self.current = saved;
        self.set_denom_style(style);
        let denom_bbox = self.offset_element(denom, d_h, -v, draw);
        self.set_style(style);
        if draw {
            if rule {
                self.current = saved;
                self.move_up(self.axis_height());
                let p0 = self.converted();
                self.move_across(width);
                let p1 = self.converted();
                self.polyline(&[p0, p1]);
            }
            self.move_to(saved.x + width, saved.y);
        }
        num_bbox.combine_aligned(denom_bbox)
    }

    /// `RenderUnderline`.
    fn underline(&mut self, body: &Expr, draw: bool) -> BBox {
        let saved = self.current;
        let bbox = self.element(body, false);
        let width = self.italic_corr(bbox, false).width;
        self.current = saved;
        let bbox = self.element(body, draw);
        let adepth = 0.1 * self.cap_x_height();
        let depth = bbox.depth + adepth;
        if draw {
            self.current = saved;
            self.move_up(-depth);
            let p0 = self.converted();
            self.move_across(width);
            let p1 = self.converted();
            self.polyline(&[p0, p1]);
            self.move_to(saved.x + width, saved.y);
        }
        bbox.enlarge(0.0, adepth, 0.0)
    }

    /// `bgroup(left, body, right)` (`RenderBGroup`): delimiters grown to
    /// the body's height.
    fn bgroup(&mut self, args: &[Expr], draw: bool) -> BBox {
        let axis = self.axis_height();
        let extra = 0.2 * self.x_height();
        let (left, right) = (delim_code(&args[0]), delim_code(&args[2]));
        let bbox = self.element(&args[1], false);
        let dist = (bbox.height - axis).max(bbox.depth + axis);
        let bbox = self.delim(left, dist + extra, draw);
        let bbox = bbox.combine(self.element(&args[1], draw));
        let bbox = self.italic_corr(bbox, draw);
        bbox.combine(self.delim(right, dist + extra, draw))
    }

    /// `RenderDelim`: a delimiter `dist` above and below the axis, built
    /// from the Symbol font's top, bottom, extension (and middle) pieces.
    fn delim(&mut self, which: char, dist: f64, draw: bool) -> BBox {
        let saved = self.current;
        let prev = self.set_font(SYMBOL);
        let axis = self.axis_height();
        let codes =
            |t: u32, e: u32, b: u32, m: u32| [t, e, b, m].map(|c| char::from_u32(c).unwrap());
        let [top, ext, bot, mid] = match which {
            '.' => {
                self.set_font(prev);
                return BBox::default();
            }
            '|' => codes(239, 239, 239, 0),
            '(' => codes(230, 231, 232, 0),
            ')' => codes(246, 247, 248, 0),
            '[' => codes(233, 234, 235, 0),
            ']' => codes(249, 250, 251, 0),
            '{' => codes(236, 239, 238, 237),
            '}' => codes(252, 239, 254, 253),
            _ => unreachable!("bgroup() delimiters are checked by Expression::parse"),
        };
        let brace = which == '{' || which == '}';
        let top_bbox = self.glyph_bbox(top);
        let ext_bbox = self.glyph_bbox(ext);
        let bot_bbox = self.glyph_bbox(bot);
        let mut dist = dist;
        if brace {
            if 1.2 * (top_bbox.height + top_bbox.depth) > dist {
                dist = 1.2 * (top_bbox.height + bot_bbox.depth);
            }
        } else if 0.8 * (top_bbox.height + top_bbox.depth) > dist {
            dist = 0.8 * (top_bbox.height + top_bbox.depth);
        }
        let ext_height = ext_bbox.height + ext_bbox.depth;
        let top_shift = dist - top_bbox.height + axis;
        let bot_shift = dist - bot_bbox.depth - axis;
        let ext_shift = 0.5 * (ext_bbox.height - ext_bbox.depth);
        let top_bbox = top_bbox.shift(top_shift);
        let bot_bbox = bot_bbox.shift(-bot_shift);
        let mut ans = top_bbox.combine_aligned(bot_bbox);
        if brace {
            let mid_bbox = self.glyph_bbox(mid);
            let mid_shift = axis - 0.5 * (mid_bbox.height - mid_bbox.depth);
            ans = ans.combine_aligned(mid_bbox.shift(mid_shift));
            if draw {
                self.move_to(saved.x, saved.y + top_shift);
                self.symbol_char(top, true);
                self.move_to(saved.x, saved.y + mid_shift);
                self.symbol_char(mid, true);
                self.move_to(saved.x, saved.y - bot_shift);
                self.symbol_char(bot, true);
                self.move_to(saved.x + ans.width, saved.y);
            }
        } else if draw {
            self.move_to(saved.x, saved.y + top_shift);
            self.symbol_char(top, true);
            self.move_to(saved.x, saved.y - bot_shift);
            self.symbol_char(bot, true);
            // Extension pieces between them.
            let ytop = axis + dist - (top_bbox.height + top_bbox.depth);
            let ybot = axis - dist + (bot_bbox.height + bot_bbox.depth);
            let n = ((ytop - ybot) / (0.99 * ext_height)).ceil() as i64;
            if n > 0 {
                let delta = (ytop - ybot) / n as f64;
                for i in 0..n {
                    let y = saved.y + ybot + (i as f64 + 0.5) * delta - ext_shift;
                    self.move_to(saved.x, y);
                    self.symbol_char(ext, true);
                }
            }
            self.move_to(saved.x + ans.width, saved.y);
        }
        self.set_font(prev);
        ans
    }

    /// `RenderIntSymbol`: in display styles two pieces, else one.
    fn int_symbol(&mut self, draw: bool) -> BBox {
        let saved = self.current;
        if self.style > Style::T {
            let (top, bot) = ('\u{F3}', '\u{F5}');
            let b1 = self.symbol_char(top, false);
            let b2 = self.symbol_char(bot, false);
            let shift = self.axis_height() + 0.99 * b1.depth;
            self.move_up(shift);
            let b1 = self.symbol_char(top, draw).shift(shift);
            self.current = saved;
            let shift = self.axis_height() - 0.99 * b2.height;
            self.move_up(shift);
            let b2 = self.symbol_char(bot, draw).shift(shift);
            if draw {
                self.move_to(saved.x + b1.width.max(b2.width), saved.y);
            } else {
                self.current = saved;
            }
            b1.combine_aligned(b2)
        } else {
            self.symbol_char('\u{F2}', draw)
        }
    }

    /// `RenderInt`: `integral(body, lower, upper)`.
    fn integral(&mut self, args: &[Expr], draw: bool) -> BBox {
        let style = self.style;
        let saved = self.current;
        let mut op = self.int_symbol(draw);
        let width = op.width;
        self.current = saved;
        if let Some(lower) = args.get(1) {
            let hshift = 0.5 * width + self.thin_space();
            self.set_sub_style(style);
            let b = self.element(lower, false);
            let vshift = op.depth + b.center_shift();
            let b = self.offset_element(lower, hshift, -vshift, draw);
            op = op.combine_aligned(b);
            self.set_style(style);
            self.current = saved;
        }
        if let Some(upper) = args.get(2) {
            let hshift = width + self.thin_space();
            self.set_sup_style(style);
            let b = self.element(upper, false);
            let vshift = op.height - b.center_shift();
            let b = self.offset_element(upper, hshift, vshift, draw);
            op = op.combine_aligned(b);
            self.set_style(style);
            self.current = saved;
        }
        self.move_across(op.width);
        op.combine(self.element(&args[0], draw))
    }

    /// `RenderOpSymbol`: a large Symbol character (centred on the axis in
    /// display styles) for `sum`, `prod`, `union`, `intersect`; the name
    /// in the plain font for the others.
    fn op_symbol(&mut self, name: &str, draw: bool) -> BBox {
        let code = match name {
            "prod" => '\u{D5}',
            "sum" => '\u{E5}',
            "union" => '\u{C8}',
            "intersect" => '\u{C7}',
            _ => {
                let prev = self.set_font(PLAIN);
                let bbox = self.str(name, draw);
                self.set_font(prev);
                return bbox;
            }
        };
        if self.style > Style::T {
            let saved = self.cex;
            self.cex *= OPERATOR_SYMBOL_MAG;
            let mut bbox = self.symbol_char(code, false);
            let shift = 0.5 * (bbox.height - bbox.depth) - self.axis_height();
            if draw {
                self.move_up(-shift);
                bbox = self.symbol_char(code, true);
                self.move_up(shift);
            }
            self.cex = saved;
            bbox.shift(-shift)
        } else {
            self.symbol_char(code, draw)
        }
    }

    /// `RenderOp`: `sum(body, lower, upper)` and the other operators,
    /// with limits centred below and above.
    fn op(&mut self, name: &str, args: &[Expr], draw: bool) -> BBox {
        let saved = self.current;
        let style = self.style;
        let op_bbox = self.op_symbol(name, false);
        let mut width = op_bbox.width;
        let big_op_spacing = |m: &Self| 0.15 * m.cap_x_height(); // xi9..xi13
        let (mut lower_bbox, mut upper_bbox) = (BBox::default(), BBox::default());
        let (mut lvshift, mut uvshift) = (0.0, 0.0);
        if let Some(lower) = args.get(1) {
            self.set_sub_style(style);
            lower_bbox = self.element(lower, false);
            self.set_style(style);
            width = width.max(lower_bbox.width);
            let s = big_op_spacing(self);
            lvshift = op_bbox.depth + lower_bbox.height + s.max(s - lower_bbox.height);
        }
        if let Some(upper) = args.get(2) {
            self.set_sup_style(style);
            upper_bbox = self.element(upper, false);
            self.set_style(style);
            width = width.max(upper_bbox.width);
            let s = big_op_spacing(self);
            uvshift = op_bbox.height + upper_bbox.depth + s.max(s - upper_bbox.depth);
        }
        let hshift = 0.5 * (width - op_bbox.width);
        let mut bbox = self.gap(hshift, draw);
        bbox = bbox.combine(self.op_symbol(name, draw));
        self.current = saved;
        if let Some(lower) = args.get(1) {
            self.set_sub_style(style);
            let hshift = 0.5 * (width - lower_bbox.width);
            let b = self.offset_element(lower, hshift, -lvshift, draw);
            self.set_style(style);
            bbox = bbox.combine_aligned(b);
            self.current = saved;
        }
        if let Some(upper) = args.get(2) {
            self.set_sup_style(style);
            let hshift = 0.5 * (width - upper_bbox.width);
            let b = self.offset_element(upper, hshift, uvshift, draw);
            self.set_style(style);
            bbox = bbox.combine_aligned(b);
            self.current = saved;
        }
        let s = big_op_spacing(self);
        bbox = bbox.enlarge(s, s, 0.0);
        if draw {
            self.move_across(width);
        }
        bbox = bbox.combine(self.gap(self.thin_space(), draw));
        bbox.combine(self.element(&args[0], draw))
    }

    /// `RenderRadical`: `sqrt(body)` and `sqrt(body, order)`.
    fn radical(&mut self, body: &Expr, order: Option<&Expr>, draw: bool) -> BBox {
        let style = self.style;
        let saved = self.current;
        let rad_gap = RADICAL_GAP * self.x_height();
        let rad_space = RADICAL_SPACE * self.x_height();
        let rad_trail = self.mu_space();
        self.set_prime_style(style);
        let body_bbox = self.element(body, false);
        let body_bbox = self.italic_corr(body_bbox, false);
        let rad_width = 0.6 * self.cap_x_height();
        let rad_height = body_bbox.height + rad_gap;
        let twiddle = body_bbox.center_shift();

        let mut lead_width = rad_width;
        let mut order_bbox = BBox::default();
        if let Some(order) = order {
            // `RenderScript` goes one script style further.
            self.set_sup_style(style);
            let script = |m: &mut Self, draw: bool| {
                let s = m.style;
                m.set_sup_style(s);
                let b = m.element(order, draw);
                m.set_style(s);
                b
            };
            let b = script(self, false);
            lead_width = lead_width.max(b.width + 0.4 * rad_width);
            let hshift = lead_width - b.width - 0.4 * rad_width;
            let mut vshift = rad_height - b.height;
            if vshift - b.depth < twiddle + rad_gap {
                vshift = twiddle + b.depth + rad_gap;
            }
            let mut b = b;
            if draw {
                self.move_to(saved.x + hshift, saved.y + vshift);
                b = script(self, true);
            }
            order_bbox = b.enlarge(vshift, 0.0, hshift);
        }
        if draw {
            self.move_to(saved.x + lead_width - rad_width, saved.y);
            self.move_up(0.8 * twiddle);
            let p0 = self.converted();
            self.move_up(0.2 * twiddle);
            self.move_across(0.3 * rad_width);
            let p1 = self.converted();
            self.move_up(-(twiddle + body_bbox.depth));
            self.move_across(0.3 * rad_width);
            let p2 = self.converted();
            self.move_up(body_bbox.depth + body_bbox.height + rad_gap);
            self.move_across(0.4 * rad_width);
            let p3 = self.converted();
            self.move_across(rad_space + body_bbox.width + rad_trail);
            let p4 = self.converted();
            self.polyline(&[p0, p1, p2, p3, p4]);
            self.current = saved;
        }
        order_bbox = order_bbox.combine_aligned(self.gap(lead_width + rad_space, draw));
        self.set_prime_style(style);
        order_bbox = order_bbox.combine(self.element(body, draw));
        order_bbox = order_bbox.combine(self.gap(2.0 * rad_trail, draw));
        order_bbox = order_bbox.enlarge(rad_gap, 0.0, 0.0);
        self.set_style(style);
        order_bbox
    }

    /// `RenderAbs`: vertical bars either side of the body.
    fn abs(&mut self, body: &Expr, draw: bool) -> BBox {
        let b = self.element(body, false);
        let (height, depth) = (b.height, b.depth);
        let bar = |m: &mut Self| {
            m.move_up(-depth);
            let p0 = m.converted();
            m.move_up(depth + height);
            let p1 = m.converted();
            m.polyline(&[p0, p1]);
            m.move_up(-height);
        };
        let mut bbox = self.gap(self.mu_space(), draw);
        if draw {
            bar(self);
        }
        bbox = bbox.combine(self.gap(self.mu_space(), draw));
        bbox = bbox.combine(self.element(body, draw));
        bbox = self.italic_corr(bbox, draw);
        bbox = bbox.combine(self.gap(self.mu_space(), draw));
        if draw {
            bar(self);
        }
        bbox.combine(self.gap(self.mu_space(), draw))
    }

    /// `RenderCommaList`.
    fn comma_list(&mut self, args: &[Expr], draw: bool) -> BBox {
        let mut bbox = BBox::default();
        for (i, a) in args.iter().enumerate() {
            if i > 0 {
                bbox = bbox.combine(self.symbol_char(S_COMMA, draw));
                bbox = bbox.combine(self.symbol_char(S_SPACE, draw));
            }
            bbox = bbox.combine(self.element(a, draw));
        }
        bbox
    }

    /// `RenderExpression`: any other call, as `f(x, y)`.
    fn generic_call(&mut self, e: &Expr, draw: bool) -> BBox {
        let Expr::Call(f, args) = e else {
            unreachable!()
        };
        let bbox = match f.as_ref() {
            // `RenderSymbolString`: the name as one string unless it's a
            // translated symbol.
            Expr::Name(n) => match translated_symbol(n) {
                Some(c) => self.symbol_char(c, draw),
                None => self.str(n, draw),
            },
            other => self.element(other, draw),
        };
        let bbox = self.italic_corr(bbox, draw);
        let bbox = bbox.combine(self.delimiter(S_PARENLEFT, draw));
        let bbox = bbox.combine(self.comma_list(args, draw));
        let bbox = self.italic_corr(bbox, draw);
        bbox.combine(self.delimiter(S_PARENRIGHT, draw))
    }
}

/// `OpTable`: the names `RenderOp` draws as operators with limits.
fn is_op(name: &str) -> bool {
    matches!(
        name,
        "prod"
            | "sum"
            | "union"
            | "intersect"
            | "lim"
            | "liminf"
            | "limsup"
            | "inf"
            | "sup"
            | "min"
            | "max"
    )
}

/// A delimiter `Expression::parse` has checked (see
/// [`crate::plotmath::delim_code`]).
fn delim_code(e: &Expr) -> char {
    crate::plotmath::delim_code(e).expect("checked by Expression::parse")
}
