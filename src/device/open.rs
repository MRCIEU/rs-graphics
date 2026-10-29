// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Opening a device, as R's `pdf()`, `svg()` and `png()` do: for a file with
//! [`Device::pdf`], [`Device::svg`], [`Device::png`] and, for a format
//! chosen at run time, [`Device::format`]; in memory with
//! [`Device::memory`]; and for no output with [`Device::null`].

use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

use super::Device;
use crate::backend::Backend;
use crate::backend::null::NullBackend;
use crate::backend::pdf::PdfBackend;
use crate::backend::raster::RasterBackend;
use crate::backend::svg::SvgBackend;
use crate::backend::{AnyBackend, Format};
use crate::color::{Color, ColorSpec};
use crate::par::Par;
use crate::setters::IntoArg;

crate::setters::options! {
    /// The options of a device being opened.
    ///
    /// Start one with [`Device::pdf`], [`Device::svg`], [`Device::png`] or
    /// [`Device::format`] (for a file), [`Device::memory`] (output kept in
    /// memory) or [`Device::null`] (no output), set its options and finish
    /// with [`open`](DeviceBuilder::open). Unset options take R's defaults, given
    /// for each. Sizes are in inches for every format unless
    /// [`units`](DeviceBuilder::units) says otherwise. The default PNG is 6.67
    /// x 6.67 inches, R's default 480 x 480 pixels at 72 ppi; it stays 6.67
    /// inches at any `res`, so a higher `res` gives more pixels, where R's
    /// `png(res = )` keeps 480 x 480 pixels.
    ///
    /// These are the arguments of R's `pdf()`, `svg()` and `png()`.
    #[must_use = "a device is only opened by `.open()`"]
    #[derive(Clone, Debug)]
    pub struct DeviceBuilder {
        format: Format => no_setter,
        /// Where the output goes.
        target: Target => no_setter,
        /// Page width in [`units`](DeviceBuilder::units). `None`: 7 inches for
        /// PDF and SVG, 480/72 inches for PNG, whatever the units.
        pub width: Option<f64> => number f64,
        /// Page height in [`units`](DeviceBuilder::units). `None`: as `width`.
        pub height: Option<f64> => number f64,
        /// Units of `width` and `height`. `None`: inches. Inches, centimetres
        /// and millimetres work for every format; pixels only for PNG, where
        /// they are divided by `res` (so `.width(1200).units("px").res(300)` is
        /// 4 inches, as R's `png(width = 1200, res = 300)`). Pixels for a PDF
        /// or SVG device make [`open`](DeviceBuilder::open) fail. In R only
        /// bitmap devices such as `png()` take `units`, defaulting to pixels.
        pub units: Option<Units> => option Units,
        /// Starting text size, [`par(ps)`], in big points; it also fixes the
        /// size of plotting symbols and margin lines from the start. `None`:
        /// 12.
        ///
        /// [`par(ps)`]: crate::par::Par::ps
        pub pointsize: Option<f64> => number f64,
        /// Page background, and the starting [`par(bg)`]; a colour that draws
        /// nothing (`ColorSpec::None`, `"transparent"`) is transparent. `None`:
        /// transparent for PDF, white for SVG and PNG (R's defaults).
        ///
        /// [`par(bg)`]: crate::par::Par::bg
        pub bg: Option<ColorSpec> => option_value ColorSpec,
        /// Resolution in pixels per inch, PNG only (ignored for PDF and SVG).
        /// `None`: 72. R's `res`.
        pub res: Option<f64> => number f64,
        /// Compresses the output: a PDF's page contents (`None`: `true`), or
        /// a whole SVG document, gzipped as a `.svgz` file (`None`: `false`).
        /// Ignored for PNG, which is always compressed.
        pub compress: Option<bool> => option bool,
        /// Threads drawing a PNG, PNG only. `None`: one per CPU core. `1`
        /// draws on the calling thread. The output is the same for any
        /// number above 1.
        pub threads: Option<usize> => count usize,
        /// Puts all pages in one file, PDF only. `false` writes a file per
        /// page, named with the page number in `file` (as `"Rplot%03d.pdf"`;
        /// without one, the file holds the last page, as in R). `None`:
        /// `true`. Ignored for SVG and PNG, which always write a file per page
        /// when `file` has a page number.
        pub onefile: Option<bool> => option bool,
    }
}

/// Where a device's output goes.
#[derive(Clone, Debug)]
pub(crate) enum Target {
    /// A file (possibly one per page).
    File(PathBuf),
    /// Kept in memory, for [`Device::finish`].
    Memory,
    /// Nowhere: the null device.
    Null,
}

