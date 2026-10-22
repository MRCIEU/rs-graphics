# rs-graphics

A Rust port of the core functionality of R's `graphics` package (see
[https://github.com/wch/r-source](https://github.com/wch/r-source) for the reference implementation this
crate is ported from), producing **SVG**, **PNG**, and **PDF** output.

Plots look the way they do in R: the same layout, ticks, margins,
symbols and line styles, so figures move between R and Rust without
surprises. The API follows R's own function and argument names, so if
you know R graphics, you already know how to use it.

It was written so that tools built in Rust can make publication-ready
plots themselves: fast, fully customisable, and with no R installation
needed. Everything, fonts included, compiles into a single static
binary, so a tool and its plots can be shipped in a small Docker image
or as one file, and drawing millions of points takes seconds.

For what is implemented and what isn't, the design notes, the example
gallery and the tests, see [DEVELOPER.md](DEVELOPER.md).

## Usage

```rust
use rs_graphics::prelude::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0];
    let y = [2.1, 3.9, 6.2, 7.8, 10.1];

    // R: pdf("scatter.pdf"); plot(x, y, ...); abline(h = 6); legend(...); dev.off()
    let mut dev = Device::pdf("scatter.pdf").open()?;
    dev.plot().x(&x).y(&y)
        .pch(19)
        .col("steelblue")
        .main(Label::math("y == beta[0] + beta[1] * x"))
        .xlab("x")
        .ylab("y")
        .draw();
    dev.abline().h(6.0).col("grey").lty("dashed").draw();
    // R: legend("topleft", legend = c("observed", "threshold"),
    //           col = c("steelblue", "grey"), pch = c(19, NA), lty = c(NA, 2))
    dev.legend().pos("topleft").labels(["observed", "threshold"])
        .col(["steelblue", "grey"])
        .pch([Some(19), None])
        .lty([None, Some(2)])
        .draw();
    dev.close()?;
    Ok(())
}
```

