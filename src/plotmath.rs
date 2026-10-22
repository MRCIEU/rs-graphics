// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Mathematical annotation (R's plotmath, `?plotmath`): expressions
//! written in R syntax, such as `"10^5"`, `"alpha[i]"` or
//! `"-log[10](italic(p))"`, parsed into the call tree R's parser would
//! build. [`Label`] is a text or expression label; `axis()`, `title()`,
//! `mtext()`, `text()` and `legend()` draw expression labels with the
//! layout rules of `plotmath.c` (`src/device/plotmath.rs`).
//!
//! Supported: numbers, names (Greek letters such as `alpha`/`Omega` and
//! `infinity`, `degree`, `minute`, `second`, `partialdiff`, `nabla`,
//! `aleph` from the Symbol font; other names as text), strings,
//! superscripts `x^y` and subscripts `x[i]` (also `x[i]^2`), unary and
//! binary `+ - * / :`, `!`, `%+-%`, `%*%`, `%/%`, `%.%`, `%intersection%`,
//! `%union%`, relations (`==`, `!=`, `<`, `<=`, `%~~%`, arrows, set
//! relations, ...), spacing with `~`, juxtaposition with `*` and
//! `paste()`, `list()`, parentheses, `group()` and `bgroup()`, braces
//! `{}` (`{}^14*C`), fractions (`frac`, `over`, `atop`), `sqrt(x)` and
//! `sqrt(x, n)`, accents (`hat`, `tilde`, `dot`, `ring`, `bar`,
//! `widehat`, `widetilde`), `abs()`, `underline()`, big operators with
//! limits (`sum(x[i], i == 1, n)`, `prod`, `integral`, `union`,
//! `intersect`, `lim`, `liminf`, `limsup`, `min`, `max`, `inf`, `sup`),
//! `plain()`, `bold()`, `italic()`, `bolditalic()`, `symbol()`, the
//! `*style()` sizes, `phantom()`/`vphantom()`, `cdots`/`ldots`, and any
//! other function call `f(x, y)`, drawn as R does.
//!
//! These constructs must have the number of arguments R uses (`frac(a,
//! b)`, `hat(x)`, `sum(body, lower, upper)` with 1 to 3); others are a
//! parse error rather than drawn wrongly. Not supported: `;` inside `{}`,
//! and `TRUE`, `NULL`, `NA` and R's control flow.

use std::fmt;

/// A node of the parsed expression, as R's parser represents it: every
/// operator, subscript, parenthesis and function call is a call whose
/// function is usually a name (`x^2` is `` `^`(x, 2) ``, `x[i]` is
/// `` `[`(x, i) ``, `(x)` is `` `(`(x) ``).
#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    /// A number, as R prints it in plotmath (7 significant digits:
    /// `1e5` is `"1e+05"`).
    Num(String),
    /// A name (R symbol), e.g. `alpha` or `x`.
    Name(String),
    /// A string literal.
    Str(String),
    /// A call: function and arguments.
    Call(Box<Expr>, Vec<Expr>),
}

impl Expr {
    /// The function's name if this is a call of a name.
    pub(crate) fn call_name(&self) -> Option<(&str, &[Expr])> {
        match self {
            Expr::Call(f, args) => match f.as_ref() {
                Expr::Name(n) => Some((n.as_str(), args.as_slice())),
                _ => None,
            },
            _ => None,
        }
    }
}

/// A parsed plotmath expression (one element of R's `expression()`).
#[derive(Clone, Debug, PartialEq)]
pub struct Expression {
    pub(crate) root: Expr,
}

impl Expression {
    /// Parses R syntax, e.g. `"10^5"`, `"alpha[i]^2"`, `"x == 0.05"` or
    /// `"frac(1, sqrt(n))"`. Fails on a syntax error, a construct the
    /// [module docs](self) list as unsupported, or a construct with the
    /// wrong number of arguments (`frac(a)`).
    pub fn parse(source: &str) -> Result<Expression, ParseError> {
        let tokens = tokenize(source)?;
        let mut p = Parser { tokens, pos: 0 };
        let root = p.expr(0)?;
        if p.pos < p.tokens.len() {
            return Err(p.error("unexpected input"));
        }
        check_supported(&root)?;
        Ok(Expression { root })
    }

