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
//! plot(&mut dev, &x, &y, &PlotOptions {
//!     pch: 19,
//!     col: "steelblue".into(),
//!     main: Some(Label::math("y == beta[0] + beta[1] * x")),
//!     xlab: Some("x".into()),
//!     ylab: Some("y".into()),
//!     ..PlotOptions::default()
//! });
//! dev.abline(None, None, Some(6.0), None, "grey".into(), 1.0, LineType::DASHED);
//! dev.save("scatter.pdf")?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! [`device::Device`] holds the page and `par()` state; its methods are
//! the low-level functions. Free functions in [`highlevel`] are the
//! high-level plots. [`prelude`] imports both, with the value types.

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
