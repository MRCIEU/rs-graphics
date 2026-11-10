# rs-graphics

A Rust port of the core of R's `graphics` package ([r-source](https://github.com/wch/r-source)), writing **PDF**, **SVG** and **PNG**.

Plots look as they do in R (layout, ticks, margins, symbols, line styles), and the API uses R's function and argument names, so what you know of R graphics carries over. Everything, fonts included, compiles into one static binary, and drawing millions of points takes seconds.

rs-graphics is a work in progress: the API is still changing between releases (see [CHANGELOG.md](CHANGELOG.md) for migration notes).

## Usage

```rust
use rs_graphics::prelude::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0];
    let y = [2.1, 3.9, 6.2, 7.8, 10.1];

    // R: pdf("scatter.pdf"); plot(x, y, ...); abline(h = 6); legend(...); dev.off()
    let mut dev = Device::pdf("scatter.pdf").open()?;
    dev.plot()
        .x(&x)
        .y(&y)
        .pch(19)
        .col("steelblue")
        .main(Label::math("y == beta[0] + beta[1] * x"))
        .xlab("x")
        .ylab("y")
        .try_draw()?;
    dev.abline().h(6.0).col("grey").lty("dashed").try_draw()?;
    // R: legend("topleft", legend = c("observed", "threshold"),
    //           col = c("steelblue", "grey"), pch = c(19, NA), lty = c(NA, 2))
    dev.legend()
        .pos("topleft")
        .labels(["observed", "threshold"])
        .col(["steelblue", "grey"])
        .pch([Some(19), None])
        .lty([None, Some(2)])
        .try_draw()?;
    dev.close()?;
    Ok(())
}
```

- A `Device` is R's open `pdf()`/`png()`/`svg()` device: it holds the page, the `par()` settings and the file, which `close()` writes.
- Every R function is a method and every argument a setter with R's name. The call runs at `.try_draw()`, which returns R's error, or `.draw()`, which panics with it, as R stops. An argument left out has R's default, often the current `par()` value.
- Numbers may be integers or floats, and vectors go by reference or by value: `.x(1..=10).cex(2).xlim([0, 12])`.
- `None` is R's `NA` in a setting (`.pch([Some(19), None])`). In data and coordinates a missing value is `f64::NAN`, as in an `f64` column, and is skipped.

### Errors

`rs_graphics::Error` is R's error, with R's message and the call's name:

- a required argument missing: `plot(): argument "x" is missing, with no default`;
- a bad value: `.lty("wavy")` (a setter keeps the error for the call to return), `plot(): 'x' and 'y' lengths differ`;
- a plot that doesn't fit: `plot.new(): figure margins too large`;
- a file that can't be written (`open()`, `close()`): `ErrorKind::Io`.

It works with `?` (also in functions returning `std::io::Result`), and `kind()`, `call()` and `message()` can be matched. The one call that panics on a bad value is `Label::math("...")`; use `Expression::parse("...")` for text from users.

### R side by side

```rust
use rs_graphics::prelude::*;

let x = [1.0, 2.0, 3.0, 4.0, 5.0];
let y = [2.1, 3.9, 6.2, 7.8, 10.1];
let groups = [1, 1, 2, 2, 2];

// R: png("points.png", 6, 4, units = "in", res = 300)
let mut dev = Device::png("points.png").width(6.0).height(4.0).res(300.0).open()?;
// R: par(las = 1, mar = c(4, 4, 2, 1))
dev.par().las(1).mar([4.0, 4.0, 2.0, 1.0]).draw();
// R: plot(x, y, pch = 19, col = groups, xlab = "Dose", ylab = "Response")
dev.plot().x(&x).y(&y).pch(19).col(groups).xlab("Dose").ylab("Response").draw();
// R: text(x, y, labels = c("a", "b", "c", "d", "e"), adj = -0.5)
dev.text().x(&x).y(&y).labels(&["a", "b", "c", "d", "e"]).adj(-0.5).draw();
// R: legend("topleft", legend = c("control", "treated"), pch = 19, col = 1:2)
dev.legend().pos("topleft").labels(["control", "treated"]).pch(19).col([1, 2]).draw();
// R: dev.off()
dev.close()?;
```

Devices have R's defaults: `Device::pdf(file)` is 7 x 7 inches; `Device::png(file)` is R's 480 x 480 pixels (6.67 inches at 72 ppi, and a higher `res` adds pixels, not inches). `units` can be `"cm"`, `"mm"` or, for PNG, `"px"`:

`Device::png(file).width(1200).height(900).units("px").res(150)`

Opening creates the file, so a bad path fails before anything is drawn. `Device::null()` is `pdf(NULL)`: it draws nothing but measures as a PDF device does.

### Several panels

Each high-level call (`plot`, `hist`, `barplot`, `boxplot`, `pie`) starts the next panel (`hist`, `barplot` and `boxplot` draw on the current one with `.add(true)`), and returns what R's function returns.

```rust
// R: pdf("panels.pdf", 8, 4); par(mfrow = c(1, 2), oma = c(0, 0, 2, 0))
let mut dev = Device::pdf("panels.pdf").width(8.0).height(4.0).open()?;
dev.par().mfrow([1, 2]).oma([0.0, 0.0, 2.0, 0.0]).draw();

let x = [4.2, 5.1, 5.3, 5.9, 6.4, 7.0, 7.7];
let h = dev.hist().x(&x).col("lightblue").main("Histogram").draw(); // panel 1
dev.abline().v(h.mids[2]).lty("dashed").draw();

let centres = dev // panel 2: bar centres, for labelling the bars
    .barplot()
    .height([3.0, 5.0, 2.0])
    .names_arg(["A", "B", "C"])
    .ylim([0.0, 6.0])
    .draw();
dev.text().x(&centres).y(&[3.3, 5.3, 2.3]).labels(&["3", "5", "2"]).draw();

// R: mtext("Two panels", outer = TRUE, line = 0.5, font = 2)
dev.mtext().text("Two panels").side(3).outer(true).line(0.5).font(2).draw();
dev.close()?;
```