    /// The root of the call tree.
    pub fn root(&self) -> &Expr {
        &self.root
    }
}

impl std::str::FromStr for Expression {
    type Err = ParseError;

    /// As [`Expression::parse`].
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Expression::parse(s)
    }
}

/// Why [`Expression::parse`] failed.
#[derive(Clone, Debug, PartialEq)]
pub struct ParseError {
    pub message: String,
    /// Byte offset in the source; 0 when not known (e.g. for unsupported
    /// constructs).
    pub offset: usize,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} (at byte {})", self.message, self.offset)
    }
}

impl std::error::Error for ParseError {}

/// A label: plain text or a plotmath expression (R's character or
/// `expression()` labels).
#[derive(Clone, Debug, PartialEq)]
pub enum Label {
    Text(String),
    Math(Expression),
}

impl Label {
    /// An expression label from R syntax. Panics if `source` doesn't parse
    /// (see [`Expression::parse`] for a `Result`), as R errors on an
    /// invalid expression.
    pub fn math(source: &str) -> Label {
        match Expression::parse(source) {
            Ok(e) => Label::Math(e),
            Err(e) => panic!("invalid plotmath expression {source:?}: {e}"),
        }
    }
}

impl From<&str> for Label {
    fn from(s: &str) -> Self {
        Label::Text(s.to_string())
    }
}

impl From<String> for Label {
    fn from(s: String) -> Self {
        Label::Text(s)
    }
}

impl From<&&str> for Label {
    fn from(s: &&str) -> Self {
        Label::Text(s.to_string())
    }
}

impl From<&String> for Label {
    fn from(s: &String) -> Self {
        Label::Text(s.clone())
    }
}

impl From<Expression> for Label {
    fn from(e: Expression) -> Self {
        Label::Math(e)
    }
}

/// One label (`&str`, `String`, [`Label`], an [`Expression`]) or several
/// (a slice, array or `Vec` of them): the text of
/// [`mtext()`](crate::graphics::Graphics::mtext), recycled with its other
/// arguments, and the labels of
/// [`legend()`](crate::graphics::Graphics::legend), one per key.
pub trait Labels {
    /// The labels, in order.
    fn into_labels(self) -> Vec<Label>;
}

macro_rules! one_label {
    ($($t:ty),*) => {
        $(
            impl Labels for $t {
                fn into_labels(self) -> Vec<Label> {
                    vec![self.into()]
                }
            }
        )*
    };
}
one_label!(&str, &&str, String, &String, Label, Expression);

impl<L: Clone + Into<Label>> Labels for &[L] {
    fn into_labels(self) -> Vec<Label> {
        self.iter().cloned().map(Into::into).collect()
    }
}

impl<L: Clone + Into<Label>, const N: usize> Labels for [L; N] {
    fn into_labels(self) -> Vec<Label> {
        self.into_iter().map(Into::into).collect()
    }
}

impl<L: Clone + Into<Label>, const N: usize> Labels for &[L; N] {
    fn into_labels(self) -> Vec<Label> {
        self.iter().cloned().map(Into::into).collect()
    }
}

impl<L: Clone + Into<Label>> Labels for Vec<L> {
    fn into_labels(self) -> Vec<Label> {
        self.into_iter().map(Into::into).collect()
    }
}

impl<L: Clone + Into<Label>> Labels for &Vec<L> {
    fn into_labels(self) -> Vec<Label> {
        self.iter().cloned().map(Into::into).collect()
    }
}

// ---------------------------------------------------------------------
// Tokens
// ---------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Num(f64),
    Name(String),
    Str(String),
    /// An operator or punctuation, including `%op%`.
    Op(String),
}

