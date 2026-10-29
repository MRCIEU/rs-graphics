// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Opening devices (`Device::pdf`/`svg`/`png`/`format`/`memory` and
//! `DeviceBuilder`): every option gives byte for byte the output of the
//! backend set up by hand, R's defaults apply when options are left out,
//! the format never comes from the file name, `open()` creates the file
//! (failing on a bad path), `close()` writes it (or fails for a device
//! without one), and an in-memory device gives the file's bytes.

use std::path::PathBuf;

use crate::backend::pdf::PdfBackend;
use crate::backend::raster::RasterBackend;
use crate::backend::svg::SvgBackend;
use crate::par::Par;
use crate::prelude::*;

/// The same drawing on any device: text (pointsize matters), a symbol
/// (`startps` matters) and the page background.
fn draw<B: Backend>(dev: &mut Device<B>) {
    dev.plot()
        .x([1.0, 2.0, 3.0])
        .y([3.0, 1.0, 2.0])
        .pch(19)
        .main("title")
        .draw();
}

fn opened(builder: DeviceBuilder) -> Vec<u8> {
    let mut dev = builder.open().unwrap();
    draw(&mut dev);
    dev.finish()
}

fn direct<B: Backend>(backend: B, par: Par) -> Vec<u8> {
    let mut dev = Device::with_par(backend, par);
    draw(&mut dev);
    dev.finish()
}

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("rs_graphics_device_open");
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

const R_PNG_SIZE: f64 = 480.0 / 72.0;

#[test]
fn defaults_are_rs() {
    assert_eq!(
        opened(Device::pdf(temp("a.pdf"))),
        direct(PdfBackend::new(7.0, 7.0), Par::default())
    );
    assert_eq!(
        opened(Device::svg(temp("a.svg"))),
        direct(SvgBackend::new(7.0, 7.0), Par::default())
    );
    assert_eq!(
        opened(Device::png(temp("a.png"))),
        direct(
            RasterBackend::new(R_PNG_SIZE, R_PNG_SIZE, 72.0, Color::WHITE),
            Par::default()
        )
    );
}

#[test]
fn size_pointsize_and_res() {
    let ps9 = || Par {
        ps: 9.0,
        ..Par::default()
    };
    assert_eq!(
        opened(
            Device::pdf(temp("a.pdf"))
                .width(5.0)
                .height(4.0)
                .pointsize(9.0)
        ),
        direct(PdfBackend::new(5.0, 4.0), ps9())
    );
    assert_eq!(
        opened(
            Device::svg(temp("a.svg"))
                .width(5.0)
                .height(4.0)
                .pointsize(9.0)
        ),
        direct(SvgBackend::new(5.0, 4.0), ps9())
    );
    assert_eq!(
        opened(
            Device::png(temp("a.png"))
                .width(5.0)
                .height(4.0)
                .res(150.0)
                .pointsize(9.0)
        ),
        direct(RasterBackend::new(5.0, 4.0, 150.0, Color::WHITE), ps9())
    );
}

#[test]
fn backgrounds() {
    let yellow = Color::try_from("yellow").unwrap();
    assert_eq!(
        opened(Device::pdf(temp("a.pdf")).bg("white")),
        direct(
            PdfBackend::new(7.0, 7.0).with_background(Color::WHITE),
            Par::default()
        )
    );
    assert_eq!(
        opened(Device::png(temp("a.png")).bg("yellow")),
        direct(
            RasterBackend::new(R_PNG_SIZE, R_PNG_SIZE, 72.0, yellow),
            Par::default()
        )
    );
    let svg_bg = |bg| Par {
        bg,
        ..Par::default()
    };
    assert_eq!(
        opened(Device::svg(temp("a.svg")).bg("yellow")),
        direct(SvgBackend::new(7.0, 7.0), svg_bg(yellow))
    );
    assert_eq!(
        opened(Device::svg(temp("a.svg")).bg("transparent")),
        direct(SvgBackend::new(7.0, 7.0), svg_bg(Color::TRANSPARENT))
    );
    assert_ne!(
        opened(Device::svg(temp("a.svg")).bg("transparent")),
        opened(Device::svg(temp("a.svg")))
    );
    assert_eq!(
        opened(Device::png(temp("a.png")).bg(ColorSpec::None)),
        direct(
            RasterBackend::new(R_PNG_SIZE, R_PNG_SIZE, 72.0, Color::TRANSPARENT),
            Par::default()
        )
    );
}

/// An explicit `.bg()` sets the starting background on every format, also
/// white (which the device would otherwise take as "not set" and replace
/// by the backend's default).
#[test]
fn explicit_background_is_kept() {
    for format in ["pdf", "svg", "png"] {
        for bg in ["white", "yellow"] {
            let dev = Device::format(format, temp(&format!("bg.{format}")))
                .bg(bg)
                .open()
                .unwrap();
            assert_eq!(
                dev.current_par().bg,
                Color::try_from(bg).unwrap(),
                "{format}, bg {bg}"
            );
        }
    }
}

