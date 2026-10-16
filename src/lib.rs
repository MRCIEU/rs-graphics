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
//! // R: pdf("scatter.pdf", 7, 7); plot(x, y, main = ...); dev.off()
//! let mut dev = Device::for_path("scatter.pdf", 7.0, 7.0, 300.0)?;
//! let opts = PlotOptions::default()
//!     .pch(19)
//!     .col("steelblue")
//!     .main(Label::math("y == beta[0] + beta[1] * x"))
//!     .xlab("x")
//!     .ylab("y");
//! plot(&mut dev, &x, &y, &opts);
//! dev.abline(&AblineOptions::default().h(6.0).col("grey").lty(LineType::DASHED));
//! dev.save("scatter.pdf")?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! [`device::Device`] holds the page and `par()` state. Its methods are
//! R's low-level functions (`points()`, `axis()`, `abline()`, `legend()`,
//! ...), each taking its data and an options struct (R's other
//! arguments). Free functions in [`highlevel`] are the high-level plots
//! (`plot()`, `hist()`, ...). Options can be set field by field or with
//! the chainable setters above ([`setters`]). [`prelude`] imports it all.
//!
//! The split follows R's: high-level functions start a new plot, and
//! low-level ones add to the current one. The low-level functions need
//! the device's internals (its clip regions, `xpd` limits, symbol
//! drawing), so they are methods. The high-level plots use only the
//! public API, so they are free functions, just like plots you write
//! yourself: `plot(&mut dev, ..)` and `my_panel(&mut dev, ..)` are called
//! the same way. (`hist(add = TRUE)` adds to the current plot but is still
//! a high-level function.)

#[doc(hidden)]
pub mod afm;
pub mod backend;
pub mod color;
pub mod device;
pub mod error;
pub mod geometry;
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
