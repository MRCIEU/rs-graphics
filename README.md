# rs-graphics

A Rust port of the core functionality of R's `graphics` package (see [https://github.com/wch/r-source](https://github.com/wch/r-source) for the reference implementation this crate is ported from), producing **SVG**, **PNG**, and **PDF** output.

It was written so that you can deploy tools capable of making publication-ready plots while incurring minimal costs. Everything, fonts included, compiles into a single static binary, so a tool and its plots can be shipped in a small Docker image or as one file. Drawing millions of points takes seconds.

Plots look the way they do in R: the same layout, ticks, margins, symbols and line styles, so figures move between R and Rust without surprises. The API follows R's own function and argument names, so if you know R graphics, you already know how to use it.

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
        .draw();
    dev.abline().h(6.0).col("grey").lty("dashed").draw();
    // R: legend("topleft", legend = c("observed", "threshold"),
    //           col = c("steelblue", "grey"), pch = c(19, NA), lty = c(NA, 2))
    dev.legend()
        .pos("topleft")
        .labels(["observed", "threshold"])
        .col(["steelblue", "grey"])
        .pch([Some(19), None])
        .lty([None, Some(2)])
        .draw();
    dev.close()?;
    Ok(())
}
```

A `Device` is an open graphics device, like the one R's `pdf()`, `png()` or `svg()` opens. It holds the page being drawn, the `par()` settings, and the file it writes when you call `close()`.

Other R functions and arguments are represented by methods, which can be chained, and must be finished with `.draw()`. Calls without `.draw()` draw nothing (which the compiler warns you about).

If you leave out an argument R requires, `.draw()` panics with R's message (`dev.plot().draw()` gives `plot(): argument "x" is missing, with no default`), as it does for R's other errors; `.try_draw()` returns the error instead, so you can handle it. If you leave out one that has a default, you get R's default, which is often the current `par()` value (no `.col()` means `par("col")`).

As in R, numbers may be integers or floats (a vector's elements must all be the same type):

`dev.plot().x(1..=10).y(&y).cex(2).xlim(0, 12)`, 

Vectors can be passed by reference or by value (`.x(&x)` or `.x(x)`, `.h([1, 2])`).

Use `None` where R uses `NA`: 

`pch = c(19, NA)` is `.pch([Some(19), None])` 

(but `.pch([19, 20])` is fine when you intend no missing values).

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

A device's arguments mostly have R's defaults, so `Device::pdf("plot.pdf").open()?` is as R's `pdf("plot.pdf")` (height = 7 inches, width = 7 inches). `Device::png(file)` defaults to 6.67 x 6.67 inches at 72 dpi, R's 480 x 480 pixels. With a higher `res`, the page stays 6.67 inches and gets more pixels, where R keeps 480 x 480 pixels. Sizes are in inches unless `units` is set: you can also use `"cm"` and `"mm"` for every format, as well as `"px"` for png, which sizes it in pixels as R does:

`Device::png(file).width(1200).height(900).units("px").res(150)` is R's `png(width = 1200, height = 900, res = 150)`.

As in R, opening the device creates the file, so a bad path fails
before anything is drawn, and `dev.close()` (R's `dev.off()`) writes it.

`Device::null()` is R's `pdf(NULL)`: a device that draws nothing and writes no file, but measures as a PDF device does. Use it to check that a figure fits a page before opening a file, or to measure text and legends.


### Several panels on a page

Each high-level call (`plot`, `hist`, `barplot`, `boxplot`, `pie`) starts the next panel in a multi-panel plot (unless `hist`, `barplot` or `boxplot` is given `.add(true)`), and the calls after it draw in that panel. High-level calls return what R's functions return.

```rust
// R: pdf("panels.pdf", 8, 4); par(mfrow = c(1, 2), oma = c(0, 0, 2, 0))
let mut dev = Device::pdf("panels.pdf").width(8.0).height(4.0).open()?;
dev.par().mfrow(1, 2).oma([0.0, 0.0, 2.0, 0.0]).draw();

let x = [4.2, 5.1, 5.3, 5.9, 6.4, 7.0, 7.7];
let h = dev.hist().x(&x).col("lightblue").main("Histogram").draw(); // panel 1
dev.abline().v(h.mids[2]).lty("dashed").draw();

let centres = dev // panel 2: bar centres, for labelling the bars
    .barplot()
    .height([3.0, 5.0, 2.0])
    .names(["A", "B", "C"])
    .ylim(0.0, 6.0)
    .draw();
dev.text().x(&centres).y(&[3.3, 5.3, 2.3]).labels(&["3", "5", "2"]).draw();

// R: mtext("Two panels", outer = TRUE, line = 0.5, font = 2)
dev.mtext().text("Two panels").side(3).outer(true).line(0.5).font(2).draw();
dev.close()?;
```

`par()` needs to end in `.draw()` like every other call, although it draws nothing. It is the only way to change graphical parameters, including R's page layout ones (`mfrow`, `mfcol`, `mfg`, `fig`, `fin`, `new`). `dev.current_par()` reads them.

`dev.layout().mat(..).draw()` is R's `layout()`.

### Reusing arguments

Build an options struct once and apply it using `.with()`. Setters called after `.with()` change only that call; anything set before `.with()` is replaced.

```rust
let grid = AblineOptions::default().col("grey").lty("dotted");
dev.abline().with(&grid).h([2.0, 4.0, 6.0, 8.0]).draw();
dev.abline().with(&grid).v([2.0, 4.0, 6.0, 8.0]).draw();
```

Every call that takes arguments has an options struct: `PlotOptions`, `HistOptions`, `BarplotOptions`, `BoxplotOptions`, `PieOptions`, `XyOptions` (for `points` and `lines`), `SegmentsOptions`, `ArrowsOptions`, `RectOptions`, `PolygonOptions`, `TextOptions`, `MtextOptions`, `TitleOptions`, `AxisOptions`, `AblineOptions`, `LegendOptions`, `BoxOptions`, `PlotWindowOptions` and `ParOptions`.

### Figures as values

A `Figure` records the same calls without a device, so you can build a figure (a plot and its margins) once and draw it later, on any device and in any panel. 

Page layout stays with the device. A figure is one panel, so a figure's `par()` has no `mfrow` or `mfcol`.

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
dev.par().mfrow(2, 2).draw();
dev.add_figure_at(1, 1, &fig); // top left
dev.add_figure_at(2, 2, &fig); // bottom right
dev.close()?;
```

A figure has no page size or format of its own, so to save one, open a device for the file and add the figure to it. The same figure can go to several files:

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

Write them once against the `Graphics` trait, and they work on a `Device` and in a `Figure`: you can call them like the built-ins.

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

The crate's documentation (`cargo doc --open`) has guides in `rs_graphics::guide`: R's plotting vocabulary for those who don't know R (`concepts`), how to translate R code (`coming_from_r`), and every R function and argument with its Rust form (`r_reference`). `rs_graphics::charts` draws the symbols, line types, colours and fonts to choose from. For tools that write code, [llms.txt](llms.txt) has the same in one file.

For a more extensive run-down of what is implemented and what isn't, the design notes, the example gallery and the tests, see [DEVELOPER.md](DEVELOPER.md).

## License

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
the notice in `assets/afm/MustRead.html`) and the Liberation Sans fonts
(`assets/fonts/`, SIL OFL 1.1). Their notices, and those of the
third-party crates compiled in, are in [NOTICE](NOTICE). Programs that
use rs-graphics embed these too, so their own notices should include them.