Each R function is a method with R's name. Chained setters, named as R's
arguments, take all of them, data included (`.x(&x)`), and `.draw()` runs
the call. A required argument you leave out panics at `.draw()` with R's
message. Arguments with defaults that you leave out get R's defaults (mostly the current `par()` values), and
calls without `.draw()` draw nothing (the compiler warns). The
argument setters are documented on the call (e.g. `PointsCall::x`), and
the rest on its options struct, e.g. `XyOptions` for `points()`. As in R, numbers may be integers or floats:
`dev.plot().x(1..=10).y(&y).cex(2).xlim(0, 12)` (`1..=10` is R's `1:10`), and
data vectors may hold any number type. Vectors can be passed by
reference or by value (`.x(&x)` or `.x(x)`, `.h([1, 2])`); large `f64`
data isn't copied either way. R's `NA` in a style vector is
`None`: R's `pch = c(19, NA)` is `.pch([Some(19), None])`.

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

`Device::pdf(file)` and `Device::svg(file)` work the same way. The
device's arguments (`width`, `height`, `pointsize`, `bg`, `res` for PNG,
`compress` for PDF) have R's defaults, so
`Device::pdf("plot.pdf").open()?` is R's `pdf("plot.pdf")`, 7 x 7 inches.
As in R, opening the device creates the file, so a bad path fails
before anything is drawn, and `dev.close()` (R's `dev.off()`) writes it.
Sizes are in inches for every format. The format is the function's, not
the file name's. `Device::format(format, file)` takes the format as a
value or a name (`"pdf"`, `"svg"`, `"png"`), for choosing it at run
time, and `dev.finish()` returns the file's bytes instead of writing
them.

### Several panels on a page

As in R, each high-level call (`plot`, `hist`, `barplot`, `boxplot`,
`pie`) starts the next panel (unless it's given `add`), and the calls
after it draw in that panel.
High-level calls return what R's functions return.

```rust
// R: pdf("panels.pdf", 8, 4); par(mfrow = c(1, 2), oma = c(0, 0, 2, 0))
let mut dev = Device::pdf("panels.pdf").width(8.0).height(4.0).open()?;
dev.par().mfrow(1, 2).oma([0.0, 0.0, 2.0, 0.0]).draw();

let x = [4.2, 5.1, 5.3, 5.9, 6.4, 7.0, 7.7];
let h = dev.hist().x(&x).col("lightblue").main("Histogram").draw(); // panel 1
dev.abline().v(h.mids[2]).lty("dashed").draw();

let centres = dev // panel 2: bar centres, for labelling the bars
    .barplot().height([3.0, 5.0, 2.0])
    .names(["A", "B", "C"])
    .ylim(0.0, 6.0)
    .draw();
dev.text().x(&centres).y(&[3.3, 5.3, 2.3]).labels(&["3", "5", "2"]).draw();

// R: mtext("Two panels", outer = TRUE, line = 0.5, font = 2)
dev.mtext().text("Two panels").side(3).outer(true).line(0.5).font(2).draw();
dev.close()?;
```

`par()` ends in `.draw()` like every other call, although it draws
nothing: `.draw()` is what runs a call. It is the one way to change
graphical parameters, including R's page layout ones (`mfrow`, `mfcol`,
`mfg`, `fig`, `fin`, `new`); `dev.current_par()` reads them.
`dev.layout(..)` gives uneven panels (R's `layout()`), and
`dev.par().mfg(row, col).draw()` picks the next one.

### Reusing arguments

Build an options struct once and start a call from it with `.with()`;
setters after it change just that call.

```rust
let grid = AblineOptions::default().col("grey").lty("dotted");
dev.abline().with(&grid).h([2.0, 4.0, 6.0, 8.0]).draw();
dev.abline().with(&grid).v([2.0, 4.0, 6.0, 8.0]).draw();
```

### Figures as values

A `Figure` records the same calls without a device, so you can build a
figure (a plot and its margins, one panel of a page) once and draw it
later, on any device and in any panel. The output is the same as making
the calls on the device. Page layout stays with the device: a figure is
one panel, so its `par()` has no `mfrow` or `mfcol`.

```rust
let dose = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
let response = [1.8, 3.1, 3.9, 5.2, 5.8, 7.1];
let fitted = [2.0, 3.0, 4.0, 5.0, 6.0, 7.0];

let mut fig = Figure::new();
fig.plot().x(&dose).y(&response)
    .xlab("Dose")
    .ylab("Response")
    .main(Label::math("y == alpha + beta * x"))
    .draw();
fig.lines().x(&dose).y(&fitted).col("red").draw();
fig.legend().pos("bottomright").labels(["observed", "fitted"])
    .col(["black", "red"])
    .pch([Some(1), None])
    .lty([None, Some(1)])
    .draw();

let mut dev = Device::pdf("fits.pdf").width(8.0).height(8.0).open()?;
dev.par().mfrow(2, 2).draw();
dev.add_figure_at(1, 1, &fig); // top left
dev.add_figure_at(2, 2, &fig); // bottom right
dev.close()?;
```

The legend is placed when the figure is drawn, so it goes in the bottom
right of whichever panel the figure lands in, sized for that page.
`add_figure` returns where each legend went (R's `legend()` return
value), since that depends on the device.

A figure has no page size or format of its own, so to save one, open a
device for the file and add the figure to it, as R replays a recorded
plot onto a device you've opened. The same figure can go to several
files:

```rust
for format in ["pdf", "png"] {
    let mut dev = Device::format(format, format!("fit.{format}"))
        .width(7.0)
        .height(5.0)
        .res(300.0) // PNG only
        .open()?;
    dev.add_figure(&fig);
    dev.close()?;
}
```

### Your own plots

Write them once against the `Graphics` trait, and they work on a device
and in a `Figure`, called like the built-in ones.

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

## License

GPL-3.0-or-later (see [LICENSE](LICENSE)). This crate ports R's own
algorithms closely enough that it's derived from `r-source`, which is
GPL-2.0-or-later (one translated file, `src/main/plot.c`,
GPL-3.0-or-later), so it is licensed under version 3 of the GPL, as R's
"or later" terms allow. Programs that use rs-graphics must therefore be
distributed under GPL-3.0 (or a later version).

The ported parts of R remain copyright The R Core Team, The R Foundation,
Robert Gentleman and Ross Ihaka, and Paul Murrell and Ross Ihaka.
[NOTICE](NOTICE) lists these notices, the R source files the code was
translated from, and when. Programs that use rs-graphics should carry the
same section in their notices.

The crate embeds Adobe's Helvetica and Symbol AFM metrics (`assets/afm/`, under
the notice in `assets/afm/MustRead.html`) and the Liberation Sans fonts
(`assets/fonts/`, SIL OFL 1.1). Their notices, and those of the
third-party crates compiled in, are in [NOTICE](NOTICE). Programs that
use rs-graphics embed these too, so their own notices should include them.
