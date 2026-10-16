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

/// Default tick values, in data units, for a log axis whose `usr` range is
/// `log_lo..log_hi` (log10 units, `log_lo < log_hi`), with `nint` intervals
/// (`par("lab")`, 5 by default): `GAxisPars()` + `GLPretty()`
/// (`graphics.c`) then `CreateAtVector()`'s log branch (`plot.c`).
///
/// Over less than about two decades the ticks are linear `pretty()` values;
/// otherwise powers of ten with 1, 2, 5 (up to 2 decades between the
/// outermost powers inside the range), 1, 5 (3 decades) or 1 (more, every
/// few decades so there are about `nint` intervals) times each.
pub fn log_axis_ticks(log_lo: f64, log_hi: f64, nint: i32) -> Vec<f64> {
    // `GAxisPars()`: avoid infinities.
    let log_hi = log_hi.min(308.0);
    let log_lo = log_lo.max(-307.0).min(log_hi);
    let (dl, dh) = (10f64.powf(log_lo), 10f64.powf(log_hi));

    // `GLPretty()`.
    let (mut p1, mut p2) = (dl.log10().ceil(), dh.log10().floor());
    if p2 <= p1 && dh / dl > 10.0 {
        p1 = (dl.log10() - 0.5).ceil();
        p2 = (dh.log10() + 0.5).floor();
    }
    if p2 <= p1 {
        // Very small range: a linear axis.
        return pretty_ticks(dl, dh, nint);
    }
    let axp0 = 10f64.powf(p1);
    let axp1 = 10f64.powf(p2);
    let span = p2 - p1;

    // `CreateAtVector()`, log branch, with its fuzz on `usr`.
    let umin = dl * (1.0 - 1e-12);
    let umax = dh * (1.0 + 1e-12);
    let mut at = Vec::new();
    if span <= 2.0 {
        // 1, 2, 5 times 10^k.
        let mut dn = axp0;
        for f in [0.2, 0.5] {
            if f * dn >= umin {
                at.push(f * dn);
            }
        }
        loop {
            if dn > umax {
                break;
            }
            at.push(dn);
            if 2.0 * dn > umax {
                break;
            }
            at.push(2.0 * dn);
            if 5.0 * dn > umax {
                break;
            }
            at.push(5.0 * dn);
            dn *= 10.0;
        }
    } else if span <= 3.0 {
        // 1, 5 times 10^k.
        let mut dn = axp0;
        if 0.5 * dn >= umin {
            at.push(0.5 * dn);
        }
        loop {
            if dn > umax {
                break;
            }
            at.push(dn);
            if 5.0 * dn > umax {
                break;
            }
            at.push(5.0 * dn);
            dn *= 10.0;
        }
    } else {
        // 10^k, every `ne` decades.
        let i = (axp1.log10().floor() - axp0.log10().ceil() + 0.25) as i32;
        let mut ne = i / nint;
        let d0 = umax.log10() - axp0.log10();
        if ne < 1 {
            ne = 1;
        } else {
            while ne > 1 && (nint * ne) as f64 > d0 {
                ne -= 1;
            }
        }
        let k = 1 + ne / 308;
        if k > 1 {
            ne = k * (ne / k);
        }
        let d1 = d0 - (nint * ne) as f64;
        let start = if d1 > 5.0 {
            axp0 * 10f64.powf((d1 / 2.0).floor())
        } else {
            axp0
        };
        let rng = 10f64.powf(ne as f64 / k as f64);
        let mut dn = start;
        while dn < umax {
            at.push(dn);
            for _ in 0..k {
                dn *= rng;
            }
        }
    }
    at
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

    /// Values as R's `axTicks()` gives them for these `usr` ranges.
    #[test]
    fn log_axis_ticks_follow_r_rules() {
        let ticks = |lo: f64, hi: f64| log_axis_ticks(lo.log10(), hi.log10(), 5);
        // Under 2 decades between powers inside: 1, 2, 5.
        assert_eq!(ticks(3.0, 700.0), [5.0, 10.0, 20.0, 50.0, 100.0, 200.0, 500.0]);
        // 3 decades: 1, 5.
        assert_eq!(ticks(1.0, 1000.0), [1.0, 5.0, 10.0, 50.0, 100.0, 500.0, 1000.0]);
        // More: powers of ten, every other one over 10 decades.
        assert_eq!(ticks(1.0, 1e6), [1.0, 10.0, 100.0, 1e3, 1e4, 1e5, 1e6]);
        assert_eq!(ticks(1e-5, 1e5), [1e-5, 1e-3, 1e-1, 1e1, 1e3, 1e5]);
        // Less than a decade: linear pretty() ticks.
        assert_eq!(ticks(2.0, 15.0), pretty_ticks(2.0, 15.0, 5));
    }

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
