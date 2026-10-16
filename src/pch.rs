//! R's built-in plotting characters (`par(pch=)` values `0..=25`), ported
//! from `GESymbol()` in `r-source/src/main/engine.c`.
//!
//! R computes symbol geometry in inches internally (via
//! `toDeviceWidth/Height(.., GE_INCHES, dd)`, which is a no-op on a device
//! whose native units already are inches - true of our `Backend` trait),
//! so this module works directly in inches and needs no unit conversion.
//!
//! The nominal symbol "size" R passes to `GESymbol()` is (see `GSTR_0` in
//! `graphics.c`, specialised to a device with `cra[1] = 1.2 * pointsize`
//! and `ipr[1] = 1/72`, true of `pdf()`/`postscript()`):
//! `size_inches = pointsize * cex / 120`.

/// Simple filled/stroked shapes a pch symbol decomposes into.
#[derive(Clone, Debug)]
pub enum Shape {
    Circle { cx: f64, cy: f64, r: f64 },
    Rect { x0: f64, y0: f64, x1: f64, y1: f64 },
    Polygon(Vec<(f64, f64)>),
}

/// The full decomposition of a pch symbol at a given center point, split
/// by how each piece should be painted (mirroring the `gc->fill`/`gc->col`
/// juggling `GESymbol()` does for each case).
#[derive(Clone, Debug, Default)]
pub struct PchGeometry {
    /// Shapes stroked with `col`, not filled (R forces `fill =
    /// R_TRANWHITE` for the "open" pch 0-14 family).
    pub outline_only: Vec<Shape>,
    /// Shapes filled solidly with `col`, no border (pch 15-18, for which
    /// R sets `gc->col = R_TRANWHITE`).
    pub filled_with_col: Vec<Shape>,
    /// Shapes filled with `col` *and* stroked with `col` at `par(lwd)` (pch
    /// 19 and 20, which keep `gc->col`), so the stroke makes them `lwd / 2`
    /// larger in radius than pch 16.
    pub filled_and_stroked_with_col: Vec<Shape>,
    /// Shapes filled with the symbol's background color (`bg`/`points(...,
    /// bg=)`) and stroked with `col` (the pch 21-25 family).
    pub filled_with_bg: Vec<Shape>,
    /// Plain line segments, always stroked with `col` at `lty = "solid"`
    /// regardless of `par(lty)` (matches `gc.lty = LTY_SOLID;` forced by
    /// `GSymbol()`).
    pub lines: Vec<((f64, f64), (f64, f64))>,
}

/// `size_inches` for `GESymbol`, given the device's starting point size
/// (`cra`, not the current `par(ps)`) and `cex`. See module docs.
pub fn symbol_size_inches(pointsize: f64, cex: f64) -> f64 {
    pointsize * cex / 120.0
}

const RADIUS: f64 = 0.375;
const SMALL: f64 = 0.25;
const SQRC: f64 = 0.886_226_925_452_758; // sqrt(pi / 4)
const DMDC: f64 = 1.253_314_137_315_500; // sqrt(pi / 4) * sqrt(2)
const TRC0: f64 = 1.555_120_301_556_214; // sqrt(4 * pi/(3 * sqrt(3)))
const TRC1: f64 = 1.346_773_687_088_598; // TRC0 * sqrt(3) / 2
const TRC2: f64 = 0.777_560_150_778_107; // TRC0 / 2
const SQRT2: f64 = std::f64::consts::SQRT_2;

