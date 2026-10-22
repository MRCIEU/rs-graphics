// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks `rs_graphics::highlevel::boxplot::boxplot_stats` against
//! R's own `boxplot.stats()` for a handful of fixed datasets (including
//! ones with outliers, to exercise the whisker-shrinking logic).
//!
//! Skipped (not failed) if `Rscript` isn't on `PATH`.

use crate::common;

use std::process::Command;

use rs_graphics::highlevel::boxplot::boxplot_stats;

fn r_boxplot_stats(data: &[f64]) -> (Vec<f64>, Vec<f64>) {
    let values: Vec<String> = data.iter().map(|v| format!("{v:.17}")).collect();
    let script = format!(
        "x <- c({}); s <- boxplot.stats(x); \
         cat(paste(sprintf('%.17g', s$stats), collapse=','), '|', \
             paste(sprintf('%.17g', s$out), collapse=','), '\\n')",
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
    let stats: Vec<f64> = parts
        .next()
        .unwrap()
        .split(',')
        .map(|s| s.trim().parse().unwrap())
        .collect();
    let out_str = parts.next().unwrap().trim();
    let outliers: Vec<f64> = if out_str.is_empty() {
        Vec::new()
    } else {
        out_str
            .split(',')
            .map(|s| s.trim().parse().unwrap())
            .collect()
    };
    (stats, outliers)
}

#[test]
fn stats_match_r_boxplot_stats() {
    if !common::r_available() {
        return;
    }

    let datasets: &[&[f64]] = &[
        &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0],
        &[
            1.0, 2.0, 2.0, 3.0, 3.0, 3.0, 4.0, 4.0, 5.0, 100.0, // outlier
        ],
        &[5.0, 7.0, 7.0, 8.0, 9.0, 9.0, 10.0, 12.0, 13.0, 14.0, -50.0], // low outlier
        &[2.0, 4.0, 6.0, 8.0, 10.0, 12.0, 14.0],
        &[1.0],
        &[1.0, 1.0, 1.0, 1.0],
    ];

    for data in datasets {
        let (expected_stats, expected_out) = r_boxplot_stats(data);
        let stats = boxplot_stats(data, 1.5);
        let actual = [
            stats.lower_whisker,
            stats.q1,
            stats.median,
            stats.q3,
            stats.upper_whisker,
        ];
        for (a, b) in actual.iter().zip(&expected_stats) {
            assert!(
                (a - b).abs() < 1e-6,
                "stats mismatch for {data:?}: rust={actual:?} r={expected_stats:?}"
            );
        }
        assert_eq!(
            stats.outliers.len(),
            expected_out.len(),
            "outlier count mismatch for {data:?}: rust={:?} r={:?}",
            stats.outliers,
            expected_out
        );
        for (a, b) in stats.outliers.iter().zip(&expected_out) {
            assert!(
                (a - b).abs() < 1e-6,
                "outlier mismatch for {data:?}: rust={:?} r={:?}",
                stats.outliers,
                expected_out
            );
        }
    }
}
