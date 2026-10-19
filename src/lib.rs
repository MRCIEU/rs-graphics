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
//! // R: pdf("scatter.pdf", 7, 7); plot(x, y, ...); abline(h = 6); dev.off()
//! let mut dev = Device::for_path("scatter.pdf", 7.0, 7.0, 300.0)?;
//! dev.plot(&x, &y)
//!     .pch(19)
//!     .col("steelblue")
//!     .main(Label::math("y == beta[0] + beta[1] * x"))
//!     .xlab("x")
//!     .ylab("y")
//!     .draw();
//! dev.abline().h(6.0).col("grey").lty(LineType::DASHED).draw();
//! dev.save("scatter.pdf")?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! [`device::Device`] holds the page and `par()` state. Every R graphics
//! function is a method of the [`graphics::Graphics`] trait, which
//! `Device` implements, named as in R (`box()` is `box_()`, as `box` is a
//! Rust keyword). A method takes R's data arguments and starts a call;
//! the call's chainable setters take R's other arguments, and `.draw()`
//! runs it. To reuse a set of arguments, build its options struct
//! (`AxisOptions`, ...) once and pass it with `.with(&opts)`.
//! [`prelude`] imports it all, including the trait.
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
//! ...) adds to the current plot. (`hist` with `add` set adds too, as R's
//! `hist(add = TRUE)` does.)
//!
//! # Multi-panel pages
//!
//! [`Device::mfrow`](device::Device::mfrow),
//! [`Device::mfcol`](device::Device::mfcol) and
//! [`Device::layout`](device::Device::layout) split the page into panels;
//! each high-level call (or `plot_new()`) fills the next one, and
//! low-level calls after it draw in that panel.
//! [`Device::mfg`](device::Device::mfg) picks the next panel, and
//! [`Device::set_new`](device::Device::set_new) draws over the current
//! one.
//!
//! ```
//! use rs_graphics::prelude::*;
//!
//! let x = [1.0, 2.0, 3.0, 4.0];
//! let mut dev = Device::pdf(8.0, 4.0);
//! dev.mfrow(1, 2);
//! dev.plot(&x, &[1.0, 4.0, 9.0, 16.0]).draw(); // panel 1
//! dev.abline().coef(0.0, 4.0).draw(); // still panel 1
//! dev.hist(&[1.0, 2.0, 2.5, 3.0, 3.2]).draw(); // panel 2
//! dev.mtext("Two panels", 3).outer(true).line(-1.5).draw();
//! let pdf = dev.finish();
//! # assert!(!pdf.is_empty());
//! ```
//!
//! # Your own plots
//!
//! Write plots of your own against the [`graphics::Graphics`] trait: as
//! functions taking `&mut G` for any `G: Graphics`, or as methods of an
//! extension trait, which read like the built-in ones.
//!
//! ```
//! use rs_graphics::prelude::*;
//!
//! trait Diagnostic {
//!     /// Observed against expected, with the line y = x.
//!     fn obs_vs_exp(&mut self, exp: &[f64], obs: &[f64]);
//! }
//!
//! impl<G: Graphics> Diagnostic for G {
//!     fn obs_vs_exp(&mut self, exp: &[f64], obs: &[f64]) {
//!         self.plot(exp, obs).pch(20).draw();
//!         self.abline().coef(0.0, 1.0).col("red").draw();
//!     }
//! }
//!
//! let mut dev = Device::svg(5.0, 5.0);
//! dev.obs_vs_exp(&[1.0, 2.0, 3.0], &[1.1, 1.9, 3.3]);
//! ```

#[doc(hidden)]
pub mod afm;
pub mod backend;
pub mod color;
pub mod device;
pub mod error;
pub mod geometry;
pub mod graphics;
pub mod highlevel;
pub mod layout;
pub mod lty;
#[doc(hidden)]
pub mod named_colors;
pub mod par;
#[doc(hidden)]
pub mod pch;
pub mod plotmath;
pub mod prelude;
#[doc(hidden)]
pub mod pretty;
pub mod setters;
