//! Cross-checks `rs_graphics::highlevel::hist`'s bin edges/counts against
//! R's own `hist(x, plot = FALSE)` for a handful of fixed datasets.
//!
//! Skipped (not failed) if `Rscript` isn't on `PATH`.

use std::process::Command;

use rs_graphics::backend::pdf::PdfBackend;
use rs_graphics::device::Device;
use rs_graphics::highlevel::{hist, HistOptions};
use rs_graphics::par::Par;

fn rscript_available() -> bool {
    Command::new("Rscript")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn r_hist(data: &[f64]) -> (Vec<f64>, Vec<f64>) {
    let values: Vec<String> = data.iter().map(|v| format!("{v:.17}")).collect();
    let script = format!(
        "x <- c({}); h <- hist(x, plot = FALSE); \
         cat(paste(sprintf('%.17g', h$breaks), collapse=','), '|', \
             paste(h$counts, collapse=','), '\\n')",
        values.join(",")
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
    let mut parts = text.trim().split('|');
    let breaks: Vec<f64> = parts
        .next()
        .unwrap()
        .split(',')
        .map(|s| s.trim().parse().unwrap())
        .collect();
    let counts: Vec<f64> = parts
        .next()
        .unwrap()
        .split(',')
        .map(|s| s.trim().parse().unwrap())
        .collect();
    (breaks, counts)
}

#[test]
fn breaks_and_counts_match_r_sturges() {
    if !rscript_available() {
        eprintln!("Rscript not found on PATH; skipping R cross-check.");
        return;
    }

    let datasets: &[&[f64]] = &[
        &[1.0, 2.0, 2.0, 3.0, 3.0, 3.0, 4.0, 4.0, 5.0],
        &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0],
        &[-5.0, -3.0, -1.0, 0.0, 1.0, 2.0, 4.0, 8.0, 10.0, 12.0, 15.0],
        &[
            0.1, 0.2, 0.15, 0.3, 0.25, 0.4, 0.35, 0.5, 0.45, 0.6, 0.55, 0.7, 0.65,
        ],
    ];

    for data in datasets {
        let (expected_breaks, expected_counts) = r_hist(data);

        let backend = PdfBackend::new(6.0, 4.0).with_compression(false);
        let mut dev = Device::new(backend, Par::default());
        let result = hist(&mut dev, data, &HistOptions::default());

        assert_eq!(
            result.breaks.len(),
            expected_breaks.len(),
            "break count mismatch for {data:?}: rust={:?} r={:?}",
            result.breaks,
            expected_breaks
        );
        for (a, b) in result.breaks.iter().zip(&expected_breaks) {
            assert!(
                (a - b).abs() < 1e-6,
                "break mismatch for {data:?}: rust={:?} r={:?}",
                result.breaks,
                expected_breaks
            );
        }
        assert_eq!(
            result.counts, expected_counts,
            "count mismatch for {data:?}: rust={:?} r={:?}",
            result.counts, expected_counts
        );
    }
}
