//! Cross-checks `rust_plots::pretty::ge_pretty` against R itself.
//!
//! `par("xaxp")` after `plot.window(xlim = c(lo, up), xaxs = "i")` reports
//! exactly the `(lo, up, ndiv)` triple that R's C-level `GEPretty()`
//! computed for that range - the same routine this crate's `pretty`
//! module ports. `xaxs = "i"` disables the usual 4% padding so the user
//! range fed to `GEPretty()` is exactly `(lo, up)`.
//!
//! This test is skipped (not failed) if `Rscript` isn't on `PATH`, so it
//! stays portable for contributors without R installed.

use std::process::Command;

use rs_graphics::pretty::ge_pretty;

fn rscript_available() -> bool {
    Command::new("Rscript")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn r_xaxp(lo: f64, up: f64, ndiv: i32) -> (f64, f64, i32) {
    let script = format!(
        "pdf(file = tempfile()); par(lab = c({ndiv}, {ndiv}, 7)); \
         plot.window(xlim = c({lo}, {up}), ylim = c(0, 1), xaxs = 'i'); \
         xaxp <- par('xaxp'); \
         cat(sprintf('%.17g %.17g %d', xaxp[1], xaxp[2], as.integer(xaxp[3]))); \
         invisible(dev.off())"
    );
    let out = Command::new("Rscript")
        .arg("-e")
        .arg(&script)
        .output()
        .expect("failed to run Rscript");
    assert!(
        out.status.success(),
        "Rscript failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8_lossy(&out.stdout);
    let mut parts = text.split_whitespace();
    let lo: f64 = parts.next().unwrap().parse().unwrap();
    let up: f64 = parts.next().unwrap().parse().unwrap();
    let ndiv: i32 = parts.next().unwrap().parse().unwrap();
    (lo, up, ndiv)
}

#[test]
fn ge_pretty_matches_r_xaxp() {
    if !rscript_available() {
        eprintln!("Rscript not found on PATH; skipping R cross-check.");
        return;
    }

    let cases: &[(f64, f64, i32)] = &[
        (0.0, 10.0, 5),
        (1.0, 7.0, 5),
        (0.0, 1.0, 5),
        (-5.0, 5.0, 5),
        (0.0, 100.0, 5),
        (2.0, 3.0, 5),
        (0.0, 0.001, 5),
        (1.0, 7.0, 3),
        (0.0, 10.0, 10),
        (-3.7, 8.2, 5),
        (1000.0, 1007.0, 5),
    ];

    for &(lo, up, ndiv) in cases {
        let expected = r_xaxp(lo, up, ndiv);
        let actual = ge_pretty(lo, up, ndiv);
        assert!(
            (actual.0 - expected.0).abs() < 1e-9
                && (actual.1 - expected.1).abs() < 1e-9
                && actual.2 == expected.2,
            "mismatch for lo={lo}, up={up}, ndiv={ndiv}: rust={actual:?} r={expected:?}"
        );
    }
}
