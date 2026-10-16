//! Fast number and colour formatting for the vector backends, which write
//! millions of coordinates for big scatter plots. Each function appends to a
//! `String` directly, with output identical to the `format!` it replaces.

use std::fmt::Write as _;

use crate::color::Color;

/// Append `v` with `prec` decimals (at most 6) to `out`, exactly as
/// `write!(out, "{v:.prec$}")` would, but several times faster: the
/// coordinates of big scatter plots are most of a PDF's or SVG's content.
///
/// Rust's formatting rounds `v`'s exact binary value to `prec` decimals
/// (ties to even). Here `|v| * 10^prec` is rounded to an integer instead,
/// which agrees unless that product is within its own rounding error of a
/// half; such values, and ones too large to scale accurately, NaN and
/// infinities, are left to `write!`.
pub(crate) fn push_fixed(out: &mut String, v: f64, prec: usize) {
    const SCALE: [f64; 7] = [1.0, 1e1, 1e2, 1e3, 1e4, 1e5, 1e6];
    // Below 2^31 the product's rounding error is at most 2^-22, well inside
    // the margin around a half.
    const LIMIT: f64 = 2_147_483_648.0;
    const MARGIN: f64 = 1e-6;
    let scaled = v.abs() * SCALE[prec];
    if scaled.is_nan() || scaled >= LIMIT || (scaled - scaled.floor() - 0.5).abs() < MARGIN {
        let _ = write!(out, "{v:.prec$}");
        return;
    }
    let n = scaled.round() as u64;
    // Like Rust's formatting, the sign is kept for -0.0 and for negative
    // values that round to zero.
    if v.is_sign_negative() {
        out.push('-');
    }
    // Digits from the right: `prec` decimals, the point, then the whole part.
    let mut digits = [0u8; 24];
    let mut at = digits.len();
    let mut rest = n;
    for i in 0.. {
        if i == prec && prec > 0 {
            at -= 1;
            digits[at] = b'.';
        }
        at -= 1;
        digits[at] = b'0' + (rest % 10) as u8;
        rest /= 10;
        if rest == 0 && i >= prec {
            break;
        }
    }
    out.push_str(std::str::from_utf8(&digits[at..]).expect("ASCII digits"));
}

/// Append `v` to `out` as the SVG backend writes numbers: a whole number
/// below 1e15 as an integer (`-0.0` as `0`), anything else with 4 decimals
/// and the trailing zeros, then a trailing point, removed (`1.5`, `-0`).
pub(crate) fn push_trimmed(out: &mut String, v: f64) {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        if v == 0.0 {
            out.push('0');
        } else {
            push_fixed(out, v, 0);
        }
        return;
    }
    let start = out.len();
    push_fixed(out, v, 4);
    // Only trim a fractional part (`NaN` and `inf` have none).
    if out[start..].contains('.') {
        let trimmed = out.trim_end_matches('0').trim_end_matches('.').len();
        out.truncate(trimmed);
    }
}

/// Append `c` as `#rrggbb`, or `#rrggbbaa` when it isn't opaque.
pub(crate) fn push_hex(out: &mut String, c: Color) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    out.push('#');
    let bytes = [c.r, c.g, c.b, c.a];
    let n = if c.a == 255 { 3 } else { 4 };
    for &b in &bytes[..n] {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 15) as usize] as char);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Values for the equivalence tests: random values at several scales,
    /// exact binary ties, the doubles either side of decimal halves, zeros,
    /// small negatives, NaN, infinities and huge values.
    fn test_values() -> Vec<f64> {
        let mut state = 0x2545_F491_4F6C_DD1Du64;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        let mut values = vec![
            0.0,
            -0.0,
            -0.0001,
            -0.00001,
            -0.0004999,
            0.0005,
            0.125,
            0.375,
            -0.125,
            2.5,
            -3.0,
            1e15,
            -1e15,
            1e20,
            -1e20,
            2_147_483_647.5,
            f64::MIN_POSITIVE,
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
        ];
        for _ in 0..200_000 {
            let r = next();
            let unit = (r >> 11) as f64 / (1u64 << 53) as f64; // [0, 1)
            let sign = if r & 1 == 0 { 1.0 } else { -1.0 };
            let scale = [1e-4, 1.0, 72.0, 1e3, 1e5, 1e9][(r >> 1) as usize % 6];
            values.push(sign * unit * scale);
            // Whole numbers, and exact binary ties at various positions.
            let k = (r >> 20) % 100_000;
            values.push(sign * k as f64);
            values.push(sign * (k as f64 + 0.5) / [1.0, 2.0, 4.0, 8.0][(r >> 3) as usize % 4]);
            for prec in [2, 3, 4, 6] {
                let half = (k as f64 + 0.5) / 10f64.powi(prec);
                // The neighbouring doubles (`half` is positive).
                let up = f64::from_bits(half.to_bits() + 1);
                let down = f64::from_bits(half.to_bits() - 1);
                values.extend([half, up, down, -half]);
            }
        }
        values
    }

    #[test]
    fn push_fixed_matches_rust_formatting() {
        for &v in &test_values() {
            for prec in [0, 2, 3, 4, 6] {
                let mut ours = String::new();
                push_fixed(&mut ours, v, prec);
                assert_eq!(ours, format!("{v:.prec$}"), "v = {v:?}, prec = {prec}");
            }
        }
    }

    /// The SVG backend's previous number formatting.
    fn fmt_num(v: f64) -> String {
        if v.fract() == 0.0 && v.abs() < 1e15 {
            format!("{}", v as i64)
        } else {
            let s = format!("{v:.4}");
            s.trim_end_matches('0').trim_end_matches('.').to_string()
        }
    }

    #[test]
    fn push_trimmed_matches_previous_svg_formatting() {
        for &v in &test_values() {
            let mut ours = String::from("x=");
            push_trimmed(&mut ours, v);
            assert_eq!(ours, format!("x={}", fmt_num(v)), "v = {v:?}");
        }
    }

    #[test]
    fn push_hex_matches_to_hex() {
        for c in [
            Color::rgb(0, 0, 0),
            Color::rgb(255, 255, 255),
            Color::rgb(0x12, 0xab, 0xf0),
            Color::rgba(1, 2, 3, 4),
            Color::rgba(200, 30, 30, 128),
        ] {
            let mut ours = String::new();
            push_hex(&mut ours, c);
            assert_eq!(ours, c.to_hex());
        }
    }
}
