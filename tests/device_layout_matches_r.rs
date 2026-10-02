//! Cross-checks `Device`'s plot-region size and `plot.window()` padding
//! against R's own `par("pin")`/`par("mai")`/`par("usr")` on a `pdf()`
//! device. Skipped if `Rscript` isn't available.

use std::process::Command;

use rs_graphics::backend::pdf::PdfBackend;
use rs_graphics::device::Device;
use rs_graphics::par::Par;

fn rscript_available() -> bool {
    Command::new("Rscript")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn r_par_values(width: f64, height: f64, ps: f64, xlim: (f64, f64), ylim: (f64, f64)) -> Vec<f64> {
    let script = format!(
        "pdf(file = tempfile(), width = {width}, height = {height}, pointsize = {ps}); \
         plot.new(); \
         plot.window(xlim = c({}, {}), ylim = c({}, {})); \
         cat(par('pin'), par('mai'), par('usr')); \
         invisible(dev.off())",
        xlim.0, xlim.1, ylim.0, ylim.1
    );
    let out = Command::new("Rscript")
        .arg("-e")
        .arg(&script)
        .output()
        .expect("run Rscript");
    assert!(
        out.status.success(),
        "Rscript failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout)
        .split_whitespace()
        .map(|s| s.parse().unwrap())
        .collect()
}

#[test]
fn plot_region_and_usr_match_r() {
    if !rscript_available() {
        eprintln!("Rscript not found on PATH; skipping R cross-check.");
        return;
    }

    let (width, height, ps) = (6.0, 4.0, 12.0);
    let xlim = (0.0, 10.0);
    let ylim = (0.0, 100.0);

    let r_values = r_par_values(width, height, ps, xlim, ylim);
    // par('pin') -> 2 values, par('mai') -> 4 values, par('usr') -> 4 values
    let (pin, rest) = r_values.split_at(2);
    let (mai, usr) = rest.split_at(4);

    let mut par = Par::default();
    par.ps = ps;
    let backend = PdfBackend::new(width, height);
    let mut dev = Device::new(backend, par);
    dev.plot_new();
    dev.plot_window(xlim, ylim, false, false);

    let plt = dev.plot_region();
    assert!(
        (plt.width() - pin[0]).abs() < 1e-6,
        "plot width mismatch: rust={} r={}",
        plt.width(),
        pin[0]
    );
    assert!(
        (plt.height() - pin[1]).abs() < 1e-6,
        "plot height mismatch: rust={} r={}",
        plt.height(),
        pin[1]
    );

    let computed_mai = dev.par.mai();
    let r_mai = [mai[0], mai[1], mai[2], mai[3]]; // bottom, left, top, right
    assert!((computed_mai.bottom - r_mai[0]).abs() < 1e-6);
    assert!((computed_mai.left - r_mai[1]).abs() < 1e-6);
    assert!((computed_mai.top - r_mai[2]).abs() < 1e-6);
    assert!((computed_mai.right - r_mai[3]).abs() < 1e-6);

    let (ux0, ux1, uy0, uy1) = dev.usr();
    // R's par('usr') order is (x0, x1, y0, y1).
    assert!(
        (ux0 - usr[0]).abs() < 1e-6,
        "usr x0: rust={ux0} r={}",
        usr[0]
    );
    assert!(
        (ux1 - usr[1]).abs() < 1e-6,
        "usr x1: rust={ux1} r={}",
        usr[1]
    );
    assert!(
        (uy0 - usr[2]).abs() < 1e-6,
        "usr y0: rust={uy0} r={}",
        usr[2]
    );
    assert!(
        (uy1 - usr[3]).abs() < 1e-6,
        "usr y1: rust={uy1} r={}",
        usr[3]
    );
}
