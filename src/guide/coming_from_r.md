How to write R's graphics calls with this crate: the translation rules,
then complete scripts in both languages. The functions and arguments
mirror R's one for one, so R's documentation and answers to "how do I
... in R base graphics" apply here once translated. Every R function and
argument, with its Rust form, is listed in [`r_reference`](crate::guide::r_reference).

# A call

An R function is a method of the device that starts a call; each
argument is a setter named as in R; `.draw()` runs it.

```r
plot(x, y, pch = 19, col = "red", main = "Weight")
```

```
# use rs_graphics::prelude::*;
# let mut dev = Device::new(SvgBackend::new(7, 7), Par::default());
# let (x, y) = ([1.0, 2.0, 3.0], [2.0, 4.0, 3.0]);
dev.plot().x(&x).y(&y).pch(19).col("red").main("Weight").draw();
```

- Arguments go in any order, all as setters, data included (`.x(&x)`).
  `legend()`'s position and text are `.pos()` and `.labels()` (R's `x`/`y`
  and `legend`).
- Without `.draw()` nothing happens (the compiler warns).
- An argument left out has R's default, often the current `par()` value.
  A required one left out is R's error: `argument "x" is missing, with no
  default`.
- R's errors: `.draw()` panics with R's message (`plot.new(): figure
  margins too large`); `.try_draw()` returns it as an
  [`Error`](crate::Error) instead.
- What R's function returns, `.draw()` returns: `hist()` its
  [`HistResult`](crate::highlevel::HistResult), `barplot()` the bar
  centres, `boxplot()` the statistics, `axis()` the tick positions,
  `legend()` its box and text positions.

# Names

| R | Rust |
|---|---|
| a dot in a name: `cex.main`, `plot.window()` | an underscore: `.cex_main()`, `dev.plot_window()` |
| `box()`, `type =` (Rust keywords) | `dev.box_()`, `.type_()` |
| `orderH`, `axTicks()` | `.order_h()`, `dev.ax_ticks()` |
| `log = "xy"` | `.xlog(true).ylog(true)` |
| `labels = FALSE` (in `axis()`) | `.draw_labels(false)` |

# Values

| R | Rust |
|---|---|
| `c(1, 2, 3)` | `[1, 2, 3]` or `&v` (a `Vec`, array or slice) |
| `1:10` | `1..=10` |
| `TRUE`, `FALSE` | `true`, `false` |
| `NA` in a setting: `pch = c(19, NA)`, `mtext(at = c(1, NA))` | `None`: `.pch([Some(19), None])`, `.at([Some(1.0), None])` |
| `NA` in data or coordinates: `x = c(1, NA, 3)` | `f64::NAN`: that value is skipped, as in R |
| `NA` as a whole argument | leave the setter out |
| `"red"`, `"#FF000080"`, `2` (colours) | the same: `.col("red")`, `.col(2)` |
| `lty = "dashed"`, `lty = 2`, `"44"` | the same: `.lty("dashed")`, `.lty(2)` |
| `pch = "+"` | `.pch('+')` |
| `xlim = c(0, 10)` | `.xlim(0, 10)` |
| `mar = c(5, 4, 4, 2)` | `.mar([5, 4, 4, 2])` |
| `expression(alpha^2)` | `Label::math("alpha^2")` |
| `font = 2` | `.font(2)` or `.font("bold")` |

Numbers may be integers or floats, as in R (`.cex(2)`, `.xlim(0, 10)`); a
single number is a vector of one (`.x(1)`). Rust arrays hold one type, so
once an element is `None` the others are written `Some(..)`.

# Devices

```r
pdf("plot.pdf", width = 7, height = 5)
plot(1:10)
dev.off()
```

```no_run
# use rs_graphics::prelude::*;
# fn main() -> std::io::Result<()> {
let mut dev = Device::pdf("plot.pdf").width(7).height(5).open()?;
dev.plot().x(1..=10).draw();
dev.close()?;
# Ok(())
# }
```

`pdf(NULL)` (no file, to measure or check) is `Device::null().open()?`.
`Device::svg(file)`, `Device::png(file)` likewise; PNG sizes are in
inches unless `.units("px")` (then `.res()` is pixels per inch, as R's
`png(res = )`). The format is the function's, never the file name's. A
file name with a page number (`"page%02d.png"`) writes a file per page,
as in R.

# `par()`

```r
par(mfrow = c(1, 2), mar = c(4, 4, 2, 1), las = 1)
```

```
# use rs_graphics::prelude::*;
# let mut dev = Device::new(SvgBackend::new(7, 7), Par::default());
dev.par().mfrow(1, 2).mar([4, 4, 2, 1]).las(1).draw();
```

- [`par("las")`] (reading a value) is `dev.current_par().las`.
- R's `op <- par(...); ...; par(op)` is reading the values first and
  setting them back: `let mar = dev.current_par().mar;` ...
  `dev.par().mar(mar).draw();`.
- Graphical parameters given to a function (`plot(x, cex.axis = 0.8)`)
  are setters of that call, and last for that call only, as in R.
- `layout(matrix(c(1, 1, 2, 3), 2, byrow = TRUE))` is
  `dev.layout().mat(&[[1, 1], [2, 3]]).draw()`: matrices are written as
  rows.

# Recorded plots

A [`Figure`](crate::recording::Figure) records calls without a device,
like R's `recordPlot()`, and is drawn later with `dev.add_figure(&fig)`
(R's `replayPlot()`), any number of times and on any device. It can hold
one panel (then `add_figure_at(row, col, &fig)` puts it in a chosen one)
or a whole plot with its page layout (`fig.par().mfrow(..)`,
`fig.layout()`) and pages. Write your own plots as functions over the
[`Graphics`](crate::graphics::Graphics) trait and they work on both.

# Where it differs from R

- Sizes are in inches for every device (PNG too, unless `.units("px")`).
- `abline()` draws several `a`/`b` lines; `coef`/`reg` are `.a()` and
  `.b()` with the coefficients.
- R's errors are the same, but a few things R accepts silently are errors
  here (`abline(b = 1)` without `a`), and where R has a bug the intended
  behaviour is implemented.
- Changing `mar` or `oma` after a plot has started applies from the next
  plot (R moves some of the current plot's margins at once).
- Text uses Helvetica's metrics on every device, as R's `pdf()`; R's
  `png()` and `svg()` use the system's fonts.
- `par(family)`, `vfont` and R's other font families aren't supported.

# Complete scripts

A scatter plot with a fitted line and a legend:

```r
x <- c(1, 2, 3, 4, 5); y <- c(2.1, 3.9, 6.2, 7.8, 10.1)
plot(x, y, pch = 19, col = "steelblue", xlab = "Dose", ylab = "Response",
     main = expression(y == beta[0] + beta[1] * x))
