//! `par()` state: the subset of R's graphics parameters this crate
//! implements, with R's exact default values (cross-checked against
//! `Rscript -e 'str(par())'` and `r-source/src/library/graphics/src/par.c`).
//!
//! `par(cex)` is R's `cexbase` (assigning it resets the per-call `cex` to
//! 1), so `cex` here scales both text *and* the "line" unit that margins
//! are measured in, exactly as `mapNDC2Dev()` does (`yNDCPerLine = mex *
//! cexbase * cra[1] / ...`). Multi-panel layouts
//! (`Device::mfrow()`/`mfcol()`/`layout()`) set `cex` (0.83 for 2 x 2, 0.66
//! beyond that) and `mex` just as R's `par(mfrow=)` does.

use crate::afm::FontFace;
use crate::color::{Color, ColorSpec, DEFAULT_PALETTE};
use crate::lty::LineType;

use super::backend::{LineEnd, LineJoin};

/// The four margin/rectangle sides in R's `mar`/`oma`/`fig`/`plt`
/// convention: bottom, left, top, right.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Sides<T> {
    pub bottom: T,
    pub left: T,
    pub top: T,
    pub right: T,
}

impl<T: Copy> Sides<T> {
    pub const fn new(bottom: T, left: T, top: T, right: T) -> Self {
        Sides {
            bottom,
            left,
            top,
            right,
        }
    }
}

/// Axis style (`par(xaxs=)`/`par(yaxs=)`). Only `"r"` (the default, 4%
/// range expansion) and `"i"` (exact, no expansion) are implemented; R's
/// `"s"`/`"e"`/`"d"` styles are treated as `"r"` (matching R's own
/// fallback, since only S and few other historical devices ever
/// implemented them differently - R's engine treats all non-`"i"` styles
/// like `"r"`).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum AxisStyle {
    Regular,
    Exact,
}

impl AxisStyle {
    pub fn parse(s: &str) -> AxisStyle {
        match s {
            "i" => AxisStyle::Exact,
            _ => AxisStyle::Regular,
        }
    }
}

/// `par(pty)`: the shape of the plot region when it comes from `mar`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Pty {
    /// `"m"`: as large as the margins allow (the default).
    Maximal,
    /// `"s"`: square, the longer side shrunk about its centre.
    Square,
}

impl Pty {
    pub fn parse(s: &str) -> Pty {
        match s {
            "s" => Pty::Square,
            _ => Pty::Maximal,
        }
    }
}

/// `par(xaxt/yaxt)`: whether `axis()` draws on that axis. R's `"s"`, `"l"`
/// and `"t"` all mean [`AxisType::Standard`]; `"n"` suppresses the axis
/// (`plot(x, y, xaxt = "n")` before a custom `axis(1, ...)`).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum AxisType {
    Standard,
    None,
}

impl AxisType {
    pub fn parse(s: &str) -> AxisType {
        match s {
            "n" => AxisType::None,
            _ => AxisType::Standard,
        }
    }
}

/// The graphics parameter set (`par()`), restricted to what this crate
/// implements. Field names match R's own where practical.
#[derive(Clone, Debug)]
pub struct Par {
    /// Margins in lines: bottom, left, top, right. Default `c(5.1, 4.1,
    /// 4.1, 2.1)`.
    pub mar: Sides<f64>,
    /// Outer margins in lines. Default `c(0, 0, 0, 0)`.
    pub oma: Sides<f64>,
    /// `par(plt = c(x0, x1, y0, y1))`: the plot region as fractions of the
    /// figure region, instead of the figure less `mar`. `None` (the default)
    /// uses `mar` and `pty`. Unlike R, setting `mar` or `pty` doesn't reset
    /// it: set it back to `None`. Applied at the next `plot_new()`.
    pub plt: Option<[f64; 4]>,
    /// `par(pty)`: [`Pty::Square`] makes the `mar`-based plot region
    /// square. Applied at the next `plot_new()`.
    pub pty: Pty,
    pub xaxs: AxisStyle,
    pub yaxs: AxisStyle,
    pub xlog: bool,
    pub ylog: bool,

    pub col: Color,
    pub bg: Color,
    pub fg: Color,
    pub col_axis: Color,
    pub col_lab: Color,
    pub col_main: Color,
    pub col_sub: Color,

    pub palette: Vec<Color>,

    pub lwd: f64,
    pub lty: LineType,
    pub lend: LineEnd,
    pub ljoin: LineJoin,
    pub lmitre: f64,

    pub pch: i32,

