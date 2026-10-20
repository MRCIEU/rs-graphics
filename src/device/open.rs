// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Opening a device for a file, as R's `pdf()`, `svg()` and `png()` do:
//! [`Device::pdf`], [`Device::svg`], [`Device::png`] and, for a format
//! chosen at run time, [`Device::format`].

use std::path::{Path, PathBuf};

use super::Device;
use crate::backend::pdf::PdfBackend;
use crate::backend::raster::RasterBackend;
use crate::backend::svg::SvgBackend;
use crate::backend::{AnyBackend, Format};
use crate::color::{Color, ColorSpec};
use crate::par::Par;

/// The arguments of R's `pdf()`, `svg()` and `png()` for a file, started by
/// [`Device::format`] (or [`Device::pdf`], [`Device::svg`],
/// [`Device::png`]) and finished by [`open`](DeviceBuilder::open). Unset
/// fields are R's defaults, given for each. Sizes are in inches for every
/// format (R's `png()` counts pixels by default; its default 480 x 480
/// pixels at 72 ppi is 6.67 x 6.67 inches here). The default PNG size
/// stays 6.67 inches at any `res`, so a higher `res` gives more pixels,
/// where R's `png(res = )` keeps 480 x 480 pixels.
#[must_use = "a device is only opened by `.open()`"]
#[derive(Clone, Debug)]
pub struct DeviceBuilder {
    format: Format,
    file: PathBuf,
    /// Page width in inches; `None`: 7 for PDF and SVG, 480/72 for PNG.
    pub width: Option<f64>,
    /// Page height in inches; `None`: as `width`.
    pub height: Option<f64>,
    /// R's `pointsize`: the starting `par(ps)`, which also fixes the size
    /// of plotting symbols and margin lines from the start (R's
    /// `startps`); `None`: 12.
    pub pointsize: Option<f64>,
    /// The page background, and the starting `par(bg)`; a colour that
    /// draws nothing (`ColorSpec::None`, `"transparent"`) is transparent.
    /// `None`: transparent for PDF, white for SVG and PNG (R's defaults).
    pub bg: Option<ColorSpec>,
    /// PNG only: pixels per inch (R's `res`); `None`: 72. Ignored for PDF
    /// and SVG.
    pub res: Option<f64>,
    /// PDF only: compress the page contents (R's `compress`); `None`:
    /// `true`. Ignored for SVG and PNG.
    pub compress: Option<bool>,
}

crate::setters::setters!(DeviceBuilder {
    width: option f64,
    height: option f64,
    pointsize: option f64,
    bg: option ColorSpec,
    res: option f64,
    compress: option bool,
}; no_setter: format, file);

impl DeviceBuilder {
    /// Opens the device and creates (or empties) its file, as R's `pdf()`
    /// and co. do, so a bad path fails here rather than after drawing; the
    /// error names the file. The contents are written by
    /// [`close`](Device::close) (R's `dev.off()`); a device dropped
    /// without it leaves the file empty.
    pub fn open(self) -> std::io::Result<Device<AnyBackend>> {
        let file = std::fs::File::create(&self.file).map_err(|e| {
            std::io::Error::new(
                e.kind(),
                format!("cannot open file '{}': {e}", self.file.display()),
            )
        })?;
        let default_size = match self.format {
            Format::Png => 480.0 / 72.0,
            Format::Pdf | Format::Svg => 7.0,
        };
        let width = self.width.unwrap_or(default_size);
        let height = self.height.unwrap_or(default_size);
        let mut par = Par::default();
        let bg = self
            .bg
            .map(|c| c.resolve(&par.palette).unwrap_or(Color::TRANSPARENT));
        let backend: AnyBackend = match self.format {
            Format::Pdf => {
                let pdf =
                    PdfBackend::new(width, height).with_compression(self.compress.unwrap_or(true));
                match bg {
                    Some(c) => pdf.with_background(c).into(),
                    None => pdf.into(),
                }
            }
            Format::Svg => SvgBackend::new(width, height).into(),
            Format::Png => RasterBackend::new(
                width,
                height,
                self.res.unwrap_or(72.0),
                bg.unwrap_or(Color::WHITE),
            )
            .into(),
        };
        if let Some(ps) = self.pointsize {
            par.ps = ps;
        }
        let mut dev = Device::new(backend, par);
        // The SVG backend has no background of its own: the page is
        // painted in `par(bg)`. Set after `Device::new`, which takes a
        // white `par.bg` as "not set" and uses the backend's default.
        if let Some(c) = bg {
            dev.par.bg = c;
        }
        dev.file = Some(file);
        Ok(dev)
    }
}

impl Device<AnyBackend> {
    /// Starts opening a device of `format` for `file`, like R's `pdf()`,
    /// `svg()` or `png()` with the format chosen at run time: `format` is
    /// a [`Format`] or its name (`"pdf"`, `"svg"`, `"png"`, ignoring
    /// case). Set R's other arguments on the [`DeviceBuilder`] and finish
    /// with [`open`](DeviceBuilder::open), which creates the file;
    /// [`close`](Device::close) writes it. The format never comes from
    /// `file`'s extension.
    ///
    /// # Panics
    /// If `format` is a string that names no format; use [`Format::parse`]
    /// or `str::parse` to handle that yourself.
    ///
    /// ```no_run
    /// use rs_graphics::prelude::*;
    ///
    /// # fn main() -> std::io::Result<()> {
    /// let format = "png"; // e.g. from a command-line option
    /// let mut dev = Device::format(format, "plot.png").res(300.0).open()?;
    /// dev.plot(&[1.0, 2.0, 3.0], &[2.0, 1.0, 3.0]).draw();
    /// dev.close()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn format(format: impl Into<Format>, file: impl AsRef<Path>) -> DeviceBuilder {
        DeviceBuilder {
            format: format.into(),
            file: file.as_ref().to_path_buf(),
            width: None,
            height: None,
            pointsize: None,
            bg: None,
            res: None,
            compress: None,
        }
    }

    /// R's `pdf(file, ...)`: [`Device::format`] with PDF, whatever `file`'s
    /// extension. Default page 7 x 7 inches, transparent background.
    ///
    /// ```no_run
    /// use rs_graphics::prelude::*;
    ///
    /// # fn main() -> std::io::Result<()> {
    /// // R: pdf("scatter.pdf", width = 7, height = 5)
    /// let mut dev = Device::pdf("scatter.pdf").width(7.0).height(5.0).open()?;
    /// dev.plot(&[1.0, 2.0, 3.0], &[2.0, 1.0, 3.0]).draw();
    /// dev.close()?; // R: dev.off()
    /// # Ok(())
    /// # }
    /// ```
    pub fn pdf(file: impl AsRef<Path>) -> DeviceBuilder {
        Device::format(Format::Pdf, file)
    }

    /// R's `svg(filename, ...)`: [`Device::format`] with SVG. Default page
    /// 7 x 7 inches, white background.
    pub fn svg(file: impl AsRef<Path>) -> DeviceBuilder {
        Device::format(Format::Svg, file)
    }

    /// R's `png(filename, ...)`: [`Device::format`] with PNG. Sizes are in
    /// inches (R's `units = "in"`), with [`res`](DeviceBuilder::res) pixels
    /// per inch (default 72); the default 6.67 x 6.67 inches at 72 ppi is
    /// R's default 480 x 480 pixel image. White background.
    pub fn png(file: impl AsRef<Path>) -> DeviceBuilder {
        Device::format(Format::Png, file)
    }
}
