//! Drawing plotmath expressions: a port of `r-source/src/main/plotmath.c`
//! for the constructs [`crate::plotmath`] accepts. Each element is laid
//! out twice, as in R: once to measure its bounding box, once to draw.
//! Boxes and positions are in inches; text is drawn baseline-left in
//! pieces (whole strings, or one character at a time for names, as R
//! does), with sizes from the TeX-like styles (`STYLE_D` ... `STYLE_SS1`).

use crate::afm::{self, FontFace};
use crate::backend::{Backend, TextStyle};
use crate::color::{Color, ColorSpec};
use crate::geometry::Point;
use crate::lty::LineType;
use crate::plotmath::{Expr, Expression, Label};

use super::{Clip, Device};

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

/// TeX styles, in R's order (`STYLE_SS1 = 1` ... `STYLE_D = 8`). The
/// cramped text and display styles (`T1`, `D1`) only arise in fractions,
/// which aren't supported.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Style {
    SS1 = 1,
    SS = 2,
    S1 = 3,
    S = 4,
    T = 6,
    D = 8,
}

const ITALIC_FACTOR: f64 = 0.15;
const DELIM_SYMBOL_MAG: f64 = 1.25;
const PLAIN: i32 = 1;
const BOLD: i32 = 2;
const ITALIC: i32 = 3;
const BOLD_ITALIC: i32 = 4;
const SYMBOL: i32 = 5;

/// `mathContext` plus the parts of the graphics context plotmath changes
/// (`gc->cex`, `gc->fontface`).
struct Math<'d, B: Backend> {
    dev: &'d mut Device<B>,
    base_cex: f64,
    style: Style,
    cex: f64,
    font: i32,
    reference: Point,
    current: Point,
    angle: f64,
    cos: f64,
    sin: f64,
    col: Color,
    clip: Clip,
}

impl<B: Backend> Device<B> {
    /// `GEExpressionMetric`: `(ascent, descent, width)` in inches of `expr`
    /// at total text expansion `cex` (`ps * cex` points).
    pub(super) fn expression_metrics(&mut self, expr: &Expression, cex: f64) -> (f64, f64, f64) {
        let mut m = Math::new(self, cex, Color::BLACK, Clip::Device);
        let bbox = m.element(&expr.root, false);
        (bbox.height, bbox.depth, bbox.width)
    }

    /// `strwidth(label, "inches", cex = cex, font = font)`, single-line
    /// text or an expression; `cex` is multiplied by `par(cex)`, as in
    /// [`Device::text`].
    pub(super) fn label_width(&mut self, label: &Label, font: i32, cex: f64) -> f64 {
        match label {
            Label::Text(t) => {
                let face = self.par.font_face(font);
                afm::str_width_inches(t, face, self.font_size_pt(cex), 1.0)
            }
            Label::Math(e) => self.expression_metrics(e, self.par.cex * cex).2,
        }
    }

    /// `strheight(label, "inches", cex = cex, font = font)`: the height of
    /// "M" for single-line text, height + depth for an expression.
    pub(super) fn label_height(&mut self, label: &Label, font: i32, cex: f64) -> f64 {
        match label {
            Label::Text(_) => {
                let face = self.par.font_face(font);
                afm::m_height_inches(face, self.font_size_pt(cex), 1.0)
            }
            Label::Math(e) => {
                let (h, d, _) = self.expression_metrics(e, self.par.cex * cex);
                h + d
            }
        }
    }

    /// Draws text (`GEText`) or an expression (`GEMathText`, in the plain
    /// font) at `anchor`. `xc`/`yc` align text as in `text_at_clipped`
    /// and an expression's bounding box; `yc = None` is R's `NA`. `cex` is
    /// multiplied by `par(cex)`.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_label(
        &mut self,
        clip: Clip,
        anchor: Point,
        label: &Label,
        xc: f64,
        yc: Option<f64>,
        rotation_deg: f64,
        col: ColorSpec,
        cex: f64,
        font: i32,
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
        clip: Clip,
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
    fn new(dev: &'d mut Device<B>, cex: f64, col: Color, clip: Clip) -> Self {
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
        FontFace::from_r_font_code(self.font)
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

    fn set_style(&mut self, style: Style) {
        self.cex = self.base_cex
            * match style {
                Style::D | Style::T => 1.0,
                Style::S | Style::S1 => 0.7,
                Style::SS | Style::SS1 => 0.5,
            };
        self.style = style;
    }

    fn set_sup_style(&mut self, style: Style) {
        self.set_style(match style {
            Style::D | Style::T => Style::S,
            Style::S | Style::SS => Style::SS,
            Style::S1 | Style::SS1 => Style::SS1,
        });
    }

    fn set_sub_style(&mut self, style: Style) {
        self.set_style(match style {
            Style::D | Style::T => Style::S1,
            _ => Style::SS1,
        });
    }

    fn is_compact(style: Style) -> bool {
        matches!(style, Style::S1 | Style::SS1)
    }

    fn using_italics(&self) -> bool {
        self.font == ITALIC || self.font == BOLD_ITALIC
    }

    fn set_font(&mut self, font: i32) -> i32 {
        std::mem::replace(&mut self.font, font)
    }

    // --- drawing -------------------------------------------------------

    fn move_across(&mut self, dx: f64) {
        self.current.x += dx;
    }

    fn move_up(&mut self, dy: f64) {
        self.current.y += dy;
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
            result.width = afm::str_width_inches(s, self.face(), self.size_pt(), 1.0);
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
            ("(", 1) => {
                let bbox = self.delimiter(S_PARENLEFT, draw);
                let bbox = bbox.combine(self.element(&args[0], draw));
                let bbox = self.italic_corr(bbox, draw);
                bbox.combine(self.delimiter(S_PARENRIGHT, draw))
            }
            ("group", 3) => self.group(args, draw),
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

    fn with_font(&mut self, font: i32, e: &Expr, draw: bool) -> BBox {
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
        let code = |e: &Expr| -> char {
            let s = match e {
                Expr::Str(s) => s.as_str(),
                Expr::Name(n) => match n.as_str() {
                    "lfloor" => return '\u{EB}',
                    "rfloor" => return '\u{FB}',
                    "lceil" => return '\u{E9}',
                    "rceil" => return '\u{F9}',
                    "langle" => return '\u{E1}',
                    "rangle" => return '\u{F1}',
                    _ => panic!("invalid group delimiter `{n}`"),
                },
                _ => panic!("invalid group delimiter"),
            };
            match s {
                "|" | "||" => '|',
                "(" | ")" | "[" | "]" | "{" | "}" => s.chars().next().unwrap(),
                "" | "." => '.',
                _ => panic!("invalid group delimiter {s:?}"),
            }
        };
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
