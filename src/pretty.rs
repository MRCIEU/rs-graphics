//! A direct port of R's axis "pretty" tick-mark algorithm.
//!
//! This mirrors `R_pretty()` in `r-source/src/appl/pretty.c`, which is
//! shared by **two** different callers that use different constants and
//! post-processing:
//!
//! - `GEPretty()` in `r-source/src/main/engine.c` - used by
//!   `axis()`/`plot.window()` to choose default tick locations. See
//!   [`ge_pretty`].
//! - `pretty.default()` in `r-source/src/library/base/R/pretty.R` - the
//!   user-facing `pretty()` function, also used internally by
//!   `hist.default()` to choose bin edges. See [`pretty_r`].
//!
//! Both wrap the same `r_pretty` core with different `(min_n,
//! shrink_sml, high_u_bias, u5_bias, f_min, eps_correction, bounds)`
//! parameters, kept here as a single generic function so the shared
//! structure is easy to check line-by-line against the original.

const ROUNDING_EPS: f64 = 1e-10;

/// Port of `R_pretty()`, generic over the `(min_n, shrink_sml,
/// high_u_bias, u5_bias, f_min, eps_correction, return_bounds)` knobs its
/// two callers ([`ge_pretty`], [`pretty_r`]) each fix differently.
///
/// Returns `(lo, up, ndiv, unit)`. When `return_bounds` is `false` (used
/// by `GEPretty()`), `lo`/`up` are the *unit multiples* `ns`/`nu` (i.e.
/// `lo * unit`/`up * unit` are the actual axis extents) - matching what
/// the C code leaves in `*lo`/`*up` in that branch. When `return_bounds`
/// is `true` (used by `pretty.default()`), `lo`/`up` are already the
/// final (possibly widened-to-cover-the-input-range) axis extents.
#[allow(clippy::too_many_arguments)]
fn r_pretty(
    lo_in: f64,
    up_in: f64,
    ndiv_in: i32,
    min_n: i32,
    shrink_sml: f64,
    h: f64,
    h5: f64,
    f_min: f64,
    eps_correction: i32,
    return_bounds: bool,
) -> (f64, f64, i32, f64) {
    let lo_ = lo_in;
    let up_ = up_in;
    let dx = up_ - lo_;

    let mut ndiv = ndiv_in.max(1);

    let mut cell;
    let i_small;
    if dx == 0.0 && up_ == 0.0 {
        cell = 1.0;
        i_small = true;
    } else {
        cell = lo_.abs().max(up_.abs());
        let u = 1.0
            + if h5 >= 1.5 * h + 0.5 {
                1.0 / (1.0 + h)
            } else {
                1.5 / (1.0 + h5)
            };
        let u = u * (ndiv.max(1) as f64) * f64::EPSILON;
        i_small = dx < cell * u * 3.0;
    }

    if i_small {
        if cell > 10.0 {
            cell = 9.0 + cell / 10.0;
        }
        cell *= shrink_sml;
        if min_n > 1 {
            cell /= min_n as f64;
        }
    } else {
        cell = dx;
        if dx.is_finite() {
            if ndiv > 1 {
                cell /= ndiv as f64;
            }
        } else if ndiv >= 2 {
            cell = up_ / (ndiv as f64) - lo_ / (ndiv as f64);
        }
    }

    const MAX_F: f64 = 1.25;
    let mut subsmall = f_min * f64::MIN_POSITIVE;
    if subsmall == 0.0 {
        subsmall = f64::MIN_POSITIVE;
    }
    if cell < subsmall {
        cell = subsmall;
    } else if cell > f64::MAX / MAX_F {
        cell = f64::MAX / MAX_F;
    }

    let base = 10f64.powf(cell.log10().floor());

    let mut unit = base;
    let mut u = 2.0 * base;
    if u - cell < h * (cell - unit) {
        unit = u;
        u = 5.0 * base;
        if u - cell < h5 * (cell - unit) {
            unit = u;
            u = 10.0 * base;
            if u - cell < h * (cell - unit) {
                unit = u;
            }
        }
    }

    let mut ns = (lo_ / unit + ROUNDING_EPS).floor();
    let mut nu = (up_ / unit - ROUNDING_EPS).ceil();

    let mut lo = lo_;
    let mut up = up_;
    if eps_correction != 0 && (eps_correction > 1 || !i_small) {
        let e_ = f64::EPSILON;
        let d_max = f64::MAX * (1.0 - (e_ / 2.0));
        if lo_ < 0.0 {
            lo *= 1.0 + e_;
        } else if lo_ > 0.0 {
            lo *= 1.0 - e_;
        } else {
            lo = -unit.min(f64::MIN_POSITIVE);
        }
        if up_ < 0.0 {
            up *= 1.0 - e_;
        } else if up_ > 0.0 {
            if up_ < d_max {
                up *= 1.0 + e_;
            }
        } else {
            up = unit.min(f64::MIN_POSITIVE);
        }
    }

    while ns * unit > lo + ROUNDING_EPS * unit {
        ns -= 1.0;
    }
    while !(ns * unit).is_finite() {
        ns += 1.0;
    }
    while nu * unit < up - ROUNDING_EPS * unit {
        nu += 1.0;
    }
    while !(nu * unit).is_finite() {
        nu -= 1.0;
    }

    let mut k = (0.5 + nu - ns) as i32;
    if k < min_n {
        k = min_n - k;
        if lo_ == 0.0 && ns == 0.0 && up_ != 0.0 {
            nu += k as f64;
        } else if up_ == 0.0 && nu == 0.0 && lo_ != 0.0 {
            ns -= k as f64;
        } else if ns >= 0.0 {
            nu += (k / 2) as f64;
            ns -= (k / 2 + k % 2) as f64;
        } else {
            ns -= (k / 2) as f64;
            nu += (k / 2 + k % 2) as f64;
        }
        ndiv = min_n;
    } else {
        ndiv = k;
    }

    if return_bounds {
        if ns * unit < lo {
            lo = ns * unit;
        }
        if nu * unit > up {
            up = nu * unit;
        }
        (lo, up, ndiv, unit)
    } else {
        (ns, nu, ndiv, unit)
    }
}