fn tokenize(src: &str) -> Result<Vec<(Tok, usize)>, ParseError> {
    let err = |message: &str, offset| ParseError {
        message: message.to_string(),
        offset,
    };
    let chars: Vec<(usize, char)> = src.char_indices().collect();
    let mut out = Vec::new();
    let mut i = 0;
    let at = |i: usize| chars.get(i).map(|c| c.1);
    while i < chars.len() {
        let (off, c) = chars[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        let starts_number =
            c.is_ascii_digit() || (c == '.' && at(i + 1).is_some_and(|d| d.is_ascii_digit()));
        if starts_number {
            let start = i;
            while at(i).is_some_and(|d| d.is_ascii_digit() || d == '.') {
                i += 1;
            }
            if at(i).is_some_and(|e| e == 'e' || e == 'E') {
                let mut j = i + 1;
                if at(j).is_some_and(|s| s == '+' || s == '-') {
                    j += 1;
                }
                if at(j).is_some_and(|d| d.is_ascii_digit()) {
                    i = j;
                    while at(i).is_some_and(|d| d.is_ascii_digit()) {
                        i += 1;
                    }
                }
            }
            let text: String = chars[start..i].iter().map(|c| c.1).collect();
            let v: f64 = text.parse().map_err(|_| err("invalid number", off))?;
            if at(i) == Some('L') {
                i += 1;
            }
            out.push((Tok::Num(v), off));
        } else if c.is_alphabetic() || c == '.' {
            let start = i;
            while at(i).is_some_and(|d| d.is_alphanumeric() || d == '.' || d == '_') {
                i += 1;
            }
            let name: String = chars[start..i].iter().map(|c| c.1).collect();
            out.push((Tok::Name(name), off));
        } else if c == '`' {
            let start = i + 1;
            i += 1;
            while at(i).is_some_and(|d| d != '`') {
                i += 1;
            }
            if at(i).is_none() {
                return Err(err("unterminated backtick name", off));
            }
            let name: String = chars[start..i].iter().map(|c| c.1).collect();
            i += 1;
            out.push((Tok::Name(name), off));
        } else if c == '"' || c == '\'' {
            let (s, next) = string_literal(&chars, i)?;
            i = next;
            out.push((Tok::Str(s), off));
        } else if c == '%' {
            let start = i;
            i += 1;
            while at(i).is_some_and(|d| d != '%') {
                i += 1;
            }
            if at(i).is_none() {
                return Err(err("unterminated %operator%", off));
            }
            i += 1;
            let op: String = chars[start..i].iter().map(|c| c.1).collect();
            out.push((Tok::Op(op), off));
        } else {
            let two: String = chars[i..(i + 2).min(chars.len())]
                .iter()
                .map(|c| c.1)
                .collect();
            let op = match two.as_str() {
                "==" | "!=" | "<=" | ">=" | "&&" | "||" | "<-" | "**" | "[[" | "::" => two,
                _ => c.to_string(),
            };
            match op.as_str() {
                "+" | "-" | "*" | "/" | "^" | "~" | "!" | "(" | ")" | "[" | "]" | "{" | "}"
                | "," | "<" | ">" | "==" | "!=" | "<=" | ">=" | ":" | "&" | "|" | "&&" | "||"
                | "**" => {}
                _ => return Err(err(&format!("`{op}` is not supported in plotmath"), off)),
            }
            i += op.chars().count();
            // R reads `**` as `^`.
            let op = if op == "**" { "^".to_string() } else { op };
            out.push((Tok::Op(op), off));
        }
    }
    Ok(out)
}

/// A quoted string from `chars[i]`, with R's escapes; returns the string
/// and the index after the closing quote.
fn string_literal(chars: &[(usize, char)], i: usize) -> Result<(String, usize), ParseError> {
    let (off, quote) = chars[i];
    let err = |message: &str| ParseError {
        message: message.to_string(),
        offset: off,
    };
    let mut s = String::new();
    let mut j = i + 1;
    loop {
        let Some(&(_, c)) = chars.get(j) else {
            return Err(err("unterminated string"));
        };
        j += 1;
        if c == quote {
            return Ok((s, j));
        }
        if c != '\\' {
            s.push(c);
            continue;
        }
        let Some(&(_, e)) = chars.get(j) else {
            return Err(err("unterminated string"));
        };
        j += 1;
        let digits = |j: &mut usize, n: usize, radix: u32| -> u32 {
            let mut v = 0;
            for _ in 0..n {
                match chars.get(*j).and_then(|c| c.1.to_digit(radix)) {
                    Some(d) => {
                        v = v * radix + d;
                        *j += 1;
                    }
                    None => break,
                }
            }
            v
        };
        let c = match e {
            'n' => '\n',
            't' => '\t',
            'r' => '\r',
            '0'..='7' => {
                j -= 1;
                char::from_u32(digits(&mut j, 3, 8)).ok_or_else(|| err("invalid escape"))?
            }
            'x' => char::from_u32(digits(&mut j, 2, 16)).ok_or_else(|| err("invalid escape"))?,
            'u' | 'U' => {
                let braced = chars.get(j).map(|c| c.1) == Some('{');
                if braced {
                    j += 1;
                }
                let v = digits(&mut j, if e == 'u' { 4 } else { 8 }, 16);
                if braced {
                    if chars.get(j).map(|c| c.1) != Some('}') {
                        return Err(err("invalid \\u{...} escape"));
                    }
                    j += 1;
                }
                char::from_u32(v).ok_or_else(|| err("invalid escape"))?
            }
            other => other,
        };
        s.push(c);
    }
}

// ---------------------------------------------------------------------
// Parser: precedence climbing with R's operator precedence (gram.y)
// ---------------------------------------------------------------------

/// Binding powers, lowest first.
const P_TILDE: u8 = 10;
const P_OR: u8 = 20;
const P_AND: u8 = 30;
const P_NOT: u8 = 40;
const P_CMP: u8 = 50;
const P_ADD: u8 = 60;
const P_MUL: u8 = 70;
const P_SPECIAL: u8 = 80;
const P_COLON: u8 = 90;
const P_UNARY: u8 = 100;
const P_POW: u8 = 110;

/// `(left binding power, right binding power)` of a binary operator.
fn infix_power(op: &str) -> Option<(u8, u8)> {
    let left = |p: u8| Some((p, p + 1));
    match op {
        "~" => left(P_TILDE),
        "|" | "||" => left(P_OR),
        "&" | "&&" => left(P_AND),
        "==" | "!=" | "<" | ">" | "<=" | ">=" => left(P_CMP),
        "+" | "-" => left(P_ADD),
        "*" | "/" => left(P_MUL),
        ":" => left(P_COLON),
        "^" => Some((P_POW, P_POW)), // right-associative
        _ if op.starts_with('%') => left(P_SPECIAL),
        _ => None,
    }
}

struct Parser {
    tokens: Vec<(Tok, usize)>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.tokens.get(self.pos).map(|t| &t.0)
    }

    fn error(&self, message: &str) -> ParseError {
        let offset = self
            .tokens
            .get(self.pos)
            .map_or_else(|| self.tokens.last().map_or(0, |t| t.1 + 1), |t| t.1);
        ParseError {
            message: message.to_string(),
            offset,
        }
    }

    fn is_op(&self, op: &str) -> bool {
        matches!(self.peek(), Some(Tok::Op(o)) if o == op)
    }

    fn expect(&mut self, op: &str) -> Result<(), ParseError> {
        if self.is_op(op) {
            self.pos += 1;
            Ok(())
        } else {
            Err(self.error(&format!("expected `{op}`")))
        }
    }

    fn expr(&mut self, min_power: u8) -> Result<Expr, ParseError> {
        let mut lhs = self.prefix()?;
        while let Some(Tok::Op(op)) = self.peek() {
            let op = op.clone();
            let Some((left, right)) = infix_power(&op) else {
                break;
            };
            if left < min_power {
                break;
            }
            self.pos += 1;
            let rhs = self.expr(right)?;
            lhs = call(&op, vec![lhs, rhs]);
        }
        Ok(lhs)
    }

    /// A primary with any prefix operator, then its postfix calls and
    /// subscripts.
    fn prefix(&mut self) -> Result<Expr, ParseError> {
        let unary = match self.peek() {
            Some(Tok::Op(o)) if o == "-" || o == "+" => Some((o.clone(), P_UNARY)),
            Some(Tok::Op(o)) if o == "!" => Some((o.clone(), P_NOT)),
            Some(Tok::Op(o)) if o == "~" => Some((o.clone(), P_TILDE)),
            _ => None,
        };
        if let Some((op, power)) = unary {
            self.pos += 1;
            let arg = self.expr(power)?;
            return Ok(call(&op, vec![arg]));
        }
        let mut e = self.primary()?;
        loop {
            if self.is_op("(") {
                self.pos += 1;
                let args = self.args(")")?;
                e = Expr::Call(Box::new(e), args);
            } else if self.is_op("[") {
                self.pos += 1;
                let mut args = vec![e];
                args.extend(self.args("]")?);
                e = call("[", args);
            } else {
                break;
            }
        }
        Ok(e)
    }

    fn args(&mut self, close: &str) -> Result<Vec<Expr>, ParseError> {
        let mut args = Vec::new();
        if self.is_op(close) {
            self.pos += 1;
            return Ok(args);
        }
        loop {
            args.push(self.expr(0)?);
            if self.is_op(",") {
                self.pos += 1;
            } else {
                self.expect(close)?;
                return Ok(args);
            }
        }
    }

    fn primary(&mut self) -> Result<Expr, ParseError> {
        let Some((tok, _)) = self.tokens.get(self.pos).cloned() else {
            return Err(self.error("unexpected end of expression"));
        };
        self.pos += 1;
        match tok {
            Tok::Num(v) => Ok(Expr::Num(format_number(v))),
            Tok::Str(s) => Ok(Expr::Str(s)),
            Tok::Name(n) => match n.as_str() {
                "Inf" | "NaN" => Ok(Expr::Num(n)),
                "TRUE" | "FALSE" | "NULL" | "NA" | "T" | "F" | "if" | "else" | "for" | "while"
                | "repeat" | "function" | "break" | "next" => {
                    self.pos -= 1;
                    Err(self.error(&format!("`{n}` is not supported in plotmath")))
                }
                _ => Ok(Expr::Name(n)),
            },
            Tok::Op(o) if o == "(" => {
                let inner = self.expr(0)?;
                self.expect(")")?;
                Ok(call("(", vec![inner]))
            }
            // `{x}` groups without drawing anything; `{}` is empty (as in
            // `{}^14*C`).
            Tok::Op(o) if o == "{" => {
                if self.is_op("}") {
                    self.pos += 1;
                    return Ok(call("{", vec![]));
                }
                let inner = self.expr(0)?;
                self.expect("}")?;
                Ok(call("{", vec![inner]))
            }
            Tok::Op(o) => {
                self.pos -= 1;
                Err(self.error(&format!("unexpected `{o}`")))
            }
        }
    }
}