impl DeviceBuilder {
    /// Opens the device and creates (or empties) its file, so a bad path fails
    /// here rather than after drawing.
    ///
    /// The output is written by [`close`](Device::close); a device dropped
    /// without it leaves the file empty. The file name may hold a C integer
    /// format for the page number, `"Rplot%03d.png"` (at most one `%d`, `%i`,
    /// `%u`, `%o`, `%x` or `%X`, with flags, width and precision; `%%` is a
    /// `%`): a PNG or SVG device then writes one file per page
    /// (`Rplot001.png`, `Rplot002.png`, ...), each when the next page starts
    /// and the last at `close()`. Without one, the file holds the last page. A
    /// PDF keeps every page in one file, named for page 1, unless
    /// [`onefile`](DeviceBuilder::onefile) is `false`: then it writes a file
    /// per page as PNG and SVG do.
    ///
    /// As in R, opening the device creates the file; the page-number format is
    /// R's `checkIntFormat()`.
    ///
    /// # Examples
    ///
    /// One PNG file per page:
    ///
    /// ```no_run
    /// # use rs_graphics::prelude::*;
    /// # fn main() -> std::io::Result<()> {
    /// let mut dev = Device::png("panel%02d.png").open()?;
    /// dev.plot().x([1, 2, 3]).draw(); // panel01.png
    /// dev.plot().x([3, 2, 1]).draw(); // panel02.png
    /// dev.close()?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// A device from [`Device::memory`] or [`Device::null`] has no file, so
    /// opening it creates nothing.
    ///
    /// If the file can't be created (the error names the file); if `file` has
    /// any other `%`, or more than one page-number format, an
    /// [`InvalidInput`](std::io::ErrorKind::InvalidInput) error ("invalid 'file'
    /// argument"); for a format name that names no format, a setter's value
    /// that isn't one (`.bg("notacolour")`, `.units("ft")`),
    /// [`units`](DeviceBuilder::units) of pixels on a PDF or SVG device, or a
    /// negative palette number as `bg`, an `InvalidInput` error, before the
    /// file is created.
    pub fn open(self) -> std::io::Result<Device<AnyBackend>> {
        if let Some(e) = &self.deferred_error {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                e.to_string(),
            ));
        }
        let units = self.units.unwrap_or(Units::In);
        if units == Units::Px && self.format != Format::Png {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "units = \"px\" needs a PNG device",
            ));
        }
        if let Some(ColorSpec::Index(i)) = self.bg {
            if i < 0 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("numerical color values must be >= 0, found {i}"),
                ));
            }
        }
        // The file, for a file device.
        let (name, file) = match &self.target {
            Target::File(path) => {
                let name = FileName::parse(path)?;
                let file = create(&name.for_page(1))?;
                (Some(name), Some(file))
            }
            Target::Memory | Target::Null => (None, None),
        };
        let default_size = match self.format {
            Format::Png => 480.0 / 72.0,
            Format::Pdf | Format::Svg => 7.0,
        };
        let res = self.res.unwrap_or(72.0);
        let inches = |size: f64| match units {
            Units::In => size,
            Units::Cm => size / 2.54,
            Units::Mm => size / 25.4,
            Units::Px => size / res,
        };
        let width = self.width.map_or(default_size, inches);
        let height = self.height.map_or(default_size, inches);
        let mut par = Par::default();
        let bg = self
            .bg
            .map(|c| c.resolve(&par.palette).unwrap_or(Color::TRANSPARENT));
        let backend: AnyBackend = match self.format {
            _ if matches!(self.target, Target::Null) => NullBackend::new(width, height)
                .with_background(bg.unwrap_or(Color::TRANSPARENT))
                .into(),
            Format::Pdf => {
                let pdf =
                    PdfBackend::new(width, height).with_compression(self.compress.unwrap_or(true));
                match bg {
                    Some(c) => pdf.with_background(c).into(),
                    None => pdf.into(),
                }
            }
            Format::Svg => SvgBackend::new(width, height)
                .with_gzip(self.compress.unwrap_or(false))
                .into(),
            Format::Png => {
                let png = RasterBackend::new(width, height, res, bg.unwrap_or(Color::WHITE));
                match self.threads {
                    Some(n) => png.with_threads(n).into(),
                    None => png.into(),
                }
            }
        };
        if let Some(ps) = self.pointsize {
            par.ps = ps;
        }
        let mut dev = Device::with_par(backend, par);
        // The SVG backend has no background of its own: the page is
        // painted in `par(bg)`. Set after `Device::new`, which takes a
        // white `par.bg` as "not set" and uses the backend's default.
        if let Some(c) = bg {
            dev.par.bg = c;
        }
        if matches!(self.target, Target::Memory) {
            return Ok(dev);
        }
        dev.output = Some(Output {
            file,
            // One file per page for PNG and SVG when the name has a page
            // number (otherwise they keep only the last page anyway); a PDF
            // keeps every page in its one file unless `onefile = FALSE`, as
            // R's `pdf()` (then, without a page number, each page replaces
            // the last in the same file).
            per_page: match (&name, self.format) {
                (None, _) => false,
                (Some(_), Format::Pdf) => !self.onefile.unwrap_or(true),
                (Some(name), Format::Png | Format::Svg) => name.numbered(),
            },
            name,
            page: 1,
            error: None,
        });
        Ok(dev)
    }
}