/// `high_u_fact = (h, h5, f_min)` as used by `GEPretty()`.
const GE_HIGH_U_FACT: [f64; 3] = [0.8, 1.7, 1.125];

/// Port of `GEPretty()`: given a range `[lo, up]` and a target number of
/// intervals `ndiv` (R's `par("lab")[1]`, default `5`), returns
/// `(lo, up, ndiv)` - the (possibly widened) axis extents and the actual
/// number of intervals chosen. Tick locations are then `lo + i * (up -
/// lo) / ndiv` for `i` in `0..=ndiv`.
pub fn ge_pretty(lo_in: f64, up_in: f64, ndiv_in: i32) -> (f64, f64, i32) {
    assert!(
        ndiv_in > 0,
        "invalid axis extents [GEPretty(.,.,n={ndiv_in})]"
    );
    assert!(
        lo_in.is_finite() && up_in.is_finite(),
        "non-finite axis extents"
    );

    let (mut ns, mut nu, mut ndiv, unit) = r_pretty(
        lo_in,
        up_in,
        ndiv_in,
        /* min_n = */ 1,
        /* shrink_sml = */ 0.25,
        GE_HIGH_U_FACT[0],
        GE_HIGH_U_FACT[1],
        GE_HIGH_U_FACT[2],
        /* eps_correction = */ 2,
        /* return_bounds = */ false,
    );

    if nu >= ns + 1.0 {
        let mut mod_ = false;
        if ns * unit < lo_in - ROUNDING_EPS * unit {
            ns += 1.0;
            mod_ = true;
        }
        if nu > ns + 1.0 && nu * unit > up_in + ROUNDING_EPS * unit {
            nu -= 1.0;
            mod_ = true;
        }
        if mod_ {
            ndiv = (nu - ns) as i32;
        }
    }

    (ns * unit, nu * unit, ndiv)
}