fn call(name: &str, args: Vec<Expr>) -> Expr {
    Expr::Call(Box::new(Expr::Name(name.to_string())), args)
}

/// A number as plotmath prints it: R's `formatReal()` with 7 significant
/// digits (`RenderNumber` after `PrintDefaults()`).
fn format_number(v: f64) -> String {
    crate::device::format_axis_labels(&[v]).remove(0)
}

/// The number of arguments the plotmath constructs that need a fixed
/// number take (R errors for some others, or ignores extra arguments;
/// here they are an error, so nothing is drawn wrongly).
const ARITY: &[(&str, usize, usize)] = &[
    ("hat", 1, 1),
    ("widehat", 1, 1),
    ("tilde", 1, 1),
    ("widetilde", 1, 1),
    ("bar", 1, 1),
    ("dot", 1, 1),
    ("ring", 1, 1),
    ("frac", 2, 2),
    ("over", 2, 2),
    ("atop", 2, 2),
    ("sqrt", 1, 2),
    ("root", 1, 2),
    ("abs", 1, 1),
    ("underline", 1, 1),
    ("group", 3, 3),
    ("bgroup", 3, 3),
    ("integral", 1, 3),
    ("sum", 1, 3),
    ("prod", 1, 3),
    ("union", 1, 3),
    ("intersect", 1, 3),
    ("lim", 1, 3),
    ("liminf", 1, 3),
    ("limsup", 1, 3),
    ("min", 1, 3),
    ("max", 1, 3),
    ("inf", 1, 3),
    ("sup", 1, 3),
];

