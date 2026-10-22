// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! A port of R's `graphics` package: `plot()`, `hist()`, `barplot()`,
//! `boxplot()`, `pie()`, and the low-level functions they are built from
//! (`axis()`, `title()`, `legend()`, `mtext()`, ...), drawing to PDF,
//! SVG or PNG with R's layout rules and defaults.
//!
//! ```no_run
//! use rs_graphics::prelude::*;
//!
//! let x = [1.0, 2.0, 3.0, 4.0, 5.0];
//! let y = [2.1, 3.9, 6.2, 7.8, 10.1];
//!
//! // R: pdf("scatter.pdf"); plot(x, y, ...); abline(h = 6); dev.off()
//! let mut dev = Device::pdf("scatter.pdf").open()?;
//! dev.plot().x(&x).y(&y)
//!     .pch(19)
//!     .col("steelblue")
//!     .main(Label::math("y == beta[0] + beta[1] * x"))
//!     .xlab("x")
//!     .ylab("y")
//!     .draw();
//! dev.abline().h(6.0).col("grey").lty("dashed").draw();
//! dev.close()?;
//! # Ok::<(), std::io::Error>(())
//! ```
//!
//! [`device::Device`] holds the page and `par()` state. Every R graphics
//! function is a method of the [`graphics::Graphics`] trait, which
//! `Device` implements, named as in R (`box()` is `box_()`, as `box` is a
//! Rust keyword). A method starts a call; the call's chainable setters
//! take all of R's arguments, named as R's (data too: `.x(&x)`), and
//! `.draw()` runs it. A required argument (one R gives no default) that is
//! left out panics at `.draw()` with R's message. To reuse a set of arguments, build its options struct
//! (`AxisOptions`, ...) once and pass it with `.with(&opts)`.
//! [`prelude`] imports it all, including the trait. As in R, numbers may
//! be integers or floats: `dev.plot().x(1..=10).y(&y).cex(2).xlim(0, 12)` (data
//! vectors are any [`Numbers`](numbers::Numbers), by reference or by
//! value; `f64` data isn't copied either way). R's `NA` in a style vector is `None`:
//! `legend(pch = c(19, NA))` is `.pch([Some(19), None])`.
//!
//! # Devices and files
//!
//! As in R, a device is opened for a file with R's arguments, which
//! creates the file, and the file is written when the device is closed.
//! [`Device::pdf`](device::Device::pdf), [`Device::svg`](device::Device::svg)
//! and [`Device::png`](device::Device::png) (or
//! [`Device::format`](device::Device::format), for a format chosen at run
//! time) take the file and return a [`DeviceBuilder`](device::DeviceBuilder)
//! for R's other arguments (`width` and `height` in inches, `pointsize`,
//! `bg`, PNG's `res`, PDF's `compress`), with R's defaults; `.open()`
//! opens the device and creates the file (so a bad path fails before any
//! drawing), and [`close`](device::Device::close) (R's `dev.off()`)
//! writes it. The format is always the constructor's, never the
//! file name's (`Device::pdf("x.png")` writes a PDF, as R's `pdf()`).
//! [`Device::new`](device::Device::new) takes a backend directly instead:
//! for output kept in memory ([`finish`](device::Device::finish)) and
//! backend options the builder doesn't cover.
//!
//! # High-level and low-level functions
//!
//! As in R, the high-level functions ([`plot`](graphics::Graphics::plot),
//! [`hist`](graphics::Graphics::hist), [`barplot`](graphics::Graphics::barplot),
//! [`boxplot`](graphics::Graphics::boxplot), [`pie`](graphics::Graphics::pie))
//! start a new plot: they call [`plot_new`](graphics::Graphics::plot_new),
//! which moves to the next panel of a multi-panel page (or to a new page).
//! Everything else ([`points`](graphics::Graphics::points),
//! [`axis`](graphics::Graphics::axis), [`legend`](graphics::Graphics::legend),
//! ...) adds to the current plot. (`hist`, `barplot` and `boxplot` with
//! `add` set add too, as R's `add = TRUE` does.)
//!
//! # Multi-panel pages
//!
//! `par()`'s [`mfrow`](graphics::ParCall::mfrow) and
//! [`mfcol`](graphics::ParCall::mfcol), as in R, and
//! [`Device::layout`](device::Device::layout) split the page into panels;
//! each high-level call (or `plot_new()`) fills the next one, and
//! low-level calls after it draw in that panel. `par()`'s
//! [`mfg`](graphics::ParCall::mfg) picks the next panel,
//! [`fig`](graphics::ParCall::fig) and [`fin`](graphics::ParCall::fin)
//! place a single figure anywhere on the page, and `new(true)` draws over
//! the current one.
//!
//! ```no_run
//! use rs_graphics::prelude::*;
//!
//! let x = [1.0, 2.0, 3.0, 4.0];
//! let mut dev = Device::pdf("panels.pdf").width(8.0).height(4.0).open()?;
//! dev.par().mfrow(1, 2).oma([0.0, 0.0, 2.0, 0.0]).draw();
//! dev.plot().x(&x).y(&[1.0, 4.0, 9.0, 16.0]).draw(); // panel 1
//! dev.abline().a(0).b(4).draw(); // still panel 1
//! dev.hist().x(&[1.0, 2.0, 2.5, 3.0, 3.2]).draw(); // panel 2
//! dev.mtext().text("Two panels").side(3).outer(true).line(0.5).draw();
//! dev.close()?;
//! # Ok::<(), std::io::Error>(())
//! ```
//!
//! # Figures as values
//!
//! A [`recording::Figure`] implements the same trait but records the calls
//! instead of drawing them, so a figure (a plot and its margins, as R's
//! figure region) can be built without a device and drawn later with
//! [`Device::add_figure`](device::Device::add_figure) (or
//! [`add_figure_at`](device::Device::add_figure_at) for a chosen panel),
//! as often as needed and on any device. The output is identical to making
//! the calls on the device.
//!
//! ```no_run
//! use rs_graphics::prelude::*;
//!
//! let mut fig = Figure::new();
//! fig.par().las(1).draw();
//! fig.hist().x(&[1.0, 2.0, 2.5, 3.0, 3.2]).main("Recorded").draw();
//!
//! let mut dev = Device::pdf("figure.pdf").width(8.0).height(4.0).open()?;
//! dev.par().mfrow(1, 2).draw();
//! dev.add_figure_at(1, 2, &fig); // right-hand panel
//! dev.close()?;
//! # Ok::<(), std::io::Error>(())
//! ```
//!
//! # Your own plots
//!
//! Write plots of your own against the [`graphics::Graphics`] trait: as
//! functions taking `&mut G` for any `G: Graphics`, or as methods of an
//! extension trait, which read like the built-in ones. Either way they
//! work on a device and in a recorded `Figure`.
//!
//! ```no_run
//! use rs_graphics::prelude::*;
//!
//! trait Diagnostic {
//!     /// Observed against expected, with the line y = x.
//!     fn obs_vs_exp(&mut self, exp: &[f64], obs: &[f64]);
//! }
//!
//! impl<G: Graphics> Diagnostic for G {
//!     fn obs_vs_exp(&mut self, exp: &[f64], obs: &[f64]) {
//!         self.plot().x(exp).y(obs).pch(20).draw();
//!         self.abline().a(0).b(1).col("red").draw();
//!     }
//! }
//!
//! let mut dev = Device::svg("diagnostic.svg").width(5.0).height(5.0).open()?;
//! dev.obs_vs_exp(&[1.0, 2.0, 3.0], &[1.1, 1.9, 3.3]);
//! dev.close()?;
//! # Ok::<(), std::io::Error>(())
//! ```

#[doc(hidden)]
pub mod afm;
pub mod backend;
pub mod color;
pub mod device;
pub mod error;
pub mod font;
pub mod geometry;
pub mod graphics;
pub mod highlevel;
pub mod layout;
pub mod lty;
#[doc(hidden)]
pub mod named_colors;
pub mod numbers;
pub mod par;
#[doc(hidden)]
pub mod pch;
pub mod plotmath;
pub mod prelude;
#[doc(hidden)]
pub mod pretty;
pub mod recording;
pub mod setters;
