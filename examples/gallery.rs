//! Renders a small gallery of each supported high-level plot type to
//! SVG, PDF, and PNG, so the output can be visually compared against real
//! R (see the companion `examples/gen_reference.R` script, which
//! produces the equivalent plots with actual R for side-by-side
//! comparison).
//!
//! Run with `cargo run --example gallery`; output lands in
//! `gallery_output/` (relative to the crate root).

use std::fs;

use rs_graphics::backend::pdf::PdfBackend;
use rs_graphics::backend::raster::RasterBackend;
use rs_graphics::backend::svg::SvgBackend;
use rs_graphics::backend::Backend;
use rs_graphics::color::{Color, ColorSpec};
use rs_graphics::device::Device;
use rs_graphics::highlevel::{barplot, boxplot, hist, pie, plot};
use rs_graphics::highlevel::{
    BarplotOptions, BoxplotOptions, HistOptions, PieOptions, PlotOptions,
};
use rs_graphics::par::Par;

/// A tiny seeded PRNG (splitmix64-style LCG) so the gallery's sample data
/// is reproducible without pulling in an external `rand` dependency.
struct Rng(u64);

impl Rng {
    fn next_f64(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.0 >> 11) as f64) / ((1u64 << 53) as f64)
    }

    fn gaussian(&mut self) -> f64 {
        let u1 = self.next_f64().max(1e-12);
        let u2 = self.next_f64();
        (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
    }
}

/// Lets [`render`] construct any of the three backends generically from
/// just a page size.
trait NewBackend: Backend + Sized {
    fn create(width_in: f64, height_in: f64) -> Self;
}

impl NewBackend for SvgBackend {
    fn create(width_in: f64, height_in: f64) -> Self {
        SvgBackend::new(width_in, height_in)
    }
}

impl NewBackend for PdfBackend {
    fn create(width_in: f64, height_in: f64) -> Self {
        PdfBackend::new(width_in, height_in)
    }
}

impl NewBackend for RasterBackend {
    fn create(width_in: f64, height_in: f64) -> Self {
        RasterBackend::new(width_in, height_in, 150.0, Color::WHITE)
    }
}

fn render<B: NewBackend>(width_in: f64, height_in: f64, draw: fn(&mut Device<B>)) -> B {
    let backend = B::create(width_in, height_in);
    let mut dev = Device::new(backend, Par::default());
    draw(&mut dev);
    dev.into_backend()
}

/// Renders `draw` to all three backends and writes
/// `gallery_output/<name>.{svg,pdf,png}`. `draw` must be a bare `fn`
/// item (not a closure), so it can be instantiated once per backend
/// type via [`render`].
macro_rules! save_gallery_plot {
    ($name:expr, $w:expr, $h:expr, $draw:ident) => {{
        let svg = render::<SvgBackend>($w, $h, $draw);
        fs::write(format!("gallery_output/{}.svg", $name), svg.finish()).expect("write svg");

        let pdf = render::<PdfBackend>($w, $h, $draw);
        fs::write(format!("gallery_output/{}.pdf", $name), pdf.finish()).expect("write pdf");

        let png = render::<RasterBackend>($w, $h, $draw);
        fs::write(format!("gallery_output/{}.png", $name), png.finish()).expect("write png");

        println!("wrote gallery_output/{}.[svg|pdf|png]", $name);
    }};
}

fn draw_scatter<B: Backend>(dev: &mut Device<B>) {
    let mut rng = Rng(42);
    let x: Vec<f64> = (1..=25).map(|i| i as f64).collect();
    let y: Vec<f64> = x
        .iter()
        .map(|&xi| (xi * 0.35).sin() * 8.0 + xi * 0.6 + rng.gaussian() * 1.5)
        .collect();

    let opts = PlotOptions {
        pch: 19,
        col: ColorSpec::Index(4),
        main: Some("plot(): scatter"),
        xlab: Some("x"),
        ylab: Some("y"),
        ..PlotOptions::default()
    };
    plot(dev, &x, &y, &opts);
}

fn draw_lineplot<B: Backend>(dev: &mut Device<B>) {
    let x: Vec<f64> = (0..=40).map(|i| i as f64 * 0.25).collect();
    let y: Vec<f64> = x.iter().map(|&xi| xi.sin()).collect();

    let opts = PlotOptions {
        type_: rs_graphics::highlevel::PlotType::Lines,
        col: ColorSpec::Index(2),
        lwd: 2.0,
        main: Some("plot(): line"),
        xlab: Some("x"),
        ylab: Some("sin(x)"),
        ..PlotOptions::default()
    };
    plot(dev, &x, &y, &opts);
}

fn draw_hist<B: Backend>(dev: &mut Device<B>) {
    let mut rng = Rng(7);
    let data: Vec<f64> = (0..300).map(|_| rng.gaussian() * 2.0 + 10.0).collect();

    let opts = HistOptions {
        main: Some("hist(): approximately normal data"),
        ..HistOptions::default()
    };
    hist(dev, &data, &opts);
}

fn draw_barplot<B: Backend>(dev: &mut Device<B>) {
    let heights = vec![23.0, 41.0, 12.0, 35.0, 28.0];
    let names = vec!["A", "B", "C", "D", "E"]
        .into_iter()
        .map(String::from)
        .collect();

    let opts = BarplotOptions {
        names: Some(names),
        main: Some("barplot()"),
        ylab: Some("value"),
        ..BarplotOptions::default()
    };
    barplot(dev, &heights, &opts);
}

fn draw_boxplot<B: Backend>(dev: &mut Device<B>) {
    let mut rng = Rng(99);
    let mut group = |n: usize, mean: f64, sd: f64, outliers: &[f64]| -> Vec<f64> {
        let mut v: Vec<f64> = (0..n).map(|_| mean + rng.gaussian() * sd).collect();
        v.extend_from_slice(outliers);
        v
    };
    let groups = vec![
        group(40, 10.0, 2.0, &[20.0]),
        group(40, 14.0, 3.0, &[]),
        group(40, 9.0, 1.5, &[2.0, 16.0]),
    ];
    let names = vec!["Ctrl", "Treat A", "Treat B"]
        .into_iter()
        .map(String::from)
        .collect();

    let opts = BoxplotOptions {
        names: Some(names),
        main: Some("boxplot()"),
        ylab: Some("value"),
        ..BoxplotOptions::default()
    };
    boxplot(dev, &groups, &opts);
}

fn draw_pie<B: Backend>(dev: &mut Device<B>) {
    let values = vec![18.0, 25.0, 12.0, 30.0, 15.0];
    let labels = vec!["Alpha", "Beta", "Gamma", "Delta", "Epsilon"]
        .into_iter()
        .map(String::from)
        .collect();

    let opts = PieOptions {
        labels: Some(labels),
        main: Some("pie()"),
        ..PieOptions::default()
    };
    pie(dev, &values, &opts);
}

fn main() {
    fs::create_dir_all("gallery_output").expect("create gallery_output/");

    save_gallery_plot!("plot_scatter", 6.0, 4.0, draw_scatter);
    save_gallery_plot!("plot_line", 6.0, 4.0, draw_lineplot);
    save_gallery_plot!("hist", 6.0, 4.0, draw_hist);
    save_gallery_plot!("barplot", 6.0, 4.0, draw_barplot);
    save_gallery_plot!("boxplot", 6.0, 4.0, draw_boxplot);
    save_gallery_plot!("pie", 6.0, 6.0, draw_pie);

    println!("\nGallery complete. See gallery_output/ for SVG/PDF/PNG output.");
}
