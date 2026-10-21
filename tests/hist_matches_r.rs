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
use rs_graphics::highlevel::HistBreaks;
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
        let result = dev.hist().x(data).draw();

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
        let result = dev.hist().x(&data).breaks(breaks).draw();
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
    dev.hist().x(&[1.0, 2.0, 3.0]).breaks(0).draw();
}

#[test]
#[should_panic(expected = "invalid number of 'breaks'")]
fn nan_breaks_panics_like_r() {
    let mut dev = Device::new(PdfBackend::new(6.0, 4.0), Par::default());
    dev.hist().x(&[1.0, 2.0, 3.0]).breaks(f64::NAN).draw();
}

/// R's output of `expr` (an R expression evaluated with `x` set to `data`),
/// printed with 17 significant digits, as numbers.
fn r_values(data: &[f64], expr: &str) -> Vec<f64> {
    let values: Vec<String> = data.iter().map(|v| format!("{v:.17}")).collect();
    let script = format!(
        "x <- c({}); v <- {expr}; cat(sprintf('%.17g', as.numeric(v)), '\\n')",
        values.join(",")
    );
    let out = Command::new("Rscript")
        .args(["-e", &script])
        .output()
        .expect("failed to run Rscript");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout)
        .split_whitespace()
        .map(|v| v.parse().unwrap())
        .collect()
}

/// The `"Scott"`/`"FD"` rules (also named in other cases or by prefix,
/// with FD's fallbacks for ties), functions returning edges or a count,
/// `equidist`, and `xlim`/`ylim`.
#[test]
fn breaks_rules_and_functions_match_r() {
    if !common::r_available() {
        return;
    }
    let spread: Vec<f64> = (0..30)
        .map(|i| (i as f64 * 0.7).sin() * 4.0 + i as f64 * 0.3)
        .collect();
    let mut ties = vec![5.0; 20];
    ties.extend([1.0, 9.0, 10.0]);
    let skewed = [-5.0, -3.0, -1.0, 0.0, 1.0, 2.0, 4.0, 8.0, 10.0, 12.0, 15.0];
    let rules: [(&str, HistBreaks); 4] = [
        ("'Scott'", "Scott".into()),
        ("'FD'", "FD".into()),
        ("'fd'", "fd".into()),
        ("'freedman'", "freedman".into()),
    ];
    for data in [&spread[..], &ties[..], &skewed[..]] {
        for (r_breaks, breaks) in &rules {
            let expected = r_values(
                data,
                &format!("hist(x, breaks = {r_breaks}, plot = FALSE)$breaks"),
            );
            let mut dev = Device::new(PdfBackend::new(6.0, 4.0), Par::default());
            let result = dev.hist().x(data).breaks(breaks.clone()).draw();
            assert_eq!(
                result.breaks, expected,
                "breaks = {r_breaks}, data {data:?}"
            );
        }
    }
    // Functions: edges, or one value (a number of classes).
    let edges = r_values(
        &skewed,
        "hist(x, breaks = function(x) c(max(x), -10, 0, 20), plot = FALSE)$breaks",
    );
    let mut dev = Device::new(PdfBackend::new(6.0, 4.0), Par::default());
    let result = dev
        .hist()
        .x(&skewed)
        .breaks(HistBreaks::function(|x| {
            let hi = x.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            vec![hi, -10.0, 0.0, 20.0]
        }))
        .draw();
    assert_eq!(result.breaks, edges);
    let r_equidist = r_values(
        &skewed,
        "hist(x, breaks = function(x) c(max(x), -10, 0, 20), plot = FALSE)$equidist",
    );
    assert_eq!(result.equidist, r_equidist == [1.0]);
    let count = r_values(
        &skewed,
        "hist(x, breaks = function(x) 4, plot = FALSE)$breaks",
    );
    let mut dev = Device::new(PdfBackend::new(6.0, 4.0), Par::default());
    let result = dev
        .hist()
        .x(&skewed)
        .breaks(HistBreaks::function(|_| vec![4.0]))
        .draw();
    assert_eq!(result.breaks, count);
    assert!(result.equidist);
    // xlim/ylim set the window.
    let usr = r_values(
        &skewed,
        "{ pdf(NULL); hist(x, xlim = c(-20, 30), ylim = c(0, 10)); par('usr') }",
    );
    let mut dev = Device::new(PdfBackend::new(7.0, 7.0), Par::default());
    dev.hist().x(&skewed).xlim(-20, 30).ylim(0, 10).draw();
    let (a, b, c, d) = dev.usr();
    for (ours, r) in [a, b, c, d].iter().zip(&usr) {
        assert!((ours - r).abs() < 1e-9, "usr {:?} vs {usr:?}", (a, b, c, d));
    }
}

#[test]
#[should_panic(expected = "is not a valid histogram breaks rule")]
fn unknown_breaks_rule_panics() {
    let _ = HistBreaks::from("f");
}

/// `right`, `include.lowest` and `fuzz` with integer data on the edges.
#[test]
fn right_include_lowest_and_fuzz_match_r() {
    if !common::r_available() {
        return;
    }
    // 1.2 is within `fuzz = 0.3` of the edge at 1.
    let data = [0.0, 1.0, 1.2, 2.0, 2.0, 2.0, 3.0, 4.0, 4.0, 4.5];
    let edges = [0.0, 1.0, 2.0, 3.0, 5.0];
    let cases: [(&str, Option<bool>, Option<bool>, Option<f64>); 5] = [
        ("right = FALSE", Some(false), None, None),
        (
            "right = FALSE, include.lowest = FALSE",
            Some(false),
            Some(false),
            None,
        ),
        ("right = TRUE, fuzz = 0", None, None, Some(0.0)),
        ("right = FALSE, fuzz = 0.3", Some(false), None, Some(0.3)),
        ("fuzz = 0.3", None, None, Some(0.3)),
    ];
    for (r_args, right, lowest, fuzz) in cases {
        let expected = r_values(
            &data,
            &format!("hist(x, breaks = c(0, 1, 2, 3, 5), {r_args}, plot = FALSE)$counts"),
        );
        let mut dev = Device::new(PdfBackend::new(6.0, 4.0), Par::default());
        let mut call = dev.hist().x(&data).breaks(edges);
        if let Some(r) = right {
            call = call.right(r);
        }
        if let Some(l) = lowest {
            call = call.include_lowest(l);
        }
        if let Some(f) = fuzz {
            call = call.fuzz(f);
        }
        assert_eq!(call.draw().counts, expected, "{r_args}");
    }
}

#[test]
#[should_panic(expected = "some 'x' not counted")]
fn breaks_not_covering_the_data_panic_like_r() {
    let mut dev = Device::new(PdfBackend::new(6.0, 4.0), Par::default());
    dev.hist().x(&[1.0, 2.0, 5.0]).breaks([1, 2, 3]).draw();
}

/// With `include.lowest = FALSE` a value on the first edge isn't counted,
/// which R reports as an error.
#[test]
#[should_panic(expected = "some 'x' not counted")]
fn include_lowest_false_leaves_the_first_edge_out_like_r() {
    let mut dev = Device::new(PdfBackend::new(6.0, 4.0), Par::default());
    dev.hist()
        .x(&[0.0, 1.0, 2.0])
        .breaks([0, 1, 2])
        .include_lowest(false)
        .draw();
}
