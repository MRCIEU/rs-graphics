// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks the bin edges and counts `hist()` returns against R's own
//! `hist(x, plot = FALSE)` for a handful of fixed datasets, with the
//! default Sturges breaks, a number of classes (`breaks = 20`) and unsorted
//! edges.
//!
//! Skipped (not failed) if `Rscript` isn't on `PATH`.

mod common;

use std::process::Command;

use rs_graphics::backend::pdf::PdfBackend;
use rs_graphics::device::Device;
use rs_graphics::graphics::Graphics;
use rs_graphics::par::Par;

/// R's `hist(x, breaks = <breaks>, plot = FALSE)` (`breaks` an R
/// expression, or "" for the default): breaks and counts.
fn r_hist(data: &[f64], breaks: &str) -> (Vec<f64>, Vec<f64>) {
    let values: Vec<String> = data.iter().map(|v| format!("{v:.17}")).collect();
    let breaks = if breaks.is_empty() {
        String::new()
    } else {
        format!(", breaks = {breaks}")
    };
    let script = format!(
        "x <- c({}); h <- hist(x{breaks}, plot = FALSE); \
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
    if !common::r_available() {
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
        let (expected_breaks, expected_counts) = r_hist(data, "");

        let backend = PdfBackend::new(6.0, 4.0).with_compression(false);
        let mut dev = Device::new(backend, Par::default());
        let result = dev.hist(data).draw();

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

/// `breaks` as one number is a number of classes (edges from `pretty()`,
/// the number truncated); two or more are edges, sorted.
#[test]
fn breaks_as_a_count_or_unsorted_edges_match_r() {
    if !common::r_available() {
        return;
    }
    let data = [-5.0, -3.0, -1.0, 0.0, 1.0, 2.0, 4.0, 8.0, 10.0, 12.0, 15.0];
    let cases: [(&str, Vec<f64>); 4] = [
        ("20", vec![20.0]),
        ("3", vec![3.0]),
        ("7.9", vec![7.9]),
        ("c(20, -10, 0, 5)", vec![20.0, -10.0, 0.0, 5.0]),
    ];
    for (r_breaks, breaks) in cases {
        let (expected_breaks, expected_counts) = r_hist(&data, r_breaks);
        let mut dev = Device::new(PdfBackend::new(6.0, 4.0), Par::default());
        let result = dev.hist(&data).breaks(breaks).draw();
        assert_eq!(
            result.breaks.len(),
            expected_breaks.len(),
            "breaks = {r_breaks}"
        );
        for (a, b) in result.breaks.iter().zip(&expected_breaks) {
            assert!(
                (a - b).abs() < 1e-9,
                "breaks = {r_breaks}: {:?} vs {expected_breaks:?}",
                result.breaks
            );
        }
        assert_eq!(result.counts, expected_counts, "breaks = {r_breaks}");
    }
}

#[test]
#[should_panic(expected = "invalid number of 'breaks'")]
fn zero_breaks_panics_like_r() {
    let mut dev = Device::new(PdfBackend::new(6.0, 4.0), Par::default());
    dev.hist(&[1.0, 2.0, 3.0]).breaks(0).draw();
}

#[test]
#[should_panic(expected = "invalid number of 'breaks'")]
fn nan_breaks_panics_like_r() {
    let mut dev = Device::new(PdfBackend::new(6.0, 4.0), Par::default());
    dev.hist(&[1.0, 2.0, 3.0]).breaks(f64::NAN).draw();
}
