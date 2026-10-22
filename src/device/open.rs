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

use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

use super::Device;
use crate::backend::Backend;
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
    /// error names the file.
    ///
    /// As in R, the file name may hold a C integer format for the page
    /// number, `"Rplot%03d.png"` (at most one `%d`, `%i`, `%u`, `%o`, `%x`
    /// or `%X`, with flags, width and precision; `%%` is a `%`): a PNG or
    /// SVG device then writes one file per page (`Rplot001.png`,
    /// `Rplot002.png`, ...), each when the next page starts and the last
    /// at `close()`. Without one, the file holds the last page, as in R. A
    /// PDF keeps every page in one file, named for page 1 (R's
    /// `pdf(onefile = TRUE)`). Any other `%` is an
    /// [`InvalidInput`](std::io::ErrorKind::InvalidInput) error ("invalid
    /// 'file' argument"), as R's `checkIntFormat()`.
    ///
    /// The contents are written by
    /// [`close`](Device::close) (R's `dev.off()`); a device dropped
    /// without it leaves the file empty.
    pub fn open(self) -> std::io::Result<Device<AnyBackend>> {
        let name = FileName::parse(&self.file)?;
        let first = name.for_page(1);
        let file = create(&first)?;
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
        dev.output = Some(Output {
            file,
            // One file per page for PNG and SVG when the name has a page
            // number; PDF keeps every page in its one file, as R's
            // `pdf(onefile = TRUE)`.
            per_page: name.numbered() && self.format != Format::Pdf,
            name,
            page: 1,
            error: None,
        });
        Ok(dev)
    }
}

/// Creates (or empties) `path`, with R's message on failure.
fn create(path: &Path) -> std::io::Result<File> {
    File::create(path).map_err(|e| {
        std::io::Error::new(
            e.kind(),
            format!("cannot open file '{}': {e}", path.display()),
        )
    })
}

/// A device's file name, which may hold a C integer format for the page
/// number (`"Rplot%03d.png"`), as R's devices take: at most one
/// `%[#0 ,+-]*[0-9.]*[diouxX]`, and `%%` for a `%` (R's
/// `checkIntFormat()`).
#[derive(Debug)]
pub(crate) struct FileName {
    /// A name that isn't UTF-8, used as it is (it has no `%` to read).
    raw: Option<PathBuf>,
    /// The name, split around its conversion, if any (`%%` already `%`).
    before: String,
    conversion: Option<Conversion>,
    after: String,
}

#[derive(Debug)]
struct Conversion {
    flags: String,
    width: usize,
    precision: Option<usize>,
    kind: char,
}

impl FileName {
    pub(crate) fn parse(path: &Path) -> std::io::Result<FileName> {
        let invalid = || {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("invalid 'file' argument '{}'", path.display()),
            )
        };
        // A name that isn't UTF-8 has no `%` to interpret.
        let Some(s) = path.to_str() else {
            return Ok(FileName {
                raw: Some(path.to_path_buf()),
                before: String::new(),
                conversion: None,
                after: String::new(),
            });
        };
        let (mut before, mut after) = (String::new(), String::new());
        let mut conversion = None;
        let mut chars = s.chars().peekable();
        while let Some(c) = chars.next() {
            let out = if conversion.is_some() {
                &mut after
            } else {
                &mut before
            };
            if c != '%' {
                out.push(c);
                continue;
            }
            if chars.peek() == Some(&'%') {
                chars.next();
                out.push('%');
                continue;
            }
            if conversion.is_some() {
                return Err(invalid());
            }
            let mut flags = String::new();
            while let Some(&f) = chars.peek().filter(|f| "#0 ,+-".contains(**f)) {
                flags.push(f);
                chars.next();
            }
            let digits = |chars: &mut std::iter::Peekable<std::str::Chars>| {
                let mut d = String::new();
                while let Some(&c) = chars.peek().filter(|c| c.is_ascii_digit()) {
                    d.push(c);
                    chars.next();
                }
                d
            };
            let width = digits(&mut chars).parse().unwrap_or(0);
            let precision = if chars.peek() == Some(&'.') {
                chars.next();
                Some(digits(&mut chars).parse().unwrap_or(0))
            } else {
                None
            };
            let kind = chars
                .next()
                .filter(|k| "diouxX".contains(*k))
                .ok_or_else(invalid)?;
            conversion = Some(Conversion {
                flags,
                width,
                precision,
                kind,
            });
        }
        Ok(FileName {
            raw: None,
            before,
            conversion,
            after,
        })
    }

    /// Whether the name has a page number in it.
    pub(crate) fn numbered(&self) -> bool {
        self.conversion.is_some()
    }

    /// The file name for page `page`, as C's `snprintf(buf, name, page)`.
    pub(crate) fn for_page(&self, page: u32) -> PathBuf {
        if let Some(raw) = &self.raw {
            return raw.clone();
        }
        let Some(c) = &self.conversion else {
            return PathBuf::from(&self.before);
        };
        let mut digits = match c.kind {
            'o' => format!("{page:o}"),
            'x' => format!("{page:x}"),
            'X' => format!("{page:X}"),
            _ => page.to_string(),
        };
        if let Some(p) = c.precision {
            while digits.len() < p {
                digits.insert(0, '0');
            }
        }
        if c.flags.contains('#') {
            match c.kind {
                'o' if !digits.starts_with('0') => digits.insert(0, '0'),
                'x' => digits.insert_str(0, "0x"),
                'X' => digits.insert_str(0, "0X"),
                _ => {}
            }
        }
        let signed = matches!(c.kind, 'd' | 'i');
        let sign = if signed && c.flags.contains('+') {
            "+"
        } else if signed && c.flags.contains(' ') {
            " "
        } else {
            ""
        };
        let len = sign.len() + digits.len();
        let pad = c.width.saturating_sub(len);
        let number = if c.flags.contains('-') {
            format!("{sign}{digits}{}", " ".repeat(pad))
        } else if c.flags.contains('0') && c.precision.is_none() {
            format!("{sign}{}{digits}", "0".repeat(pad))
        } else {
            format!("{}{sign}{digits}", " ".repeat(pad))
        };
        PathBuf::from(format!("{}{number}{}", self.before, self.after))
    }
}

