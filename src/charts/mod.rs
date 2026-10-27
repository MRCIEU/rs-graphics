// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Reference charts of the built-in values: plotting symbols, line types,
//! line ends and joins, fonts, colour names, the palette, and the regions
//! of a page.
//!
//! Each function draws its chart as a new plot on a device or in a
//! [`Figure`](crate::recording::Figure), so values can be chosen by sight;
//! each one's doc shows the chart. They use the crate's public calls only.
//!
//! These are what R users get from `example(points)` (`pchShow()`),
//! `example(par)` (`showLty()`), `demo(colors)` and the diagram of a page's
//! regions in R's documentation.
//!
//! # Examples
//!
//! ```no_run
//! # use rs_graphics::prelude::*;
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut dev = Device::svg("symbols.svg").width(8).height(4.5).open()?;
//! rs_graphics::charts::pch(&mut dev)?;
//! dev.close()?;
//! # Ok(())
//! # }
//! ```

use crate::color::{Color, ColorSpec, DEFAULT_PALETTE};
use crate::device::BoxWhich;
use crate::error::Error;
use crate::graphics::Graphics;
use crate::named_colors::NAMED_COLORS;

/// Starts a plot with user coordinates `xlim` by `ylim` and no axes.
fn blank<G: Graphics>(g: &mut G, xlim: (f64, f64), ylim: (f64, f64)) -> Result<(), Error> {
    g.try_plot_new()?;
    g.plot_window()
        .xlim(xlim.0, xlim.1)
        .ylim(ylim.0, ylim.1)
        .try_draw()
}

/// Draws the plotting symbols: `pch` 0 to 25 and some characters, each
/// in blue with a gold fill (which only 21 to 25 use), labelled with its
/// value.
///
/// Drawn best on a page about 8 x 4.5 inches.
///
#[doc = include_str!("pch.svg")]
///
/// This is R's `pchShow()` (from `example(points)`).
///
/// # Examples
///
/// ```
/// # use rs_graphics::prelude::*;
/// # let mut dev = Device::null().width(8).height(4.5).open().unwrap();
/// rs_graphics::charts::pch(&mut dev)?;
/// # Ok::<(), rs_graphics::Error>(())
/// ```
///
/// # Errors
///
/// If the page is too small for the default margins (R's "figure margins
/// too large").
pub fn pch<G: Graphics>(g: &mut G) -> Result<(), Error> {
    let chars = ['*', '.', 'o', 'O', '0', '+', '-', '|', '%', '#'];
    let mut x = Vec::new();
    let mut y = Vec::new();
    let mut codes = Vec::new();
    let mut labels = Vec::new();
    for i in 0..26 {
        x.push((i % 10) as f64 + 1.0);
        y.push(4.0 - (i / 10) as f64);
        codes.push(i);
        labels.push(i.to_string());
    }
    for (i, c) in chars.iter().enumerate() {
        x.push(i as f64 + 1.0);
        y.push(1.0);
        codes.push(*c as i32);
        labels.push(format!("'{c}'"));
    }
    blank(g, (0.5, 10.5), (0.5, 4.5))?;
    g.points()
        .x(&x)
        .y(&y)
        .pch(codes)
        .cex(2.5)
        .col("blue")
        .bg("gold")
        .try_draw()?;
    let below: Vec<f64> = y.iter().map(|v| v - 0.38).collect();
    g.text()
        .x(&x)
        .y(&below)
        .labels(labels)
        .cex(0.9)
        .col("grey30")
        .try_draw()?;
    g.title().main("Plotting symbols: .pch()").try_draw()
}