#[test]
fn compress_and_res_apply_where_they_mean_something() {
    assert_eq!(
        opened(Device::pdf(temp("a.pdf")).compress(false)),
        direct(
            PdfBackend::new(7.0, 7.0).with_compression(false),
            Par::default()
        )
    );
    assert_ne!(
        opened(Device::pdf(temp("a.pdf")).compress(false)),
        opened(Device::pdf(temp("a.pdf")))
    );
    // Ignored by the other formats.
    assert_eq!(
        opened(Device::pdf(temp("a.pdf")).res(300.0)),
        opened(Device::pdf(temp("a.pdf")))
    );
    assert_eq!(
        opened(Device::svg(temp("a.svg")).res(300.0)),
        opened(Device::svg(temp("a.svg")))
    );
    assert_eq!(
        opened(Device::svg(temp("a.svg")).compress(false)),
        opened(Device::svg(temp("a.svg")))
    );
    // For SVG, `compress(true)` gzips the document (`.svgz`).
    assert_eq!(
        opened(Device::svg(temp("a.svg")).compress(true)),
        direct(SvgBackend::new(7.0, 7.0).with_gzip(true), Par::default())
    );
    assert_eq!(
        opened(Device::png(temp("a.png")).compress(false)),
        opened(Device::png(temp("a.png")))
    );
}

#[test]
fn format_by_name_or_value() {
    assert_eq!(
        opened(Device::format("png", temp("a.png"))),
        opened(Device::png(temp("a.png")))
    );
    assert_eq!(
        opened(Device::format("PDF", temp("a.pdf"))),
        opened(Device::pdf(temp("a.pdf")))
    );
    assert_eq!(
        opened(Device::format(Format::Svg, temp("a.svg"))),
        opened(Device::svg(temp("a.svg")))
    );
}

#[test]
fn unknown_format_name_fails_at_open() {
    let path = temp("unknown_format.jpg");
    let _ = std::fs::remove_file(&path);
    let e = Device::format("jpg", &path).open().err().unwrap();
    assert_eq!(e.kind(), ErrorKind::InvalidArgument);
    assert_eq!(e.call(), "pdf");
    assert!(
        e.message()
            .starts_with("\"jpg\" is not a valid output format"),
        "{e}"
    );
    assert!(!path.exists());
}

#[test]
fn close_writes_the_finished_file_whatever_its_extension() {
    for (format, name) in [("pdf", "out.png"), ("svg", "out.pdf"), ("png", "out.svg")] {
        let path = temp(name);
        let _ = std::fs::remove_file(&path);
        let mut a = Device::format(format, &path).open().unwrap();
        draw(&mut a);
        a.close().unwrap();
        // (Opening `path` again would empty it, so the reference device
        // gets a file of its own.)
        let reference = opened(Device::format(format, temp(&format!("ref_{name}"))));
        assert_eq!(
            std::fs::read(&path).unwrap(),
            reference,
            "{format} to {name}"
        );
    }
    // The bytes are a PDF although the name says PNG, as R's pdf("x.png").
    assert!(std::fs::read(temp("out.png")).unwrap().starts_with(b"%PDF"));
}

#[test]
fn close_without_a_file_fails() {
    let dev = Device::memory("pdf").width(7.0).height(7.0).open().unwrap();
    let err = dev.close().unwrap_err();
    assert_eq!(err.kind(), ErrorKind::InvalidArgument);
    assert_eq!(err.call(), "dev.off");
}

#[test]
fn open_creates_the_file_and_close_fills_it() {
    let path = temp("created.pdf");
    std::fs::write(&path, b"old contents").unwrap();
    let mut dev = Device::pdf(&path).open().unwrap();
    // Created (emptied) at open, as R's pdf(); written at close.
    assert_eq!(std::fs::read(&path).unwrap(), b"");
    draw(&mut dev);
    dev.close().unwrap();
    assert!(std::fs::read(&path).unwrap().starts_with(b"%PDF"));

    // Dropped without close(): the file stays empty.
    let path = temp("dropped.pdf");
    let mut dev = Device::pdf(&path).open().unwrap();
    draw(&mut dev);
    drop(dev);
    assert_eq!(std::fs::read(&path).unwrap(), b"");
}

#[test]
fn open_fails_on_a_bad_path_and_names_it() {
    let path = temp("no_such_directory").join("plot.pdf");
    let err = Device::pdf(&path).open().err().expect("open should fail");
    assert_eq!(err.kind(), ErrorKind::Io);
    assert_eq!(err.call(), "pdf");
    assert_eq!(err.io_error().unwrap().kind(), std::io::ErrorKind::NotFound);
    assert!(err.to_string().contains("no_such_directory"), "{err}");
    // As an `io::Error` (for `?` in `io::Result` functions) it keeps the
    // I/O kind and the message.
    let io: std::io::Error = err.clone().into();
    assert_eq!(io.kind(), std::io::ErrorKind::NotFound);
    assert_eq!(io.to_string(), err.to_string());
    assert!(!path.exists());
}

