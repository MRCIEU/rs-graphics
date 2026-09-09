//! `pie(x, ...)`, ported from `r-source/src/library/graphics/R/pie.R`.

use std::f64::consts::PI;

use crate::backend::Backend;
use crate::color::{Color, ColorSpec};
use crate::device::Device;
use crate::lty::LineType;
use crate::par::AxisStyle;

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
pub struct PieOptions<'a> {
    pub labels: Option<Vec<String>>,
    /// Pie radius, as a fraction of the plot region half-extent. R's
    /// default is `0.8`.
    pub radius: f64,
    /// Slice fill colors, cycled if shorter than `x`. Defaults to R's
    /// built-in 6-color cycle if `None`.
    pub col: Option<Vec<ColorSpec>>,
    pub border: ColorSpec,
    pub main: Option<&'a str>,
    /// Approximate number of polygon edges for a full circle. R's
    /// default is `200`.
    pub edges: usize,
    pub clockwise: bool,
    pub init_angle_deg: f64,
}

impl<'a> Default for PieOptions<'a> {
    fn default() -> Self {
        PieOptions {
            labels: None,
            radius: 0.8,
            col: None,
            border: ColorSpec::Index(1),
            main: None,
            edges: 200,
            clockwise: false,
            init_angle_deg: 0.0,
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

    // Adjust xlim/ylim so the circle isn't distorted by a non-square
    // plot region (matches `pie()`'s manual `pin`-ratio pre-scaling +
    // `plot.window(..., asp = 1)`).
    let plt = dev.plot_region();
    let (w, h) = (plt.width(), plt.height());
    let (xlim, ylim) = if w > h {
        ((-(w / h), w / h), (-1.0, 1.0))
    } else {
        ((-1.0, 1.0), (-(h / w), h / w))
    };
    let (saved_xaxs, saved_yaxs) = (dev.par.xaxs, dev.par.yaxs);
    dev.par.xaxs = AxisStyle::Exact;
    dev.par.yaxs = AxisStyle::Exact;
    dev.plot_window(xlim, ylim, false, false);
    dev.par.xaxs = saved_xaxs;
    dev.par.yaxs = saved_yaxs;

    let twopi = if opts.clockwise { -2.0 * PI } else { 2.0 * PI };
    let init = opts.init_angle_deg.to_radians();
    let t2xy = |t: f64| -> (f64, f64) {
        let tp = twopi * t + init;
        (opts.radius * tp.cos(), opts.radius * tp.sin())
    };

    let mut cum = Vec::with_capacity(nx + 1);
    cum.push(0.0);
    let mut acc = 0.0;
    for &v in x {
        acc += v / total;
        cum.push(acc);
    }

    let lwd = dev.par.lwd;
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
        dev.polygon(&xs, &ys, col, opts.border, lwd, LineType::Pattern(&[]));

        let mid = 0.5 * (cum[i] + cum[i + 1]);
        let (px, py) = t2xy(mid);
        let label = &labels[i];
        if !label.is_empty() {
            dev.lines(
                &[px, 1.05 * px],
                &[py, 1.05 * py],
                crate::device::LineStyle::new(
                    ColorSpec::Color(dev.par.col),
                    lwd,
                    LineType::Pattern(&[]),
                ),
            );
            let hadj = if px < 0.0 { 1.0 } else { 0.0 };
            let (saved_xpd, col_lab, font) = (dev.par.xpd, dev.par.col, dev.par.font);
            dev.par.xpd = true;
            dev.text(
                1.1 * px,
                1.1 * py,
                label,
                hadj,
                0.5,
                0.0,
                ColorSpec::Color(col_lab),
                1.0,
                font,
            );
            dev.par.xpd = saved_xpd;
        }
    }

    if let Some(main) = opts.main {
        dev.title(Some(main), None, None, None);
    }
}