/// Draws the line types: R's named types (codes 0 to 6) and some
/// patterns given as hex digits, at line width 2.
///
/// A hex pattern alternates drawn and blank lengths, in units of the line
/// width: `"44"` is 4 on, 4 off.
///
/// Drawn best on a page about 7 x 5 inches.
///
#[doc = include_str!("line_types.svg")]
///
/// This is R's `showLty()` (from `example(par)`).
///
/// # Examples
///
/// ```
/// # use rs_graphics::prelude::*;
/// # let mut dev = Device::null().width(7).height(5).open().unwrap();
/// rs_graphics::charts::line_types(&mut dev)?;
/// # Ok::<(), rs_graphics::Error>(())
/// ```
///
/// # Errors
///
/// If the page is too small for the default margins (R's "figure margins
/// too large").
pub fn line_types<G: Graphics>(g: &mut G) -> Result<(), Error> {
    let types = [
        ("blank", "0 or \"blank\""),
        ("solid", "1 or \"solid\""),
        ("dashed", "2 or \"dashed\""),
        ("dotted", "3 or \"dotted\""),
        ("dotdash", "4 or \"dotdash\""),
        ("longdash", "5 or \"longdash\""),
        ("twodash", "6 or \"twodash\""),
        ("44", "\"44\""),
        ("13", "\"13\""),
        ("1343", "\"1343\""),
        ("73", "\"73\""),
        ("2262", "\"2262\""),
    ];
    let n = types.len() as f64;
    blank(g, (0.0, 1.0), (0.5, n + 0.5))?;
    let y: Vec<f64> = (0..types.len()).map(|i| n - i as f64).collect();
    let lty: Vec<&str> = types.iter().map(|t| t.0).collect();
    g.segments()
        .x0(0.4)
        .y0(&y)
        .x1(0.98)
        .lty(lty)
        .lwd(2)
        .try_draw()?;
    let labels: Vec<&str> = types.iter().map(|t| t.1).collect();
    g.text().x(0.36).y(&y).labels(labels).adj(1).try_draw()?;
    g.title().main("Line types: .lty()").try_draw()
}

/// Draws the line ends (`lend`) and joins (`ljoin`), on thick lines with
/// their ends marked by a thin white line.
///
/// Drawn best on a page about 7 x 4 inches.
///
#[doc = include_str!("line_ends.svg")]
///
/// This is R's `par(lend = , ljoin = )`.
///
/// # Examples
///
/// ```
/// # use rs_graphics::prelude::*;
/// # let mut dev = Device::null().width(7).height(4).open().unwrap();
/// rs_graphics::charts::line_ends(&mut dev)?;
/// # Ok::<(), rs_graphics::Error>(())
/// ```
///
/// # Errors
///
/// If the page is too small for the default margins (R's "figure margins
/// too large").
pub fn line_ends<G: Graphics>(g: &mut G) -> Result<(), Error> {
    blank(g, (0.0, 3.0), (0.0, 2.0))?;
    for (i, end) in ["round", "butt", "square"].into_iter().enumerate() {
        let x0 = i as f64 + 0.3;
        let (x, y) = ([x0, x0 + 0.4], [1.5, 1.5]);
        g.lines()
            .x(x)
            .y(y)
            .lwd(18)
            .col("grey60")
            .lend(end)
            .try_draw()?;
        g.lines().x(x).y(y).lwd(1).col("white").try_draw()?;
        g.text()
            .x(x0 + 0.2)
            .y(1.15)
            .labels(format!(".lend(\"{end}\")"))
            .try_draw()?;
    }
    for (i, join) in ["round", "mitre", "bevel"].into_iter().enumerate() {
        let x0 = i as f64 + 0.3;
        let (x, y) = ([x0, x0 + 0.2, x0 + 0.4], [0.35, 0.75, 0.35]);
        g.lines()
            .x(x)
            .y(y)
            .lwd(18)
            .col("grey60")
            .lend("butt")
            .ljoin(join)
            .try_draw()?;
        g.lines().x(x).y(y).lwd(1).col("white").try_draw()?;
        g.text()
            .x(x0 + 0.2)
            .y(0.15)
            .labels(format!(".ljoin(\"{join}\")"))
            .try_draw()?;
    }
    g.title().main("Line ends and joins").try_draw()
}

/// Draws the five font faces, with their number and name.
///
/// [`FontFace::Symbol`](crate::font::FontFace::Symbol) shows Greek
/// letters: its text is in Adobe Symbol encoding (`'a'` is alpha).
///
/// Drawn best on a page about 7 x 4 inches.
///
#[doc = include_str!("fonts.svg")]
///
/// This is R's `par(font = )`.
///
/// # Examples
///
/// ```
/// # use rs_graphics::prelude::*;
/// # let mut dev = Device::null().width(7).height(4).open().unwrap();
/// rs_graphics::charts::fonts(&mut dev)?;
/// # Ok::<(), rs_graphics::Error>(())
/// ```
///
/// # Errors
///
/// If the page is too small for the default margins (R's "figure margins
/// too large").
pub fn fonts<G: Graphics>(g: &mut G) -> Result<(), Error> {
    let faces = [
        (1, "1 or \"plain\""),
        (2, "2 or \"bold\""),
        (3, "3 or \"italic\""),
        (4, "4 or \"bold.italic\""),
        (5, "5 or \"symbol\""),
    ];
    blank(g, (0.0, 1.0), (0.5, 5.5))?;
    for (i, (code, name)) in faces.into_iter().enumerate() {
        let y = 5.0 - i as f64;
        g.text().x(0.36).y(y).labels(name).adj(1).try_draw()?;
        let sample = if code == 5 {
            "abgdpqs ABGDPQS"
        } else {
            "Plots in R and Rust 0123"
        };
        g.text()
            .x(0.4)
            .y(y)
            .labels(sample)
            .adj(0)
            .font(code)
            .cex(1.3)
            .try_draw()?;
    }
    g.title().main("Font faces: .font()").try_draw()
}