#[test]
fn units_convert_sizes_to_inches() {
    // 12.7 cm = 101.6 mm = 5 in; 4 in the other way.
    assert_eq!(
        opened(
            Device::pdf(temp("a.pdf"))
                .width(12.7)
                .height(10.16)
                .units("cm")
        ),
        direct(PdfBackend::new(5.0, 4.0), Par::default())
    );
    assert_eq!(
        opened(
            Device::svg(temp("a.svg"))
                .width(127)
                .height(101.6)
                .units("mm")
        ),
        direct(SvgBackend::new(5.0, 4.0), Par::default())
    );
    assert_eq!(
        opened(
            Device::png(temp("a.png"))
                .width(5)
                .height(4)
                .units(Units::Inches)
        ),
        opened(Device::png(temp("a.png")).width(5).height(4))
    );
    // Pixels are divided by `res`: R's png(width = 750, height = 600, res = 150).
    assert_eq!(
        opened(
            Device::png(temp("a.png"))
                .width(750)
                .height(600)
                .units("px")
                .res(150)
        ),
        direct(
            RasterBackend::new(5.0, 4.0, 150.0, Color::WHITE),
            Par::default()
        )
    );
    // ... at 72 ppi by default, so 480 px is R's default page.
    assert_eq!(
        opened(
            Device::png(temp("a.png"))
                .width(480)
                .height(480)
                .units("px")
        ),
        opened(Device::png(temp("a.png")))
    );
}

#[test]
fn units_leave_the_default_size_alone() {
    for units in ["in", "cm", "mm"] {
        assert_eq!(
            opened(Device::pdf(temp("a.pdf")).units(units)),
            opened(Device::pdf(temp("a.pdf")))
        );
    }
    assert_eq!(
        opened(Device::png(temp("a.png")).units("px").res(150)),
        opened(Device::png(temp("a.png")).res(150))
    );
}

#[test]
fn pixels_need_a_png_device() {
    for format in ["pdf", "svg"] {
        let path = temp(&format!("px.{format}"));
        let _ = std::fs::remove_file(&path);
        let err = Device::format(format, &path)
            .width(480)
            .units("px")
            .open()
            .err()
            .expect("open should fail");
        assert_eq!(err.kind(), ErrorKind::InvalidArgument);
        assert!(err.to_string().contains("px"), "{err}");
        assert!(!path.exists());
    }
}

#[test]
fn units_parse_like_match_arg() {
    assert_eq!("px".parse::<Units>().ok(), Some(Units::Pixels));
    assert_eq!("i".parse::<Units>().ok(), Some(Units::Inches));
    assert_eq!("c".parse::<Units>().ok(), Some(Units::Centimetres));
    assert_eq!("m".parse::<Units>().ok(), Some(Units::Millimetres));
    assert_eq!("".parse::<Units>().ok(), None);
    assert_eq!("pt".parse::<Units>().ok(), None);
    assert_eq!("mm".parse::<Units>(), Ok(Units::Millimetres));
}

#[test]
fn unknown_units_and_colours_fail_at_open() {
    let path = temp("unknown_units.pdf");
    let _ = std::fs::remove_file(&path);
    let e = Device::pdf(&path).units("pt").open().err().unwrap();
    assert_eq!(e.kind(), ErrorKind::InvalidArgument);
    assert!(
        e.message().starts_with("\"pt\" is not a valid units"),
        "{e}"
    );
    let e = Device::pdf(&path).bg("notacolour").open().err().unwrap();
    assert_eq!(e.to_string(), "pdf(): invalid color name 'notacolour'");
    assert!(!path.exists());
}

#[test]
fn memory_devices_give_the_files_bytes() {
    for format in ["pdf", "svg", "png"] {
        let builder = |b: DeviceBuilder| b.width(5).height(4).pointsize(10).bg("ivory");
        assert_eq!(
            opened(builder(Device::memory(format))),
            opened(builder(Device::format(
                format,
                temp(&format!("m.{format}"))
            ))),
            "{format}"
        );
    }
    // No file is created, and closing is an error (use `finish()`).
    let dev = Device::memory("png").open().unwrap();
    assert_eq!(dev.close().unwrap_err().kind(), ErrorKind::InvalidArgument);
}

#[test]
fn threads_reach_the_png_backend() {
    assert_eq!(
        opened(Device::memory("png").width(5).height(4).threads(3)),
        direct(
            RasterBackend::new(5.0, 4.0, 72.0, Color::WHITE).with_threads(3),
            Par::default()
        )
    );
    // Ignored by the other formats.
    assert_eq!(
        opened(Device::memory("pdf").threads(3)),
        opened(Device::memory("pdf"))
    );
}