fn check_supported(e: &Expr) -> Result<(), ParseError> {
    if let Expr::Call(f, args) = e {
        if let Expr::Name(n) = f.as_ref() {
            if let Some(&(_, lo, hi)) = ARITY.iter().find(|a| a.0 == n) {
                if !(lo..=hi).contains(&args.len()) {
                    let want = if lo == hi {
                        lo.to_string()
                    } else {
                        format!("{lo} to {hi}")
                    };
                    return Err(ParseError {
                        message: format!(
                            "plotmath `{n}()` takes {want} argument(s), not {}",
                            args.len()
                        ),
                        offset: 0,
                    });
                }
            }
        }
        check_supported(f)?;
        for a in args {
            check_supported(a)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The tree in R's `deparse()`-like prefix form, for compact asserts.
    fn show(e: &Expr) -> String {
        match e {
            Expr::Num(n) => n.clone(),
            Expr::Name(n) => n.clone(),
            Expr::Str(s) => format!("{s:?}"),
            Expr::Call(f, args) => {
                let args: Vec<String> = args.iter().map(show).collect();
                format!("{}({})", show(f), args.join(", "))
            }
        }
    }

    fn p(src: &str) -> String {
        show(&Expression::parse(src).unwrap().root)
    }

    #[test]
    fn parses_like_r() {
        assert_eq!(p("10^5"), "^(10, 5)");
        assert_eq!(p("x[i]^2"), "^([(x, i), 2)");
        assert_eq!(p("-log[10](italic(p))"), "-([(log, 10)(italic(p)))");
        assert_eq!(p("-a^b"), "-(^(a, b))");
        assert_eq!(p("a^-b^c"), "^(a, -(^(b, c)))");
        assert_eq!(p("a^b^c"), "^(a, ^(b, c))");
        assert_eq!(p("a + b * c"), "+(a, *(b, c))");
        assert_eq!(p("a %+-% b == c"), "==(%+-%(a, b), c)");
        assert_eq!(p("x ~ ~y"), "~(x, ~(y))");
        assert_eq!(p("(a + b) * c"), "*(((+(a, b)), c)");
        assert_eq!(p("paste(\"P = \", 0.05)"), "paste(\"P = \", 0.05)");
        assert_eq!(p("a**2"), "^(a, 2)");
        assert_eq!(p("x[i, j]"), "[(x, i, j)");
        assert_eq!(p("1e5 + 0.1234567891 + 5L"), "+(+(1e+05, 0.1234568), 5)");
        assert_eq!(p("'a\\tb' * `my name`"), "*(\"a\\tb\", my name)");
        assert_eq!(p("{}^14*C"), "*(^({(), 14), C)");
        assert_eq!(p("{a + b}^2"), "^({(+(a, b)), 2)");
        assert_eq!(p("frac(1, sqrt(x, 3))"), "frac(1, sqrt(x, 3))");
    }

    #[test]
    fn rejects_bad_or_unsupported_input() {
        for bad in [
            "",
            "a +",
            "f(a",
            "a b",
            "x <- 1",
            "TRUE",
            "sqrt(x, 2, 3)",
            "hat(beta, 1)",
            "frac(a)",
            "group(\"(\", x)",
            "sum()",
            "{x",
            "{x; y}",
            "'open",
        ] {
            assert!(Expression::parse(bad).is_err(), "{bad:?}");
        }
    }
}
