//! `pie(x, ...)`, ported from `r-source/src/library/graphics/R/pie.R`.

use std::f64::consts::PI;

use crate::backend::Backend;
use crate::color::{Color, ColorSpec};
use crate::device::{
    Device, PlotWindowOptions, PolygonOptions, TextOptions, TitleOptions, XyOptions,
};
use crate::plotmath::Label;

/// R's default pie slice color cycle (used when `col` isn't given).
const DEFAULT_COLORS: [&str; 6] = [
    "white",
    "lightblue",
    "mistyrose",
    "lightcyan",
    "lavender",
    "cornsilk",
];

/// Options for [`pie`], covering the subset of `pie()`'s arguments this
/// crate implements.
#[derive(Clone, Debug)]
pub struct PieOptions {
    pub labels: Option<Vec<String>>,
    /// Pie radius in user units, where the plot region's shorter side
    /// spans `[-1, 1]` before the usual 4% axis extension. R's default is
    /// `0.8`.
    pub radius: f64,
    /// Slice fill colors, cycled if shorter than `x`. Defaults to R's
    /// built-in 6-color cycle if `None`.
    pub col: Option<Vec<ColorSpec>>,
    pub border: ColorSpec,
    /// Title, as for `plot()` (text or a plotmath expression).
    pub main: Option<Label>,
    /// Approximate number of polygon edges for a full circle. R's
    /// default is `200`.
    pub edges: usize,
    pub clockwise: bool,
    /// Starting angle in degrees (R's `init.angle`). `None`: R's default,
    /// 90 (12 o'clock) when `clockwise`, else 0 (3 o'clock).
    pub init_angle_deg: Option<f64>,
}

crate::setters::setters!(PieOptions {
    labels: strings String,
    radius: value f64,
    col: option_values ColorSpec,
    border: value ColorSpec,
    main: option Label,
    edges: value usize,
    clockwise: value bool,
    init_angle_deg: option f64,
});

impl Default for PieOptions {
    fn default() -> Self {
        PieOptions {
            labels: None,
            radius: 0.8,
            col: None,
            border: ColorSpec::Index(1),
            main: None,
            edges: 200,
            clockwise: false,
            init_angle_deg: None,
        }
    }
}

/// `pie(x, ...)`.
pub fn pie<B: Backend>(dev: &mut Device<B>, x: &[f64], opts: &PieOptions) {
    let nx = x.len();
    assert!(
        nx > 0 && x.iter().all(|&v| v.is_finite() && v >= 0.0),
        "pie: 'x' values must be positive"
    );
    let total: f64 = x.iter().sum();

    let labels: Vec<String> = opts
        .labels
        .clone()
        .unwrap_or_else(|| (1..=nx).map(|i| i.to_string()).collect());

    dev.plot_new();

    // Pre-scale xlim/ylim by the plot region's aspect ratio, then
    // `plot.window(xlim, ylim, asp = 1)` (with the current `xaxs`/`yaxs`,
    // i.e. R's default 4% extension), exactly as `pie.R` does.
    let plt = dev.plot_region();
    let (w, h) = (plt.width(), plt.height());
    let (xlim, ylim) = if w > h {
        ((-(w / h), w / h), (-1.0, 1.0))
    } else {
        ((-1.0, 1.0), (-(h / w), h / w))
    };
    let window = PlotWindowOptions {
        asp: Some(1.0),
        ..PlotWindowOptions::default()
    };
    dev.plot_window(xlim, ylim, &window);

    let twopi = if opts.clockwise { -2.0 * PI } else { 2.0 * PI };
    let init = opts
        .init_angle_deg
        .unwrap_or(if opts.clockwise { 90.0 } else { 0.0 })
        .to_radians();
    let t2xy = |t: f64| -> (f64, f64) {
        let tp = twopi * t + init;
        (opts.radius * tp.cos(), opts.radius * tp.sin())
    };

    let mut cum = Vec::with_capacity(nx + 1);
    cum.push(0.0);
    // `c(0, cumsum(x) / sum(x))`: divide the running sum (not each term),
    // so slice fractions - and hence edge counts - round exactly as in R.
    let mut acc = 0.0;
    for &v in x {
        acc += v;
        cum.push(acc / total);
    }

    for i in 0..nx {
        let dxi = cum[i + 1] - cum[i];
        let n = (2usize).max((opts.edges as f64 * dxi).floor() as usize);
        let mut xs = Vec::with_capacity(n + 1);
        let mut ys = Vec::with_capacity(n + 1);
        for k in 0..n {
            let t = if n > 1 {
                cum[i] + dxi * (k as f64) / (n as f64 - 1.0)
            } else {
                cum[i]
            };
            let (px, py) = t2xy(t);
            xs.push(px);
            ys.push(py);
        }
        xs.push(0.0);
        ys.push(0.0);

        let col = match &opts.col {
            Some(cols) if !cols.is_empty() => cols[i % cols.len()],
            _ => ColorSpec::Color(Color::parse(DEFAULT_COLORS[i % DEFAULT_COLORS.len()]).unwrap()),
        };
        let slice = PolygonOptions {
            col: vec![col],
            border: vec![opts.border],
            ..PolygonOptions::default()
        };
        dev.polygon(&xs, &ys, &slice);

        let mid = 0.5 * (cum[i] + cum[i + 1]);
        let (px, py) = t2xy(mid);
        let label = &labels[i];
        if !label.is_empty() {
            dev.lines(&[px, 1.05 * px], &[py, 1.05 * py], &XyOptions::default());
            // `text(1.1 * P$x, 1.1 * P$y, labels[i], xpd = TRUE, adj =
            // ifelse(P$x < 0, 1, 0))`.
            let at = TextOptions {
                adj: Some(if px < 0.0 { 1.0 } else { 0.0 }),
                xpd: Some(true),
                ..TextOptions::default()
            };
            dev.text(&[1.1 * px], &[1.1 * py], &[label.as_str()], &at);
        }
    }

    if let Some(main) = &opts.main {
        dev.title(&TitleOptions {
            main: Some(main.clone()),
            ..TitleOptions::default()
        });
    }
}