/// The units of a device's `width` and `height`
/// ([`DeviceBuilder::units`]): R's `units` of `png()`.
///
/// From a string (`"cm".parse::<Units>()` or `TryFrom<&str>`): `"px"`,
/// `"in"`, `"cm"` or `"mm"`, or a unique prefix (R's `match.arg()`). Anything
/// else is an error.
///
/// # Examples
///
/// ```
/// # use rs_graphics::prelude::*;
/// assert_eq!("c".parse::<Units>().unwrap(), Units::Cm);
/// assert!("pt".parse::<Units>().is_err());
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Units {
    /// `"in"`: inches (the default here; R's `png()` defaults to pixels).
    #[default]
    In,
    /// `"cm"`: centimetres.
    Cm,
    /// `"mm"`: millimetres.
    Mm,
    /// `"px"`: pixels, at the device's `res` (PNG only).
    Px,
}

impl Units {
    /// `"px"`, `"in"`, `"cm"` or `"mm"`, or a unique prefix; `None`
    /// otherwise. The public routes are `FromStr`/`TryFrom<&str>`.
    pub(crate) fn parse(s: &str) -> Option<Units> {
        let all = [
            ("px", Units::Px),
            ("in", Units::In),
            ("cm", Units::Cm),
            ("mm", Units::Mm),
        ];
        let mut hits = all
            .iter()
            .filter(|(name, _)| !s.is_empty() && name.starts_with(s));
        match (hits.next(), hits.next()) {
            (Some(&(_, u)), None) => Some(u),
            _ => None,
        }
    }
}