`par()` is the one way to change graphical parameters, page layout included (`mfrow`, `mfcol`, `mfg`, `fig`, `fin`, `new`); it ends in `.draw()` or `.try_draw()` like every call (an invalid value is R's error), and `dev.current_par()` reads them. `dev.layout().mat(..).draw()` is R's `layout()`.

### Reusing arguments

```rust
let grid = AblineOptions::default().col("grey").lty("dotted");
dev.abline().with(&grid).h([2.0, 4.0, 6.0, 8.0]).draw();
dev.abline().with(&grid).v([2.0, 4.0, 6.0, 8.0]).draw();
```

Every call has an options struct with the same setters (`PlotOptions`, `LegendOptions`, `XyOptions` for `points` and `lines`, ...); setters after `.with()` override it for that call.

### Figures as values

A `Figure` records the same calls without a device, to draw later on any device, in any panel, or as a whole plot with its own page layout (R's `recordPlot()`/`replayPlot()`).

```rust
let dose = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
let response = [1.8, 3.1, 3.9, 5.2, 5.8, 7.1];
let fitted = [2.0, 3.0, 4.0, 5.0, 6.0, 7.0];

let mut fig = Figure::new();
fig.plot()
    .x(&dose)
    .y(&response)
    .xlab("Dose")
    .ylab("Response")
    .main(Label::math("y == alpha + beta * x"))
    .draw();
fig.lines().x(&dose).y(&fitted).col("red").draw();
fig.legend()
    .pos("bottomright")
    .labels(["observed", "fitted"])
    .col(["black", "red"])
    .pch([Some(1), None])
    .lty([None, Some(1)])
    .draw();

let mut dev = Device::pdf("fits.pdf").width(8.0).height(8.0).open()?;
dev.par().mfrow([2, 2]).draw();
dev.add_figure_at(1, 1, &fig); // top left
dev.add_figure_at(2, 2, &fig); // bottom right
dev.close()?;
```

A whole plot goes in with `add_figure`. Mistakes in the arguments are found as the calls are recorded, and a null device checks the page before any file is written:

```rust
let mut summary = Figure::new();
summary.par().mfrow([1, 2]).oma([0, 0, 2, 0]).try_draw()?;
summary.hist().x(&response).main("Histogram").try_draw()?;
summary.boxplot().x(&response).main("Box plot").try_draw()?;
summary.mtext().text("Response").outer(true).line(0.5).font(2).try_draw()?;

// Does it fit a 6 x 3 inch page? (On a 2 x 2 inch page this returns
// "plot.new(): figure margins too large".)
let mut check = Device::null().width(6).height(3).open()?;
check.try_add_figure(&summary)?;

for format in ["pdf", "svg", "png"] {
    let mut dev = Device::format(format, format!("summary.{format}"))
        .width(6)
        .height(3)
        .open()?;
    dev.add_figure(&summary);
    dev.close()?;
}
```

A figure with a page layout leaves the device as it left it, as `replayPlot()` does; one without leaves `par()` alone.

### Your own plots

Write them against the `Graphics` trait and they work on a `Device` and in a `Figure`:

```rust
trait Diagnostics {
    /// Observed against expected values, with the line y = x.
    fn obs_vs_exp(&mut self, exp: &[f64], obs: &[f64]);
}

impl<G: Graphics> Diagnostics for G {
    fn obs_vs_exp(&mut self, exp: &[f64], obs: &[f64]) {
        self.plot().x(exp).y(obs).pch(20).xlab("Expected").ylab("Observed").draw();
        self.abline().a(0).b(1).col("red").draw();
    }
}

dev.obs_vs_exp(&[1.0, 2.0, 3.0], &[1.2, 1.9, 3.4]);
```

### Guides

In `cargo doc --open`, `rs_graphics::guide` has `concepts` (R's plotting vocabulary), `coming_from_r` (translating R code) and `r_reference` (every R function and argument with its Rust form), and `rs_graphics::charts` draws the symbols, line types, colours and fonts.

[llms.txt](llms.txt) has the same in one file, for tools that write code. [DEVELOPER.md](DEVELOPER.md) covers what is and isn't implemented, the design, the gallery and the tests.

## Licence

GPL-3.0-or-later (see [LICENSE](LICENSE)). This crate ports R's own
algorithms closely enough that it's derived from `r-source`, which is
GPL-2.0-or-later (one translated file, `src/main/plot.c`,
GPL-3.0-or-later), so it is licensed under version 3 of the GPL, as R's
"or later" terms allow. Programs that use rs-graphics must therefore be
distributed under GPL-3.0 (or a later version).

The ported parts of R remain copyright The R Core Team, The R Foundation,
Robert Gentleman and Ross Ihaka, Paul Murrell and Ross Ihaka, Ross Ihaka
(`signif()`), and Kevin Buhr (polygon hatching).
[NOTICE](NOTICE) lists these notices, the R source files the code was
translated from, and when. Programs that use rs-graphics should carry the
same section in their notices.

The crate embeds Adobe's Helvetica and Symbol AFM metrics (`assets/afm/`, under
the notice in `assets/afm/MustRead.html`), the Liberation Sans fonts
(`assets/fonts/`, SIL OFL 1.1) and, among R's named palettes, ColorBrewer's
colour schemes (Apache 2.0). Their notices, and those of the
third-party crates compiled in, are in [NOTICE](NOTICE). Programs that
use rs-graphics embed these too, so their own notices should include them.