    /// Base point size, in big points (`par(ps)`). Default `12`. Its value
    /// at `Device::new` is the device's starting size (R's
    /// `pdf(pointsize = )`), from which margin lines and symbols are
    /// sized; changing it later only changes text size, as in R.
    pub ps: f64,
    /// `par(cex)`, i.e. R's `cexbase`. Also scales the margin "line"
    /// unit (see [`Device::line_height_inches`](crate::device::Device::line_height_inches)).
    pub cex: f64,
    /// `par(mex)`: margin-line expansion factor (a multiple of `cex`).
    pub mex: f64,
    pub cex_axis: f64,
    pub cex_lab: f64,
    pub cex_main: f64,
    pub cex_sub: f64,

    pub font: i32,
    pub font_axis: i32,
    pub font_lab: i32,
    pub font_main: i32,
    pub font_sub: i32,

    /// Label orientation (`par(las)`): 0 = parallel to axis, 1 =
    /// horizontal, 2 = perpendicular to axis, 3 = vertical.
    pub las: i32,
    /// `par(xaxt)` / `par(yaxt)`: [`AxisType::None`] stops `axis()`
    /// drawing on the x (sides 1, 3) / y (sides 2, 4) axis.
    pub xaxt: AxisType,
    pub yaxt: AxisType,
    /// Tick mark length in margin lines (negative = outward; R's default
    /// is -0.5).
    pub tcl: f64,
    /// Margin line positions for axis title/labels/line, `c(3, 1, 0)` by
    /// default.
    pub mgp: [f64; 3],
    /// `par(lab)`: the approximate number of tick intervals on the x and y
    /// axes for default ticks (the third value, label length, is unused,
    /// as in R). `c(5, 5, 7)` by default.
    pub lab: [i32; 3],
    /// `par(xaxp)` / `par(yaxp)`: the first and last default tick and the
    /// number of intervals between them (on a log axis, a tick-rule code
    /// instead: see [`crate::pretty::axis_pars`]). `plot_window()` sets
    /// them from the axis range and `par(lab)`; change them afterwards to
    /// change the ticks `axis()` draws by default. `c(0, 1, 5)` initially.
    pub xaxp: [f64; 3],
    pub yaxp: [f64; 3],
    pub lheight: f64,
    /// Default text justification (`par(adj)`).
    pub adj: f64,
    pub xpd: bool,
    pub ann: bool,
}

impl Default for Par {
    fn default() -> Self {
        Par {
            mar: Sides::new(5.1, 4.1, 4.1, 2.1),
            oma: Sides::new(0.0, 0.0, 0.0, 0.0),
            plt: None,
            pty: Pty::Maximal,
            xaxs: AxisStyle::Regular,
            yaxs: AxisStyle::Regular,
            xlog: false,
            ylog: false,

            col: Color::BLACK,
            bg: Color::WHITE,
            fg: Color::BLACK,
            col_axis: Color::BLACK,
            col_lab: Color::BLACK,
            col_main: Color::BLACK,
            col_sub: Color::BLACK,

            palette: DEFAULT_PALETTE.to_vec(),

            lwd: 1.0,
            lty: LineType::Pattern(&[]),
            lend: LineEnd::Round,
            ljoin: LineJoin::Round,
            lmitre: 10.0,

            pch: 1,

            ps: 12.0,
            cex: 1.0,
            mex: 1.0,
            cex_axis: 1.0,
            cex_lab: 1.0,
            cex_main: 1.2,
            cex_sub: 1.0,

            font: 1,
            font_axis: 1,
            font_lab: 1,
            font_main: 2,
            font_sub: 1,

            las: 0,
            xaxt: AxisType::Standard,
            yaxt: AxisType::Standard,
            tcl: -0.5,
            mgp: [3.0, 1.0, 0.0],
            lab: [5, 5, 7],
            xaxp: [0.0, 1.0, 5.0],
            yaxp: [0.0, 1.0, 5.0],
            lheight: 1.0,
            adj: 0.5,
            xpd: false,
            ann: true,
        }
    }
}

impl Par {
    /// The colour to draw `spec` with, or `None` for nothing. Colour 0 is
    /// the background, `par.bg`, as in R (`RGBpar3`); like any fully
    /// transparent colour it draws nothing when `bg` is transparent (the
    /// PDF default).
    pub fn resolve_color(&self, spec: ColorSpec) -> Option<Color> {
        match spec {
            ColorSpec::Index(0) => Some(self.bg).filter(|c| c.a > 0),
            _ => spec.resolve(&self.palette),
        }
    }

    pub fn font_face(&self, code: i32) -> FontFace {
        FontFace::from_r_font_code(code)
    }
}