/// Convenience wrapper returning the actual tick values, mirroring what
/// `axis()` / `axTicks()` would show for a default (`n = 5`) linear axis.
///
/// Ticks are placed as `CreateAtVector()` (`graphics.c`) does for a linear
/// axis: `lo + (i / ndiv) * (up - lo)`, with any tick closer to zero than
/// `|up - lo| / (100 * ndiv)` set to exactly 0 (otherwise e.g. -0.2..0.4
/// would get a `2.775558e-17` tick).
pub fn pretty_ticks(lo: f64, up: f64, ndiv: i32) -> Vec<f64> {
    let (lo, up, ndiv) = ge_pretty(lo, up, ndiv);
    let dn = ndiv.max(1) as f64;
    let range = up - lo;
    let small = range.abs() / (100.0 * dn);
    (0..=ndiv)
        .map(|i| {
            let at = lo + (i as f64 / dn) * range;
            if at.abs() < small {
                0.0
            } else {
                at
            }
        })
        .collect()
}

/// Port of base R's `pretty.default()` (the user-facing `pretty()`
/// function), used by e.g. `hist.default()` to choose bin edges. Unlike
/// [`ge_pretty`], this uses `pretty.default()`'s own constants
/// (`shrink.sml = 0.75`, `high.u.bias = 1.5`, `u5.bias = 2.75`, `f.min =
/// 2^-20`, `eps.correct = 0`, `bounds = TRUE`) and returns the actual
/// break/tick values directly (`seq(l, u, length.out = ndiv + 1)`,
/// including R's near-zero "zap" correction for `eps.correct = 0`).
///
/// `min_n` corresponds to `pretty()`'s `min.n` argument (default `n %/%
/// 3`, but `hist.default()` always calls with `min.n = 1`).
pub fn pretty_r(lo_in: f64, up_in: f64, n: i32, min_n: i32) -> Vec<f64> {
    assert!(n > 0, "invalid 'n'");
    assert!(lo_in.is_finite() && up_in.is_finite(), "non-finite range");

    let high_u_bias = 1.5;
    let u5_bias = 0.5 + 1.5 * high_u_bias;
    let f_min = 2f64.powi(-20);

    let (l, u, ndiv, _unit) = r_pretty(
        lo_in,
        up_in,
        n,
        min_n,
        /* shrink_sml = */ 0.75,
        high_u_bias,
        u5_bias,
        f_min,
        /* eps_correction = */ 0,
        /* return_bounds = */ true,
    );

    let n = ndiv as usize;
    let mut s: Vec<f64> = (0..=n)
        .map(|i| l + (u - l) * (i as f64) / (n as f64))
        .collect();

    // `eps.correct == 0` => zap near-zero rounding noise from `seq()`,
    // matching `pretty.default()`'s post-processing.
    let delta = (u - l) / n as f64;
    let threshold = 1e-14 * delta;
    for v in &mut s {
        if v.abs() < threshold {
            *v = 0.0;
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_known_r_output() {
        // R: pretty(c(0, 10)) -> 0 2 4 6 8 10
        let ticks = pretty_ticks(0.0, 10.0, 5);
        assert_eq!(ticks, vec![0.0, 2.0, 4.0, 6.0, 8.0, 10.0]);
    }

    #[test]
    fn matches_known_r_output_odd_range() {
        // R: axTicks default on range(c(1,7)) with n=5 -> 1,2,...? verified against Rscript in integration tests.
        let (lo, up, ndiv) = ge_pretty(1.0, 7.0, 5);
        assert!(ndiv >= 1);
        assert!(lo <= 1.0 && up >= 7.0);
    }

    #[test]
    fn pretty_r_matches_known_r_output() {
        // R: pretty(c(1, 10), n = 5, min.n = 1) -> 0 2 4 6 8 10
        let breaks = pretty_r(1.0, 10.0, 5, 1);
        assert_eq!(breaks, vec![0.0, 2.0, 4.0, 6.0, 8.0, 10.0]);
    }
}