/// The file(s) of a device opened for a file.
pub(crate) struct Output {
    /// The current page's file (the only one unless `per_page`).
    file: File,
    name: FileName,
    /// One file per page: each finished page is written to its file and
    /// the next page's file is created.
    per_page: bool,
    page: u32,
    /// The first error from writing a finished page, reported by `close`.
    error: Option<std::io::Error>,
}

impl Output {
    /// Writes a finished page's `bytes` to the current file and creates the
    /// next page's (after the first error, does nothing).
    fn next_page(&mut self, bytes: &[u8]) {
        if self.error.is_some() {
            return;
        }
        let result = self
            .file
            .write_all(bytes)
            .and_then(|_| self.file.flush())
            .and_then(|_| create(&self.name.for_page(self.page + 1)));
        match result {
            Ok(file) => {
                self.file = file;
                self.page += 1;
            }
            Err(e) => self.error = Some(e),
        }
    }

    /// Writes the last page's `bytes`; returns the first error.
    pub(crate) fn close(mut self, bytes: &[u8]) -> std::io::Result<()> {
        if let Some(e) = self.error.take() {
            return Err(e);
        }
        self.file.write_all(bytes)?;
        self.file.flush()
    }
}

impl<B: Backend> Device<B> {
    /// At a new page, with one file per page: write the finished page to
    /// its file and start the next one.
    pub(super) fn next_page_file(&mut self) {
        if let Some(out) = self.output.as_mut().filter(|o| o.per_page) {
            out.next_page(&self.backend.take_page());
        }
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
    /// dev.plot().x(&[1.0, 2.0, 3.0]).y(&[2.0, 1.0, 3.0]).draw();
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
    /// dev.plot().x(&[1.0, 2.0, 3.0]).y(&[2.0, 1.0, 3.0]).draw();
    /// dev.close()?; // R: dev.off()
    /// # Ok(())
    /// # }
    /// ```
    pub fn pdf(file: impl AsRef<Path>) -> DeviceBuilder {
        Device::format(Format::Pdf, file)
    }

    /// R's `svg(filename, ...)`: [`Device::format`] with SVG. Default page
    /// 7 x 7 inches, white background. One file per page with a page
    /// number in `file` (`"plot%03d.svg"`; see
    /// [`open`](DeviceBuilder::open)).
    pub fn svg(file: impl AsRef<Path>) -> DeviceBuilder {
        Device::format(Format::Svg, file)
    }

    /// R's `png(filename, ...)`: [`Device::format`] with PNG. Sizes are in
    /// inches (R's `units = "in"`), with [`res`](DeviceBuilder::res) pixels
    /// per inch (default 72); the default 6.67 x 6.67 inches at 72 ppi is
    /// R's default 480 x 480 pixel image. White background. One file per
    /// page with a page number in `file`, as R's default
    /// `"Rplot%03d.png"` (see [`open`](DeviceBuilder::open)).
    pub fn png(file: impl AsRef<Path>) -> DeviceBuilder {
        Device::format(Format::Png, file)
    }
}
