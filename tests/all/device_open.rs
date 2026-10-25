// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Opening devices for files (`Device::pdf`/`svg`/`png`/`format` and
//! `DeviceBuilder`): every option gives byte for byte the output of the
//! equivalent `Device::new` route, R's defaults apply when options are
//! left out, the format never comes from the file name, `open()` creates
//! the file (failing on a bad path), and `close()` writes it (or fails for
//! a device without one).

use std::path::PathBuf;

use rs_graphics::prelude::*;

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
    let mut dev = Device::new(backend, par);
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
    let yellow = Color::parse("yellow").unwrap();
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

/// An explicit `.bg()` is the device's `par(bg)` on every format, also
/// white (which `Device::new` would otherwise take as "not set" and replace
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
                Color::parse(bg).unwrap(),
                "{format}, bg {bg}"
            );
        }
    }
}

#[test]
fn compress_is_pdf_only_and_res_png_only() {
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
#[should_panic(expected = "\"jpg\" is not a valid output format")]
fn unknown_format_name_panics() {
    let _ = Device::format("jpg", temp("a.jpg"));
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
    let dev = Device::new(PdfBackend::new(7.0, 7.0), Par::default());
    let err = dev.close().unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
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
    assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
    assert!(err.to_string().contains("no_such_directory"), "{err}");
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
                .units(Units::In)
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
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
        assert!(err.to_string().contains("px"), "{err}");
        assert!(!path.exists());
    }
}

#[test]
fn units_parse_like_match_arg() {
    assert_eq!(Units::parse("px"), Some(Units::Px));
    assert_eq!(Units::parse("i"), Some(Units::In));
    assert_eq!(Units::parse("c"), Some(Units::Cm));
    assert_eq!(Units::parse("m"), Some(Units::Mm));
    assert_eq!(Units::parse(""), None);
    assert_eq!(Units::parse("pt"), None);
    assert_eq!("mm".parse::<Units>(), Ok(Units::Mm));
}

#[test]
#[should_panic(expected = "\"pt\" is not a valid units")]
fn unknown_units_panic() {
    let _ = Device::pdf(temp("a.pdf")).units("pt");
}