crate::error::from_r_string!(Units, "units", "\"px\", \"in\", \"cm\" or \"mm\"");

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
/// `%[#0 ,+-]*` conversion with a width and an optional `.precision`, of
/// `d`, `i`, `o`, `u`, `x` or `X`, and `%%` for a `%`. This is R's
/// `checkIntFormat()`, a little stricter: R's pattern `[0-9.]*` also lets
/// malformed widths such as `%1.2.3d` through.
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
    /// The current page's file (the only one unless `per_page`); `None`
    /// for the null device, which writes nothing.
    file: Option<File>,
    name: Option<FileName>,
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
        let (Some(file), Some(name)) = (&mut self.file, &self.name) else {
            return;
        };
        if self.error.is_some() {
            return;
        }
        let result = file
            .write_all(bytes)
            .and_then(|_| file.flush())
            .and_then(|_| create(&name.for_page(self.page + 1)));
        match result {
            Ok(next) => {
                self.file = Some(next);
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
        match &mut self.file {
            Some(file) => {
                file.write_all(bytes)?;
                file.flush()
            }
            None => Ok(()),
        }
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
    /// Starts opening a device for `file` in a format chosen at run time.
    ///
    /// `format` is a [`Format`] or its name (`"pdf"`, `"svg"`, `"png"`,
    /// ignoring case); it never comes from `file`'s extension. Set the other
    /// options on the returned [`DeviceBuilder`] and finish with
    /// [`open`](DeviceBuilder::open), which creates the file;
    /// [`close`](Device::close) writes it.
    ///
    /// This is R's `pdf()`, `svg()` or `png()`, chosen at run time.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use rs_graphics::prelude::*;
    /// # fn main() -> std::io::Result<()> {
    /// let format = "png"; // e.g. from a command-line option
    /// let mut dev = Device::format(format, "plot.png").res(300).open()?;
    /// dev.plot().x([1, 2, 3]).y([2, 1, 3]).draw();
    /// dev.close()?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// A `format` string that names no format is an error of
    /// [`open`](DeviceBuilder::open).
    pub fn format(format: impl IntoArg<Format>, file: impl AsRef<Path>) -> DeviceBuilder {
        let (format, deferred_error) = match format.into_arg() {
            Ok(f) => (f, None),
            Err(e) => (Format::Pdf, Some(e)),
        };
        DeviceBuilder {
            deferred_error,
            format,
            target: Target::File(file.as_ref().to_path_buf()),
            width: None,
            height: None,
            units: None,
            pointsize: None,
            bg: None,
            res: None,
            compress: None,
            threads: None,
            onefile: None,
        }
    }

    /// Starts opening a PDF device for `file`, whatever its extension.
    ///
    /// Defaults: a 7 x 7 inch page, a transparent background, and all pages in
    /// one file (see [`onefile`](DeviceBuilder::onefile)). See [`Device::format`].
    ///
    /// This is R's `pdf()`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use rs_graphics::prelude::*;
    /// # fn main() -> std::io::Result<()> {
    /// let mut dev = Device::pdf("scatter.pdf").width(7).height(5).open()?;
    /// dev.plot().x([1, 2, 3]).y([2, 1, 3]).draw();
    /// dev.close()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn pdf(file: impl AsRef<Path>) -> DeviceBuilder {
        Device::format(Format::Pdf, file)
    }

    /// Starts opening an SVG device for `file`, whatever its extension.
    ///
    /// Defaults: a 7 x 7 inch page and a white background. A page number in
    /// `file` (`"plot%03d.svg"`) gives one file per page (see
    /// [`open`](DeviceBuilder::open)). See [`Device::format`].
    ///
    /// This is R's `svg()`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use rs_graphics::prelude::*;
    /// # fn main() -> std::io::Result<()> {
    /// let mut dev = Device::svg("plot.svg").width(5).height(5).open()?;
    /// dev.plot().x([1, 2, 3]).draw();
    /// dev.close()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn svg(file: impl AsRef<Path>) -> DeviceBuilder {
        Device::format(Format::Svg, file)
    }

    /// Starts opening a PNG device for `file`, whatever its extension.
    ///
    /// Sizes are in inches unless [`units`](DeviceBuilder::units) says
    /// otherwise, at [`res`](DeviceBuilder::res) pixels per inch (default 72);
    /// the default 6.67 x 6.67 inches at 72 ppi is R's default 480 x 480
    /// pixels. White background. A page number in `file` gives one file per
    /// page (see [`open`](DeviceBuilder::open)). See [`Device::format`].
    ///
    /// This is R's `png()`, with sizes in inches by default where R uses
    /// pixels.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use rs_graphics::prelude::*;
    /// # fn main() -> std::io::Result<()> {
    /// let mut dev = Device::png("plot.png").width(6).height(4).res(300).open()?;
    /// dev.plot().x([1, 2, 3]).draw();
    /// dev.close()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn png(file: impl AsRef<Path>) -> DeviceBuilder {
        Device::format(Format::Png, file)
    }

    /// Starts opening a device that draws nothing and writes no file.
    ///
    /// Everything works as on a PDF device of the same size: calls return
    /// the same values (`legend()`'s box, `axis()`'s ticks, `strwidth()`),
    /// and the same errors (margins too large for the page), but nothing is
    /// drawn or written, so it is also the fastest way to run drawing code.
    /// Use it to check that a [`Figure`](crate::recording::Figure) fits a
    /// page before opening a file, or to measure. It takes
    /// [`Device::pdf`]'s options (`width`, `height`, `units`, `pointsize`,
    /// `bg`); [`open`](DeviceBuilder::open) creates nothing, and
    /// [`close`](Device::close) succeeds without writing.
    ///
    /// This is R's `pdf(NULL)`.
    ///
    /// # Examples
    ///
    /// Does a figure fit a 1.5 x 1.5 inch page?
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut fig = Figure::new();
    /// fig.plot().x([1, 2, 3]).main("A").draw();
    ///
    /// let mut dev = Device::null().width(1.5).height(1.5).open()?;
    /// let e = dev.try_add_figure(&fig).unwrap_err();
    /// assert_eq!(e.kind(), ErrorKind::DoesNotFit); // figure margins too large
    /// dev.close()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn null() -> DeviceBuilder {
        let mut builder = Device::format(Format::Pdf, "");
        builder.target = Target::Null;
        builder
    }

    /// Starts opening a device whose output is kept in memory: get it with
    /// [`finish`](Device::finish) when done.
    ///
    /// `format` is a [`Format`] or its name (`"pdf"`, `"svg"`, `"png"`,
    /// ignoring case), and every option of [`Device::format`] works except
    /// the file ones (one file per page): the bytes are the last page for
    /// PNG and SVG, all pages for PDF. [`open`](DeviceBuilder::open) creates
    /// nothing, and [`close`](Device::close) is an error (there is no file).
    /// Use it to serve a plot, embed it, or write it yourself.
    ///
    /// R draws to files or screens; this has no R equivalent.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rs_graphics::prelude::*;
    /// # fn main() -> std::io::Result<()> {
    /// let mut dev = Device::memory("svg").width(5).height(4).open()?;
    /// dev.plot().x([1, 2, 3]).draw();
    /// let svg = String::from_utf8(dev.finish()).unwrap();
    /// assert!(svg.contains("<svg"));
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// A `format` string that names no format is an error of
    /// [`open`](DeviceBuilder::open).
    pub fn memory(format: impl IntoArg<Format>) -> DeviceBuilder {
        let mut builder = Device::format(format, "");
        builder.target = Target::Memory;
        builder
    }
}

#[cfg(test)]
#[path = "open_tests.rs"]
mod tests;