/// R's colour names, in R's order, all `657` of them (`"white"`,
/// `"aliceblue"`, ..., `"yellowgreen"`), including the numbered variants
/// (`"red1"` to `"red4"`) and greys (`"grey0"` to `"grey100"`, also spelled
/// `"gray"`).
///
/// Any of them, in any case and with or without spaces, is a colour
/// argument (`.col("steelblue")`).
///
/// This is R's `colors()`.
///
/// # Examples
///
/// ```
/// let names: Vec<&str> = rs_graphics::charts::color_names().collect();
/// assert_eq!(names.len(), 657);
/// assert!(names.contains(&"steelblue"));
/// ```
pub fn color_names() -> impl Iterator<Item = &'static str> {
    NAMED_COLORS.iter().map(|c| c.0)
}

/// Draws R's colour names without a number at the end (136 of them), each
/// as a swatch with its name.
///
/// The numbered variants (`"red1"` to `"red4"`, `"grey0"` to `"grey100"`)
/// and the `"gray"` spellings of the greys are listed by [`color_names`].
///
/// Drawn best on a page about 9 x 12 inches.
///
#[doc = include_str!("colors.svg")]
///
/// This is R's `demo(colors)`.
///
/// # Examples
///
/// ```
/// # use rs_graphics::prelude::*;
/// # let mut dev = Device::null().width(9).height(12).open().unwrap();
/// rs_graphics::charts::colors(&mut dev)?;
/// # Ok::<(), rs_graphics::Error>(())
/// ```
///
/// # Errors
///
/// If the page is too small for the default margins (R's "figure margins
/// too large").
pub fn colors<G: Graphics>(g: &mut G) -> Result<(), Error> {
    let names: Vec<&str> = color_names()
        .filter(|n| !n.ends_with(|c: char| c.is_ascii_digit()))
        .filter(|n| !n.contains("gray"))
        .collect();
    let ncol = 4;
    let nrow = names.len().div_ceil(ncol);
    blank(g, (0.0, ncol as f64), (0.0, nrow as f64))?;
    let (mut x0, mut y0, mut x1, mut y1, mut col) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let (mut tx, mut ty) = (Vec::new(), Vec::new());
    for (i, name) in names.iter().enumerate() {
        let (c, r) = ((i / nrow) as f64, (i % nrow) as f64);
        let top = nrow as f64 - r;
        x0.push(c + 0.02);
        x1.push(c + 0.22);
        y0.push(top - 0.9);
        y1.push(top - 0.1);
        col.push(ColorSpec::Color(
            Color::parse(name).expect("a named colour"),
        ));
        tx.push(c + 0.25);
        ty.push(top - 0.5);
    }
    g.rect()
        .xleft(&x0)
        .ybottom(&y0)
        .xright(&x1)
        .ytop(&y1)
        .col(col)
        .border("grey50")
        .lwd(0.5)
        .try_draw()?;
    g.text()
        .x(&tx)
        .y(&ty)
        .labels(names)
        .adj(0)
        .cex(0.75)
        .try_draw()?;
    g.title().main("Colour names: .col(\"name\")").try_draw()
}

