//! Cross-checks `arrows()` against a real R `pdf()` device's uncompressed
//! output: every vertex of every shaft and head, for heads at the start,
//! end and both ends, `angle = 90` error-bar caps, steep/diagonal shafts,
//! a skipped `NA` segment and a zero-length arrow (drawn without heads).
//! Skipped if `Rscript` isn't available.

use std::process::Command;

use rs_graphics::backend::pdf::PdfBackend;
use rs_graphics::color::{Color, ColorSpec};
use rs_graphics::device::{Device, LineStyle};
use rs_graphics::lty::LineType;
use rs_graphics::par::Par;

fn rscript_available() -> bool {
    Command::new("Rscript")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Every stroked/filled `m`/`l` path's vertices, in drawing order (clips
/// and rectangles are skipped).
fn paths(pdf: &str) -> Vec<Vec<(f64, f64)>> {
    let mut out = Vec::new();
    let mut cur: Vec<(f64, f64)> = Vec::new();
    for tok_line in pdf.lines() {
        let toks: Vec<&str> = tok_line.split_whitespace().collect();
        for (i, t) in toks.iter().enumerate() {
            let num = |k: usize| toks[i - k].parse::<f64>().unwrap();
            match *t {
                "m" | "l" => cur.push((num(2), num(1))),
                "re" | "n" => cur.clear(),
                "S" | "s" | "f" | "B" | "b" | "f*" | "B*" | "b*" => {
                    if !cur.is_empty() {
                        out.push(std::mem::take(&mut cur));
                    }
                }
                _ => {}
            }
        }
    }
    out
}

/// `(x0, y0, x1, y1, length, angle, code)` per `arrows()` call.
type Call = (&'static [f64], &'static [f64], &'static [f64], &'static [f64], f64, f64, u8);

const CALLS: &[Call] = &[
    // R's defaults.
    (&[1.0], &[1.0], &[4.0], &[2.0], 0.25, 30.0, 2),
    (&[2.0, 6.0], &[8.0, 9.0], &[2.0, 9.0], &[5.0, 9.0], 0.1, 20.0, 1),
    // Horizontal and vertical error bars, with an NA in the middle.
    (&[5.0, 6.0, 7.0], &[3.0, f64::NAN, 4.0], &[8.0, 6.5, 9.5], &[3.0, 6.0, 4.0], 0.05, 90.0, 3),
    (&[3.0], &[1.0], &[3.0], &[6.0], 0.05, 90.0, 3),
    // Steep diagonal, both ends, wide angle.
    (&[9.0], &[1.0], &[8.0], &[7.0], 0.2, 60.0, 3),
    // Zero length: shaft only.
    (&[5.0], &[5.0], &[5.0], &[5.0], 0.25, 30.0, 2),
];

fn r_vec(v: &[f64]) -> String {
    let items: Vec<String> = v
        .iter()
        .map(|x| if x.is_nan() { "NA".to_string() } else { x.to_string() })
        .collect();
    format!("c({})", items.join(", "))
}

#[test]
fn arrows_match_r() {
    if !rscript_available() {
        eprintln!("Rscript not found on PATH; skipping R cross-check.");
        return;
    }
    let dir = std::env::temp_dir();
    let pdf_path = dir.join("rs_graphics_arrows.pdf");
    let mut script = format!(
        "pdf(file = '{}', width = 7, height = 5, compress = FALSE)\n\
         plot.new(); plot.window(c(0, 10), c(0, 10))\n",
        pdf_path.display()
    );
    for (x0, y0, x1, y1, length, angle, code) in CALLS {
        script.push_str(&format!(
            "suppressWarnings(arrows({}, {}, {}, {}, length = {length}, angle = {angle}, code = {code}))\n",
            r_vec(x0),
            r_vec(y0),
            r_vec(x1),
            r_vec(y1)
        ));
    }
    script.push_str("invisible(dev.off())\n");
    let script_path = dir.join("rs_graphics_arrows.R");
    std::fs::write(&script_path, script).unwrap();
    let out = Command::new("Rscript")
        .arg(&script_path)
        .output()
        .expect("run Rscript");
    assert!(
        out.status.success(),
        "Rscript failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let r_pdf = String::from_utf8_lossy(&std::fs::read(&pdf_path).unwrap()).into_owned();

    let mut dev = Device::new(PdfBackend::new(7.0, 5.0).with_compression(false), Par::default());
    dev.plot_new();
    dev.plot_window((0.0, 10.0), (0.0, 10.0), false, false);
    let style = LineStyle::new(ColorSpec::Color(Color::BLACK), 1.0, LineType::Pattern(&[]));
    for &(x0, y0, x1, y1, length, angle, code) in CALLS {
        dev.arrows(x0, y0, x1, y1, length, angle, code, style);
    }
    let ours = String::from_utf8_lossy(&dev.into_backend().finish()).into_owned();

    let (r, o) = (paths(&r_pdf), paths(&ours));
    assert_eq!(o.len(), r.len(), "path count\nours={o:?}\nr={r:?}");
    for (i, (a, b)) in o.iter().zip(&r).enumerate() {
        assert_eq!(a.len(), b.len(), "path {i}: ours={a:?} r={b:?}");
        for (p, q) in a.iter().zip(b) {
            assert!(
                (p.0 - q.0).abs() < 0.05 && (p.1 - q.1).abs() < 0.05,
                "path {i}: ours={a:?} r={b:?}"
            );
        }
    }
}