abline(lm(y ~ x), col = "red", lty = 2)
legend("topleft", legend = c("observed", "fit"),
       col = c("steelblue", "red"), pch = c(19, NA), lty = c(NA, 2))
```

```
# use rs_graphics::prelude::*;
# let mut dev = Device::new(SvgBackend::new(7, 7), Par::default());
let x = [1.0, 2.0, 3.0, 4.0, 5.0];
let y = [2.1, 3.9, 6.2, 7.8, 10.1];
dev.plot()
    .x(&x)
    .y(&y)
    .pch(19)
    .col("steelblue")
    .xlab("Dose")
    .ylab("Response")
    .main(Label::math("y == beta[0] + beta[1] * x"))
    .draw();
// lm() isn't part of graphics: fit the line, then pass its coefficients.
let (a, b) = (0.05, 1.99);
dev.abline().a(a).b(b).col("red").lty(2).draw();
dev.legend()
    .pos("topleft")
    .labels(["observed", "fit"])
    .col(["steelblue", "red"])
    .pch([Some(19), None])
    .lty([None, Some(2)])
    .draw();
```

Two panels: a histogram with its bin midpoints marked, and a barplot
labelled at its bar centres:

```r
par(mfrow = c(1, 2), oma = c(0, 0, 2, 0))
h <- hist(c(4.2, 5.1, 5.3, 5.9, 6.4, 7.0, 7.7), col = "lightblue", main = "Histogram")
abline(v = h$mids[3], lty = "dashed")
mp <- barplot(c(3, 5, 2), names.arg = c("A", "B", "C"), ylim = c(0, 6))
text(mp, c(3, 5, 2) + 0.3, labels = c("3", "5", "2"))
mtext("Two panels", outer = TRUE, line = 0.5, font = 2)
```

```
# use rs_graphics::prelude::*;
# let mut dev = Device::new(SvgBackend::new(8, 4), Par::default());
dev.par().mfrow(1, 2).oma([0, 0, 2, 0]).draw();
let h = dev
    .hist()
    .x([4.2, 5.1, 5.3, 5.9, 6.4, 7.0, 7.7])
    .col("lightblue")
    .main("Histogram")
    .draw();
dev.abline().v(h.mids[2]).lty("dashed").draw(); // R counts from 1
let heights = [3.0, 5.0, 2.0];
let mp = dev.barplot().height(heights).names(["A", "B", "C"]).ylim(0, 6).draw();
let above: Vec<f64> = heights.iter().map(|h| h + 0.3).collect();
dev.text().x(&mp).y(&above).labels(["3", "5", "2"]).draw();
dev.mtext().text("Two panels").outer(true).line(0.5).font(2).draw();
```

[`par("las")`]: crate::par::Par::las