/// Draws the default palette: the eight colours `.col(1)` to `.col(8)`
/// stand for, with their numbers and hex values.
///
/// Numbers above 8 start again at 1; 0 is the background colour.
///
/// Drawn best on a page about 7 x 3 inches.
///
#[doc = include_str!("palette.svg")]
///
/// This is R's `palette()` (R 4's default, `"R4"`).
///
/// # Examples
///
/// ```
/// # use rs_graphics::prelude::*;
/// # let mut dev = Device::null().width(7).height(3).open().unwrap();
/// rs_graphics::charts::palette(&mut dev)?;
/// # Ok::<(), rs_graphics::Error>(())
/// ```
///
/// # Errors
///
/// If the page is too small for the default margins (R's "figure margins
/// too large").
pub fn palette<G: Graphics>(g: &mut G) -> Result<(), Error> {
    let n = DEFAULT_PALETTE.len();
    blank(g, (0.0, n as f64), (0.0, 1.0))?;
    let x0: Vec<f64> = (0..n).map(|i| i as f64 + 0.1).collect();
    let x1: Vec<f64> = (0..n).map(|i| i as f64 + 0.9).collect();
    let col: Vec<i32> = (1..=n as i32).collect();
    g.rect()
        .xleft(&x0)
        .ybottom(0.35)
        .xright(&x1)
        .ytop(0.95)
        .col(col)
        .border("grey50")
        .try_draw()?;
    let mid: Vec<f64> = (0..n).map(|i| i as f64 + 0.5).collect();
    let numbers: Vec<String> = (1..=n).map(|i| format!(".col({i})")).collect();
    let hex: Vec<String> = DEFAULT_PALETTE.iter().map(|c| c.to_hex()).collect();
    g.text().x(&mid).y(0.22).labels(numbers).try_draw()?;
    g.text()
        .x(&mid)
        .y(0.08)
        .labels(hex)
        .cex(0.8)
        .col("grey30")
        .try_draw()?;
    g.title().main("The palette: .col(1) to .col(8)").try_draw()
}

/// Draws the regions of a page: the device, the outer margins (`oma`),
/// the figure region, the margins (`mar`) and the plot region, with the
/// margin lines numbered.
///
/// It sets [`par(oma)`] to show the outer margins, and back to 0 at the end.
///
/// Drawn best on a page about 7 x 6 inches.
///
///
/// [`par(oma)`]: crate::par::Par::oma
///
#[doc = include_str!("regions.svg")]
///
/// This is the diagram of R's `par(mar = , oma = )`.
///
/// # Examples
///
/// ```
/// # use rs_graphics::prelude::*;
/// # let mut dev = Device::null().width(7).height(6).open().unwrap();
/// rs_graphics::charts::regions(&mut dev)?;
/// # Ok::<(), rs_graphics::Error>(())
/// ```
///
/// # Errors
///
/// If the page is too small for the margins (R's "figure margins too
/// large").
pub fn regions<G: Graphics>(g: &mut G) -> Result<(), Error> {
    g.par().oma([2, 2, 2, 2]).try_draw()?;
    blank(g, (0.0, 1.0), (0.0, 1.0))?;
    g.box_()
        .which(BoxWhich::Outer)
        .col("grey40")
        .lwd(2)
        .try_draw()?;
    g.box_()
        .which(BoxWhich::Figure)
        .col("darkorange")
        .lwd(2)
        .try_draw()?;
    g.box_()
        .which(BoxWhich::Plot)
        .col("steelblue")
        .lwd(2)
        .try_draw()?;
    g.text()
        .x(0.5)
        .y([0.62, 0.5, 0.38])
        .labels([
            "plot region",
            "user coordinates: par(usr)",
            "drawing clipped here: .xpd(false)",
        ])
        .col(["steelblue", "black", "grey30"])
        .try_draw()?;
    // R's default `mar` is 5.1, 4.1, 4.1 and 2.1 lines.
    for (side, nlines) in [(1, 5), (2, 4), (3, 4), (4, 2)] {
        for line in 0..nlines {
            g.mtext()
                .text(format!("line {line}"))
                .side(side)
                .line(line)
                .adj(0.08)
                .cex(0.7)
                .col("grey40")
                .try_draw()?;
        }
        g.mtext()
            .text(format!("margin {side}: mar[{side}]"))
            .side(side)
            .line(nlines - 2)
            .adj(0.6)
            .col("darkorange")
            .try_draw()?;
        g.mtext()
            .text(format!("outer margin {side}: oma[{side}]"))
            .side(side)
            .line(0.5)
            .outer(true)
            .col("grey40")
            .try_draw()?;
    }
    g.mtext()
        .text("figure region")
        .side(3)
        .line(3)
        .adj(1)
        .col("darkorange")
        .try_draw()?;
    g.mtext()
        .text("device")
        .side(3)
        .line(0.5)
        .adj(1)
        .outer(true)
        .col("grey40")
        .try_draw()?;
    g.par().oma([0, 0, 0, 0]).try_draw()
}