/// Build the shape decomposition for `pch` (`0..=25`) centered at `(x,
/// y)`, both in inches, with symbol size `size` in inches (see
/// [`symbol_size_inches`]). Returns `None` for pch values not handled
/// here (negative pch/UTF-8 code points and pch as an ASCII character,
/// including `pch = '.'`, are text-rendering special cases handled by the
/// caller instead, exactly as in `GESymbol()`).
pub fn pch_geometry(pch: i32, x: f64, y: f64, size: f64) -> Option<PchGeometry> {
    if !(0..=25).contains(&pch) {
        return None;
    }
    let mut g = PchGeometry::default();
    let gstr0 = size; // already in inches; GSTR_0 in R is size converted to inches (no-op for us)

    match pch {
        0 => {
            let (xc, yc) = (RADIUS * gstr0, RADIUS * gstr0);
            g.outline_only.push(Shape::Rect {
                x0: x - xc,
                y0: y - yc,
                x1: x + xc,
                y1: y + yc,
            });
        }
        1 => {
            g.outline_only.push(Shape::Circle {
                cx: x,
                cy: y,
                r: RADIUS * size,
            });
        }
        2 => {
            let xc0 = RADIUS * gstr0;
            let r = TRC0 * xc0;
            let yc = TRC2 * xc0;
            let xc = TRC1 * xc0;
            g.outline_only.push(Shape::Polygon(vec![
                (x, y + r),
                (x + xc, y - yc),
                (x - xc, y - yc),
            ]));
        }
        3 => {
            let xc = SQRT2 * RADIUS * gstr0;
            let yc = xc;
            g.lines.push(((x - xc, y), (x + xc, y)));
            g.lines.push(((x, y - yc), (x, y + yc)));
        }
        4 => {
            let (xc, yc) = (RADIUS * gstr0, RADIUS * gstr0);
            g.lines.push(((x - xc, y - yc), (x + xc, y + yc)));
            g.lines.push(((x - xc, y + yc), (x + xc, y - yc)));
        }
        5 => {
            let xc = SQRT2 * RADIUS * gstr0;
            let yc = xc;
            g.outline_only.push(Shape::Polygon(vec![
                (x - xc, y),
                (x, y + yc),
                (x + xc, y),
                (x, y - yc),
            ]));
        }
        6 => {
            let xc0 = RADIUS * gstr0;
            let r = TRC0 * xc0;
            let yc = TRC2 * xc0;
            let xc = TRC1 * xc0;
            g.outline_only.push(Shape::Polygon(vec![
                (x, y - r),
                (x + xc, y + yc),
                (x - xc, y + yc),
            ]));
        }
        7 => {
            let (xc, yc) = (RADIUS * gstr0, RADIUS * gstr0);
            g.outline_only.push(Shape::Rect {
                x0: x - xc,
                y0: y - yc,
                x1: x + xc,
                y1: y + yc,
            });
            g.lines.push(((x - xc, y - yc), (x + xc, y + yc)));
            g.lines.push(((x - xc, y + yc), (x + xc, y - yc)));
        }
        8 => {
            let (xc, yc) = (RADIUS * gstr0, RADIUS * gstr0);
            g.lines.push(((x - xc, y - yc), (x + xc, y + yc)));
            g.lines.push(((x - xc, y + yc), (x + xc, y - yc)));
            let xc2 = SQRT2 * RADIUS * gstr0;
            let yc2 = xc2;
            g.lines.push(((x - xc2, y), (x + xc2, y)));
            g.lines.push(((x, y - yc2), (x, y + yc2)));
        }
        9 => {
            let xc = SQRT2 * RADIUS * gstr0;
            let yc = xc;
            g.lines.push(((x - xc, y), (x + xc, y)));
            g.lines.push(((x, y - yc), (x, y + yc)));
            g.outline_only.push(Shape::Polygon(vec![
                (x - xc, y),
                (x, y + yc),
                (x + xc, y),
                (x, y - yc),
            ]));
        }
        10 => {
            let (xc, yc) = (RADIUS * gstr0, RADIUS * gstr0);
            g.outline_only.push(Shape::Circle {
                cx: x,
                cy: y,
                r: xc,
            });
            g.lines.push(((x - xc, y), (x + xc, y)));
            g.lines.push(((x, y - yc), (x, y + yc)));
        }
        11 => {
            let xc0 = RADIUS * gstr0;
            let r = TRC0 * xc0;
            let yc = 0.5 * (TRC2 * xc0 + r);
            let xc = TRC1 * xc0;
            g.outline_only.push(Shape::Polygon(vec![
                (x, y - r),
                (x + xc, y + yc),
                (x - xc, y + yc),
            ]));
            g.outline_only.push(Shape::Polygon(vec![
                (x, y + r),
                (x + xc, y - yc),
                (x - xc, y - yc),
            ]));
        }
        12 => {
            let (xc, yc) = (RADIUS * gstr0, RADIUS * gstr0);
            g.lines.push(((x - xc, y), (x + xc, y)));
            g.lines.push(((x, y - yc), (x, y + yc)));
            g.outline_only.push(Shape::Rect {
                x0: x - xc,
                y0: y - yc,
                x1: x + xc,
                y1: y + yc,
            });
        }
        13 => {
            g.outline_only.push(Shape::Circle {
                cx: x,
                cy: y,
                r: RADIUS * size,
            });
            let (xc, yc) = (RADIUS * gstr0, RADIUS * gstr0);
            g.lines.push(((x - xc, y - yc), (x + xc, y + yc)));
            g.lines.push(((x - xc, y + yc), (x + xc, y - yc)));
        }
        14 => {
            let (xc, yc) = (RADIUS * gstr0, RADIUS * gstr0);
            g.outline_only.push(Shape::Polygon(vec![
                (x, y + yc),
                (x + xc, y - yc),
                (x - xc, y - yc),
            ]));
            g.outline_only.push(Shape::Rect {
                x0: x - xc,
                y0: y - yc,
                x1: x + xc,
                y1: y + yc,
            });
        }
        15 => {
            let (xc, yc) = (RADIUS * gstr0, RADIUS * gstr0);
            g.filled_with_col.push(Shape::Polygon(vec![
                (x - xc, y - yc),
                (x + xc, y - yc),
                (x + xc, y + yc),
                (x - xc, y + yc),
            ]));
        }
        16 => {
            g.filled_with_col.push(Shape::Circle {
                cx: x,
                cy: y,
                r: RADIUS * size,
            });
        }
        17 => {
            let xc0 = RADIUS * gstr0;
            let r = TRC0 * xc0;
            let yc = TRC2 * xc0;
            let xc = TRC1 * xc0;
            g.filled_with_col.push(Shape::Polygon(vec![
                (x, y + r),
                (x + xc, y - yc),
                (x - xc, y - yc),
            ]));
        }
        18 => {
            let (xc, yc) = (RADIUS * gstr0, RADIUS * gstr0);
            g.filled_with_col.push(Shape::Polygon(vec![
                (x - xc, y),
                (x, y + yc),
                (x + xc, y),
                (x, y - yc),
            ]));
        }
        19 => {
            g.filled_and_stroked_with_col.push(Shape::Circle {
                cx: x,
                cy: y,
                r: RADIUS * size,
            });
        }
        20 => {
            g.filled_and_stroked_with_col.push(Shape::Circle {
                cx: x,
                cy: y,
                r: SMALL * size,
            });
        }
        21 => {
            g.filled_with_bg.push(Shape::Circle {
                cx: x,
                cy: y,
                r: RADIUS * size,
            });
        }
        22 => {
            let xc = RADIUS * SQRC * gstr0;
            let yc = xc;
            g.filled_with_bg.push(Shape::Rect {
                x0: x - xc,
                y0: y - yc,
                x1: x + xc,
                y1: y + yc,
            });
        }
        23 => {
            let xc = RADIUS * DMDC * gstr0;
            let yc = xc;
            g.filled_with_bg.push(Shape::Polygon(vec![
                (x, y - yc),
                (x + xc, y),
                (x, y + yc),
                (x - xc, y),
            ]));
        }
        24 => {
            let xc0 = RADIUS * gstr0;
            let r = TRC0 * xc0;
            let yc = TRC2 * xc0;
            let xc = TRC1 * xc0;
            g.filled_with_bg.push(Shape::Polygon(vec![
                (x, y + r),
                (x + xc, y - yc),
                (x - xc, y - yc),
            ]));
        }
        25 => {
            let xc0 = RADIUS * gstr0;
            let r = TRC0 * xc0;
            let yc = TRC2 * xc0;
            let xc = TRC1 * xc0;
            g.filled_with_bg.push(Shape::Polygon(vec![
                (x, y - r),
                (x + xc, y + yc),
                (x - xc, y + yc),
            ]));
        }
        _ => unreachable!(),
    }
    Some(g)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn circle_pch_is_outline_only() {
        let g = pch_geometry(1, 0.0, 0.0, symbol_size_inches(12.0, 1.0)).unwrap();
        assert_eq!(g.outline_only.len(), 1);
        assert!(g.filled_with_col.is_empty());
        assert!(g.filled_with_bg.is_empty());
    }

    #[test]
    fn solid_pch_16_has_no_border_but_19_and_20_do() {
        let size = symbol_size_inches(12.0, 1.0);
        let g = pch_geometry(16, 0.0, 0.0, size).unwrap();
        assert_eq!(g.filled_with_col.len(), 1);
        assert!(g.filled_and_stroked_with_col.is_empty());
        for pch in [19, 20] {
            let g = pch_geometry(pch, 0.0, 0.0, size).unwrap();
            assert!(g.filled_with_col.is_empty());
            assert_eq!(g.filled_and_stroked_with_col.len(), 1);
        }
    }

    #[test]
    fn open_circle_with_bg_pch21() {
        let g = pch_geometry(21, 0.0, 0.0, symbol_size_inches(12.0, 1.0)).unwrap();
        assert_eq!(g.filled_with_bg.len(), 1);
    }

    #[test]
    fn out_of_range_pch_is_none() {
        assert!(pch_geometry(26, 0.0, 0.0, 0.1).is_none());
        assert!(pch_geometry(-1, 0.0, 0.0, 0.1).is_none());
    }
}
