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

    // R: pdf("scatter.pdf", 7, 7); plot(x, y, ...); abline(h = 6); legend(...); dev.off()
    let mut dev = Device::for_path("scatter.pdf", 7.0, 7.0, 300.0)?;
    dev.plot(&x, &y)
        .pch(19)
        .col("steelblue")
        .main(Label::math("y == beta[0] + beta[1] * x"))
        .xlab("x")
        .ylab("y")
        .draw();
    dev.abline().h(6.0).col("grey").lty(LineType::DASHED).draw();
    let keys = [
        LegendEntry::point("observed", "steelblue", 19),
        LegendEntry::line("threshold", "grey", 1.0, LineType::DASHED),
    ];
    dev.legend("topleft", &keys).draw();
    dev.save("scatter.pdf")?;
    Ok(())
}
```

Each R function is a method with R's name. It takes R's data arguments;
chained setters take the others, and `.draw()` runs the call. Arguments
you leave out get R's defaults (mostly the current `par()` values), and
calls without `.draw()` draw nothing (the compiler warns). Every
setter is documented on its options struct, e.g. `XyOptions` for
`points()`.

### R side by side

```rust
use rs_graphics::prelude::*;

let x = [1.0, 2.0, 3.0, 4.0, 5.0];
let y = [2.1, 3.9, 6.2, 7.8, 10.1];
let groups = [1, 1, 2, 2, 2];

// R: png("points.png", 6, 4, units = "in", res = 300)
let mut dev = Device::png(6.0, 4.0, 300.0);
// R: par(las = 1, mar = c(4, 4, 2, 1))
dev.par().las(1).mar([4.0, 4.0, 2.0, 1.0]).draw();
// R: plot(x, y, pch = 19, col = groups, xlab = "Dose", ylab = "Response")
dev.plot(&x, &y).pch(19).col(groups).xlab("Dose").ylab("Response").draw();
// R: text(x, y, labels = c("a", "b", "c", "d", "e"), adj = -0.5)
dev.text(&x, &y, &["a", "b", "c", "d", "e"]).adj(-0.5).draw();
// R: legend("topleft", legend = c("control", "treated"), pch = 19, col = 1:2)
let keys = [
    LegendEntry::point("control", 1, 19),
    LegendEntry::point("treated", 2, 19),
];
dev.legend("topleft", &keys).draw();
// R: dev.off()
dev.save("points.png")?;
```

`Device::pdf(w, h)` and `Device::svg(w, h)` work the same way, and
`Device::for_path(path, w, h, dpi)` picks the format from the file
extension. `dev.finish()` returns the file's bytes instead of saving.

### Several panels on a page

As in R, each high-level call (`plot`, `hist`, `barplot`, `boxplot`,
`pie`) starts the next panel (unless it's given `add`), and the calls
after it draw in that panel.
High-level calls return what R's functions return.

```rust
// R: pdf("panels.pdf", 8, 4); par(mfrow = c(1, 2), oma = c(0, 0, 2, 0))
let mut dev = Device::pdf(8.0, 4.0);
dev.par().mfrow(1, 2).oma([0.0, 0.0, 2.0, 0.0]).draw();

let x = [4.2, 5.1, 5.3, 5.9, 6.4, 7.0, 7.7];
let h = dev.hist(&x).col("lightblue").main("Histogram").draw(); // panel 1
dev.abline().v(h.mids[2]).lty(LineType::DASHED).draw();

let centres = dev // panel 2: bar centres, for labelling the bars
    .barplot([3.0, 5.0, 2.0])
    .names(["A", "B", "C"])
    .ylim(0.0, 6.0)
    .draw();
dev.text(&centres, &[3.3, 5.3, 2.3], &["3", "5", "2"]).draw();

// R: mtext("Two panels", outer = TRUE, line = 0.5, font = 2)
dev.mtext("Two panels", 3).outer(true).line(0.5).font(2).draw();
dev.save("panels.pdf")?;
```

`par()` ends in `.draw()` like every other call, although it draws
nothing: `.draw()` is what runs a call. `dev.mfrow(1, 2)` is a shortcut
for `dev.par().mfrow(1, 2).draw()`. `dev.layout(..)` gives uneven panels
(R's `layout()`), and `dev.mfg(row, col)` picks the next one.

### Reusing arguments

Build an options struct once and start a call from it with `.with()`;
setters after it change just that call.

```rust
let grid = AblineOptions::default().col("grey").lty(LineType::DOTTED);
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
fig.plot(&dose, &response)
    .xlab("Dose")
    .ylab("Response")
    .main(Label::math("y == alpha + beta * x"))
    .draw();
fig.lines(&dose, &fitted).col("red").draw();
let keys = [
    LegendEntry::point("observed", 1, 1),
    LegendEntry::line("fitted", "red", 1.0, LineType::SOLID),
];
fig.legend("bottomright", &keys).draw();

let mut dev = Device::pdf(8.0, 8.0);
dev.par().mfrow(2, 2).draw();
dev.add_figure_at(1, 1, &fig); // top left
dev.add_figure_at(2, 2, &fig); // bottom right
```

The legend is placed when the figure is drawn, so it goes in the bottom
right of whichever panel the figure lands in, sized for that page.
`add_figure` returns where each legend went (R's `legend()` return
value), since that depends on the device.

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
        self.plot(exp, obs).pch(20).xlab("Expected").ylab("Observed").draw();
        self.abline().coef(0.0, 1.0).col("red").draw();
    }
}

dev.obs_vs_exp(&[1.0, 2.0, 3.0], &[1.2, 1.9, 3.4]);
```

## License

GPL-3.0-or-later (see [LICENSE](LICENSE)). This crate ports R's own
algorithms closely enough that it's derived from `r-source`, which is
GPL-2.0-or-later, so it is licensed under version 3 of the GPL, as R's
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
