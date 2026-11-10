# Changelog

All notable changes to rs-graphics, newest first. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions
follow [Semantic Versioning](https://semver.org/): below 1.0, a minor
version may break the API. Breaking changes are marked **Breaking** and
come with a table of old and new calls.

## [0.6.0] - 2026-11-10

### Changed

- **Breaking:** every R argument is now exactly one setter, named as R's
  argument and taking any kind of value R accepts. The setters that split
  or merged R arguments are replaced:

| Before | After |
|---|---|
| `dev.barplot()...names(["a", "b"])` | `dev.barplot()...names_arg(["a", "b"])` (R's `names.arg`; boxplot's `.names()` is unchanged) |
| `dev.axis().side(1).axp([0, 10, 5])` | `.xaxp([0, 10, 5])` on an x axis, `.yaxp(..)` on a y axis |
| `dev.axis().side(2).axt("n")` | `.yaxt("n")` on a y axis, `.xaxt(..)` on an x axis |
| `dev.axis().side(1).draw_labels(false)` | `dev.axis().side(1).labels(false)` (R's `labels = FALSE`) |
| `dev.plot()...xlog(true).ylog(true)` | `dev.plot()...log("xy")` (also `hist()`, `barplot()`, `boxplot()`, `plot_window()`) |
| `dev.text()...adj(0).vadj(1)` | `dev.text()...adj([0, 1])` (also `legend()`) |
| text `.vadj(v)` without `.adj()` | `let a = dev.current_par().adj;` then `.adj([a, v])` (R's `c(par("adj"), v)`; `[None, Some(v)]` would centre horizontally, R's `c(NA, v)`) |
| legend `.vadj(v)` without `.adj()` | `.adj([0.0, v])` (the default horizontal 0) |
| `dev.par().col(None::<&str>)` (left `col` unchanged) | leave the setter out; `None` is now R's `NA`, a transparent colour (also `bg`, `fg`, `col.axis`, `col.lab`, `col.main`, `col.sub`) |
| `dev.pie()...edges(200)` with a `usize` | any number: `.edges(200)` still compiles |

- `par()`'s colours (`col`, `bg`, `fg`, `col.axis`, `col.lab`, `col.main`,
  `col.sub`) take any colour R's `par()` does: a name or `#RRGGBB[AA]`
  string, a `Color`, a palette number (`.col(2)`; 0 is the background
  colour, a negative number R's "numerical color values must be >= 0"
  error) or `None` (R's `NA`, transparent), looked up in the palette when
  the call runs, as R stores it. `par(lend)`/`par(ljoin)` (and the inline
  setters) take R's integer codes (`0` round, `1` butt/mitre, `2`
  square/bevel, above 2 recycled as R does; negative is R's "invalid line
  end"/"invalid line join"). `pie()`'s `edges` is any number (floored per
  slice, at least 2 edges; not finite is R's "'length.out' must be a
  finite number"). `layout()`'s `mat` takes its rows by value
  (`[[1, 2], [3, 4]]`) or by reference, as the other data setters.
- `axis()` uses `xaxp`/`xaxt` on an x axis and `yaxp`/`yaxt` on a y axis
  and ignores the other direction's, as R does; a given `xaxp` or `yaxp`
  is checked against its log scale whichever the side (R's `invalid value
  specified for graphical parameter "yaxp"`).
- A `log` other than `x`/`y` characters is R's error from
  `plot.window()` (`invalid "log=z" specification`).
- `par()` keeps `xlog`/`ylog` (R's own `par()` names); they only flip the
  current plot's state, and their docs point to `.log()`.
- Docs: the palette is per device (R's `palette()` is session-wide);
  `DeviceBuilder::threads` is marked **Not in R.**, and SVG compression
  noted as not in R;
  the R reference lists boxplot's style setters (`.boxwex()`,
  `.medcol()`, ...) as R arguments, which R takes through `...`; NOTICE
  credits the ColorBrewer palettes; the README is shorter, with one
  explanation of each point and the same examples.

### Added

- `Adj` (`text()`/`legend()` `adj`): one number (R's one-value `adj`),
  two, or two `Option`s; `.adj([None, Some(1)])` is R's `c(NA, 1)`,
  which centres horizontally even under `par(adj = 0)`, as in R.
- `AxisLabels` (`axis()` `labels`): `true`, `false` or labels.
- `Log` (`log`): `"x"`, `"y"`, `"xy"` (any order, repeats allowed).
- `plot()`'s `.ann()` (R's `ann`), as `hist()`, `barplot()` and
  `boxplot()` already had.
- `par(pch)` takes a character or a string (its first character), as R.
- `par(palette)` takes R's palette names (`"Okabe-Ito"`, `"Set 1"`,
  `"R3"`, ..., matched as R's `palette()` does) or colour strings; an
  ambiguous prefix is an error naming the candidates.
- Integer-code setters take ranges, as R's `1:n`: `.col(1..=3)`,
  `.pch(15..=18)`, `.lty(1..=4)`, `.font(1..=4)`, text `pos`, mtext
  `side`.

### Fixed

- `par(usr = )` with an infinite value is R's `invalid value specified for
  graphical parameter "usr"` (before, a panic at the next axis);
  `par(bty = "N")` is an error, as in R (the other letters still take
  either case);
  `hist(add = TRUE)` no longer checks `xlim`/`ylim` on a device (it
  draws no window, as R; a `Figure` already didn't); `pie()` with an
  `edges` too large for R's longest vector is R's `result would be too
  long a vector` (before, a panic).
- `mtext()` recycles `side` and `outer` over the labels, and `barplot()`
  recycles `border` over the bars, as R does.
- Five more of R's argument errors, with R's messages: `axis()` with no
  finite `at` ("no locations are finite"); `arrows()` with a negative or
  non-finite `length` ("invalid arrow head length"), a non-finite `angle`
  ("invalid arrow head angle") or a `code` above 3 ("invalid arrow head
  specification"); `legend(ncol = 0)` (R's "invalid 'times' argument");
  `boxplot()` with a negative `range` ("'coef' must not be negative");
  `text()` with a non-finite `srt` (R's `par(srt)` error).
- `plot.window()` rejects non-finite limits with R's `need finite 'xlim'
  values` (then `'ylim'`), whichever call made the window: `plot()` of
  no data or all-`NaN` data (R's `range()` is `(Inf, -Inf)`), or with a
  `NaN`/`Inf` `xlim`/`ylim`; `hist()`, `barplot()` and `boxplot()` with
  such limits, `barplot()` of no finite height or with `space = NaN`,
  `boxplot()` of no finite data. `hist()` of no finite data is R's
  `invalid number of 'breaks'` (Sturges) or, with a count of breaks, an
  error where R's own message is the quirk `character(0)` (ours:
  `hist.default: pretty() error, breaks=`); with `"FD"` it no longer
  panics. Before, these drew an empty `(0, 1)` window.
- `par()` checks its values as R's `par()` does (`par-common.c`), each
  with R's `invalid value specified for graphical parameter "<name>"`:
  `adj` in 0 to 1; `cex` (and `cex.axis`/`cex.lab`/`cex.main`/`cex.sub`),
  `lwd`, `mex` and `lheight` positive and finite; `mar`, `mai`, `oma`,
  `omi`, `plt` and `pin` non-negative and finite; `omd` in 0 to 1; `mgp`
  and `srt` finite; `las` 0 to 3; `ps` non-negative and finite;
  `xaxp`/`yaxp` with finite limits and, on a linear axis, a positive
  count. Before, `par(cex = 0)` or `par(mar = c(-1, 1, 1, 1))` was
  accepted and the next plot drew nonsense or failed with another error.
- Three crashes found by a pre-release review: `par(mfrow)`/`mfcol`/`mfg`
  with a value beyond R's integer range (`1e300`) overflowed a figure
  count (a panic in debug builds) instead of R's `invalid value specified
  for graphical parameter "mfrow"`; `par(lab)` wasn't checked, so
  `lab = c(0, 0, 7)` or a negative or `NaN` value panicked inside the
  next `plot()` (now R's `par()` error: the first two values at least 1,
  the third at least 0; `par()`'s integer settings read `NaN` and values
  beyond R's integer range as R's `NA`); on a PNG device a glyph larger
  than the whole page (`cex = 1e6`) tried to allocate its full pixel
  bounds and aborted the process (now filled as a clipped path; beyond
  tiny-skia's coordinate range nothing is drawn).


## [0.5.0] - 2026-10-31

### Changed

- **Breaking:** options structs (`PlotOptions`, `AxisOptions`, `ParOptions`, ...) no longer have public fields. Build them with the call's setters and apply them with `.with(&opts)`. Every value now goes through its setter's conversion and checks.
- **Breaking:** two-number settings take one value, an array or a tuple (a tuple may mix integers and floats). This covers `xlim`/`ylim` (plot, hist, barplot, boxplot, plot_window) and `par()`'s `mfrow`/`mfcol`/`mfg`/`fin`/`pin`. Legend `pos` now also takes arrays, with corners as `[[xa, xb], [ya, yb]]`. Legend `inset` also takes a single number, as R recycles it.
- **Breaking:** readers return arrays. `Device::usr()` is `[x0, x1, y0, y1]` (the form `par().usr()` takes), `Device::din()` is `[width, height]` (it can be passed to `.fin()`/`.pin()`), and `BoxStats::conf` is `[low, high]`.
- **Breaking:** there is one way to open a device. `Device::memory("pdf" | "svg" | "png")` opens an in-memory device with every `DeviceBuilder` option, and `finish()` returns the bytes. `Device::new(backend)` is only for custom backends: it no longer takes a `Par` and starts from `Par::default()`. The built-in backends' constructors, `with_*`/`finish*` methods and `AnyBackend::new` are crate-private, and the backends are no longer in the prelude.
- **Breaking:** `DeviceBuilder::open()` and `Device::close()` return `rs_graphics::Error` instead of `io::Error`. File failures are the new `ErrorKind::Io`, and `Error::io_error()`/`source()` give the underlying `io::Error`. Errors are named after R's `pdf`/`svg`/`png` and `dev.off`. `From<Error> for io::Error` keeps `?` working in `io::Result` functions.
- **Breaking:** each string-valued type converts from a string in one way only: `FromStr` or `TryFrom<&str/String/&String>`, both returning `rs_graphics::Error` with the same message. This covers colours, line types/ends/joins, fonts, the `par()` enums, plot types, hist breaks, `OrderH`, formats, units, legend keywords/positions and `BoxWhich`. The public `parse()` functions are removed (`Expression::parse` stays), and so is `ParseValueError`.
- **Breaking:** `LineType::from_code` and `FontFace::from_code` are crate-private. Use `TryFrom<i32>` instead.
- **Breaking:** `legend()`'s `.bty()` takes `par`'s `Bty`, and `LegendBty` is removed.
- **Breaking:** `Units` variants are full words: `Inches`, `Centimetres`, `Millimetres`, `Pixels`. The strings are still R's (`"in"`, `"cm"`, `"mm"`, `"px"`).
- **Breaking:** `AnyBackend::format()` returns `Option<Format>` (`None` for the null device).
- **Breaking:** the prelude no longer exports traits that are only used as bounds (`Numbers`, `Groups`, `Pair`, `Labels`). Setters work without them; import one from `rs_graphics::numbers`/`plotmath` to name it.
- **Breaking:** these items are removed:
  - `LabelValues`: the barplot, boxplot and pie label setters take `Labels`.
  - `LegendPch`: it is now `rs_graphics::setters::Pch`.
  - The `backend::FontFace` re-export: use `font::FontFace`.
- **Breaking:** `Device::layout` is removed as an inherent method. `layout()` is now a `Graphics` method, so calls through the prelude are unchanged.
- In settings, R's `NA` is written `None`. This applies to `par().tck()`, legend `.vadj()`, mtext `line`/`at`/`adj`/`padj`, axis `padj` and legend `text_width`. `NaN` is still accepted. Data and coordinates keep `NaN` for missing values.
- Option and numeric setters take any number type and integer ranges (`.at(1..=5)`, `.lwd(2u8)`), and also an `Option` of a value: `None` leaves the default, or for `par()` leaves the setting unchanged. For example, `.main(title)` works with `title: Option<String>`.

| Before | After |
|---|---|
| `PlotOptions { main: Some("A".into()), ..Default::default() }` | `PlotOptions::default().main("A")` |
| `.xlim(0, 10)` | `.xlim([0, 10])` or `.xlim((0, 10.5))` |
| `dev.par().mfrow(2, 3).draw()` | `dev.par().mfrow([2, 3]).draw()` |
| `dev.par().mfg(1, 2).draw()` | `dev.par().mfg([1, 2]).draw()` |
| `let (x0, x1, y0, y1) = dev.usr();` | `let [x0, x1, y0, y1] = dev.usr();` |
| `let (w, h) = dev.din();` | `let [w, h] = dev.din();` |
| `stats.conf.0` | `stats.conf[0]` |
| `Device::new(SvgBackend::new(7, 7), Par::default())` | `Device::memory("svg").width(7).height(7).open()?` |
| `Device::new(PdfBackend::new(7, 7).with_compression(false), Par::default())` | `Device::memory("pdf").width(7).height(7).compress(false).open()?` |
| `RasterBackend::new(..).with_threads(4)` | `Device::png("out.png").threads(4)` (or `Device::memory("png").threads(4)`) |
| `SvgBackend::finish_svgz()` | `Device::svg("plot.svgz").compress(true)` |
| `Device::new(my_backend, par)` | `Device::new(my_backend)`, then `dev.par()...draw()` |
| `Color::parse("red")` (an `Option`) | `Color::try_from("red")` or `"red".parse::<Color>()` (a `Result<_, rs_graphics::Error>`) |
| `LineType::from_code(2)` | `LineType::try_from(2)` |
| `.bty(LegendBty::None)` / `LegendBty::Box` | `.bty(Bty::N)` / `Bty::O` (or `"n"` / `"o"`) |
| `Units::In`, `Units::Cm`, `Units::Mm`, `Units::Px` | `Units::Inches`, `Units::Centimetres`, `Units::Millimetres`, `Units::Pixels` |
| `rs_graphics::backend::FontFace` | `rs_graphics::font::FontFace` |
| `Err(e) if e.kind() == io::ErrorKind::NotFound` (from `open()`) | `Err(e) if e.kind() == ErrorKind::Io` (`e.io_error()` gives the `io::Error`) |
| `let f: Format = backend.format();` | `let f: Option<Format> = backend.format();` |
| `LegendPch`, `LabelValues` | `rs_graphics::setters::Pch`, `rs_graphics::plotmath::Labels` |
| `use rs_graphics::prelude::*;` to name `Pair`/`Numbers`/`Groups`/`Labels` | `use rs_graphics::numbers::Pair;` (etc.) |
| `.tck(f64::NAN)`, `.vadj(f64::NAN)` (still accepted) | `.tck(None)`, `.vadj(None)` |
| mtext `.at([1.0, f64::NAN])` (still accepted) | `.at([Some(1.0), None])` |

### Added

- `Device::null()`: R's `pdf(NULL)`. The device measures as a PDF device but draws and writes nothing, so it can check that a `Figure` fits or measure text quickly. Its backend is `backend::null::NullBackend`.
- Whole-page figures (R's `recordPlot()`/`replayPlot()`):
  - A `Figure` can record `par()`'s page setters (`mfrow`, `mfcol`, `mfg`, `fig`, `fin`) and `layout()`.
  - `add_figure` replays the page layout, and `add_figure_at` refuses a figure that has one.
  - Errors in these values are found when recording.
- `DeviceBuilder::threads()` (PNG) and `.compress(true)` for SVG, which writes gzipped `.svgz`.
- `pch` accepts strings in `points()`, `lines()`, `plot()` and boxplot's `pch`/`medpch`/`outpch`, as in R: each string's first character, and `""`/`None` draw no symbol.
- `NA` (`None` elements) in barplot `names`/`legend_text`, boxplot `names` and pie `labels`, with R's behaviour:
  - barplot/boxplot names: the tick is drawn with no label.
  - barplot legend: the key is kept without a label.
  - pie: neither the label nor its tick line is drawn.
- `box_().which()` takes R's strings (`"outer"`, with prefixes as R's `match.arg`). `par().plt()` takes any numbers or an `Option`, and `par().lab()` takes any numbers (truncated).
- The first options that are not in R (module `rs_graphics::extra`):
  - `.label_srt(deg)` on `axis()` and on the x axis of `plot`/`hist` (the names axis of `barplot`/`boxplot`) rotates the tick labels.
  - `.panel_bg(col)` on `plot`, `hist`, `barplot` and `boxplot` fills the plot region first.
- `Stroke::dash_pt()` for custom backends: R's dash rule for a stroke.
- Guides as doc-only modules: `rs_graphics::guide` (`concepts`, `coming_from_r`, and `r_reference`, which lists every R function and argument with its Rust setter) and an `llms.txt`.
- `rs_graphics::charts`: reference charts (`pch`, `line_types`, `line_ends`, `fonts`, `colors`, `palette`, `regions`) that draw on any `Graphics`, plus `color_names()` (R's `colors()`).
- Docs: `par(x)` mentions link to the setting, and the README's examples are compiled with the tests.

### Fixed

- Text with a newline (`\n`) is drawn and measured line by line, as R does. Before, it was drawn as one string (shown as `?` in PDFs). This applies to `text()`, `mtext()`, `title()`, `legend()`, `axis()` labels and `barplot()` names, and to `strwidth`/`strheight` and `gap.axis`.
- `legend()` draws its whole box for every `bty` except `"n"`, as R does. Before, values such as `"l"` or `"7"` were rejected.
- pch 9 and 12 draw the cross before the outline, as R does. This changes the order of elements in PDF and SVG output; PNG output is unchanged.
- `par(mfrow/mfcol)` grid counts are truncated as R's `as.integer()` does, and `mfg` 0 gives R's "invalid value" error.

### Removed

- The hidden modules `afm`, `pretty`, `pch` and `named_colors` are private.

## [0.4.0] - 2026-10-27

### Changed

- **Breaking:** `layout()` is a builder like every other R function: `dev.layout().mat(..).widths(..).heights(..).respect(..).draw()`. `.draw()` returns the number of figures, as R does. `respect` takes a `bool` or a grid of cells through the new `LayoutRespect`. `Layout`, `LayoutError` and `MAX_LAYOUT_*` are crate-private, and the prelude exports `LayoutRespect` instead of `Layout`.
- **Breaking:** device queries use R's names:
  - `text_metrics()` is replaced by `strwidth()`/`strheight()` builders with `units` `"user"` (the default, as in R), `"inches"` or `"figure"`.
  - `default_ticks()` is `ax_ticks()` (R's `axTicks`).
  - `width_in()`/`height_in()` are replaced by `din()`.
  - `user_to_inches()` and `inches_to_user()` both take and return a `Point`.
- **Breaking:** conversions that can fail are `TryFrom` (error `rs_graphics::Error`) instead of panicking `From`. This covers every R-string enum, `Color`/`ColorSpec`/`LegendPos` from strings, and `Side`/`FontFace`/`LineType` from numbers. Setter calls compile unchanged.
- **Breaking:** the label-list setters (barplot `names`/`legend_text`, boxplot `names`, pie `labels`) no longer take arbitrary iterators. Collect into a `Vec`. They now also take a single label.
- **Breaking:** options-struct literals need `..Default::default()`, because of a new hidden field.
- Setters no longer panic on a value they cannot convert (`.lty("wavy")`, `.side(5)`, `.col("notacolour")`). The error is returned by `.try_draw()` (`.draw()` panics with it), also through `.with()`, and by `DeviceBuilder::open()` before the file is created. `Label::math` is the one exception.

| Before | After |
|---|---|
| `dev.layout(Layout::new(&[vec![1, 1], vec![2, 3]])?.widths(&[2.0, 1.0]))` | `dev.layout().mat(&[[1, 1], [2, 3]]).widths([2, 1]).draw()` |
| `.respect_matrix(&cells)?` | `.respect(&cells)` |
| `let (w, h) = dev.text_metrics("Title", 1, 1.2);` | `let w = dev.strwidth().s("Title").font(1).cex(1.2).units("inches").get();` (and `dev.strheight()` for the height) |
| `dev.default_ticks(1)` | `dev.ax_ticks(1)` |
| `(dev.width_in(), dev.height_in())` | `dev.din()` |
| `dev.user_to_inches(x, y)` | `dev.user_to_inches(Point::new(x, y))` |
| `let (x, y) = dev.inches_to_user(p);` | `let q = dev.inches_to_user(p);` (a `Point`) |
| `ColorSpec::from("red")` | `ColorSpec::try_from("red")?` |
| `Side::from(1)`, `LineType::from(2)` | `Side::try_from(1)?`, `LineType::try_from(2)?` |
| `.names(v.iter().map(String::as_str))` | `.names(v.iter().map(String::as_str).collect::<Vec<_>>())` |

### Added

- R's errors as values: every call builder has `.try_draw()`, which returns `rs_graphics::Error` (`call()`, `kind()`, `message()`) for each error the R function raises. `.draw()` panics with the same text. The same applies to:
  - `Graphics::try_plot_new()`
  - `Device::try_add_figure()` / `try_add_figure_at()`
  - `layout()`'s `.try_draw()`
  - `strwidth()`'s `.try_get()`
- `rs_graphics::Error` and `ErrorKind` are at the crate root, and `ErrorKind` is in the prelude.
- New errors that match R's checks:
  - `par(xaxp/yaxp)` on a log axis.
  - `axis()` with an unusable log `axp` code (this was a panic).
  - A negative `bg` index at `open()`.
- A `Figure` now checks `text`, `axis`, `abline`, `legend`, `mtext`, `pie` and `lmitre` when recording.
- `DeviceBuilder::units()` sets the units of `width`/`height`: `"in"`, `"cm"` or `"mm"` for every format, and `"px"` for PNG only (divided by `res`). The new `Units` enum is in the prelude.
- Data arguments take a single number, as R's length-one vectors (`.x(1).y(1)` is R's `plot(1, 1)`). One vector passed to `boxplot()` is one group, as in R's `boxplot(x)`.
- `StrUnits` and `LayoutRespect`.
- API docs follow rustdoc's usual layout, with runnable examples. Each setter's docs are the docs of the setting it sets.

### Fixed

- `hist()` of one value or all-equal values (R's `hist(2)`) panicked. It now gives R's breaks and counts.
- Error messages are R's exact text, without a prefix such as `"barplot: "`.
- Colour strings follow R. An unknown name is R's error "invalid color name" (before, nothing was drawn), and a whole-number string such as `"2"` is a palette index, as R's `col = "2"`.

### Removed

- `Device::text_metrics`, `Device::default_ticks`, `Device::width_in`, `Device::height_in`.
- Public `Layout`, `LayoutError` and `MAX_LAYOUT_*`.

## [0.3.0] - 2026-10-23

### Changed

- **Breaking:** every R argument is now a setter named as R's, data included. `Graphics` methods take no arguments. A required argument you leave out panics at `.draw()` with R's message (`argument "x" is missing, with no default`), and so does a `Figure` when it records the call. R's defaults apply to the rest:
  - `plot()`, `points()`, `lines()`, `polygon()` and `text()` accept `x` alone, as R's `plot(v)`, and `plot()` then labels the x axis "Index".
  - `x1`/`y1` default to `x0`/`y0` in `segments()` and `arrows()`.
  - `text()` labels default to `1, 2, ...`.
  - `mtext()` defaults to `side = 3`.
  - `legend()` takes R's `x`/`y` as `.pos()` and R's `legend` as `.labels()`.
- **Breaking:** devices open as in R. `Device::pdf(file)`, `Device::svg(file)`, `Device::png(file)` and `Device::format(format, file)` return a `DeviceBuilder`:
  - Its setters are `width`, `height`, `pointsize`, `bg`, `res` (PNG only), `compress` and `onefile` (PDF only), all with R's defaults. Sizes are in inches for every format; PNG defaults to 480/72 in at 72 ppi.
  - `.open()` creates the file, so a bad path fails before anything is drawn, and returns `io::Result<Device<AnyBackend>>`.
  - `dev.close()` (R's `dev.off()`) writes the file and replaces `dev.save(path)`.
  - The format is never taken from the file name.
  - For a typed backend, use `Device::new(PdfBackend::new(w, h), Par::default())` and `dev.finish()`.
- **Breaking:** `dev.par()` is the only way to change graphical parameters, including R's page-layout ones:
  - `.mfrow()`, `.mfcol()`, `.mfg()`, `.fig()`, `.fin()` and `.new()` exist on a device's `par()` call only. A `Figure` is one panel, so its `par()` has none of them.
  - `Device::{mfrow, mfcol, mfg, fig, set_new}` and the public `Device.par` field are gone. `dev.current_par()` reads the parameters (R's `par("x")`).
  - `par()` calls now apply R's unit rules: `mar`/`mai`/`pty` reset `plt`/`pin`, and setting `oma`/`omi`/`omd` starts a new page at the next plot.
- **Breaking:** `legend()` takes R's per-key vectors as recycled setters: `col`, `pch`, `lty`, `lwd`, `fill`, `border`, `pt_bg`, `pt_cex`, `pt_lwd`, plus per-key `cex`, `x_intersp`, `seg_len`, per-row `y_intersp` and per-column `text_width`. The layout is ported from R 4.x's `legend.R`. `LegendEntry` is removed. `legend().draw()` returns `LegendLayout`, no longer an `Option`, and `FigureResult::legends` is a `Vec<LegendLayout>`. `text_col`/`text_font` are recycled per entry.
- **Breaking:** R's `NA` in a style vector is `None`. Per-element style fields are `Vec<Option<T>>`: `pch`, `cex`, `lwd` and `lty` on points/lines/plot/segments/arrows/rect/polygon/abline/hist, text and mtext `cex`/`font`, every boxplot `*lty`/`*lwd`/`*pch`/`*cex`, and the legend vectors. Colour vectors accept `Option` elements. `None` skips what it styles, or gives the default for text sizes, fonts and legend spacing. Setters still take plain values (`.pch(19)`); only struct literals change.
- **Breaking:** font arguments are `FontFace` (`Plain`, `Bold`, `Italic`, `BoldItalic`, `Symbol`; new module `font`, in the prelude). This covers `Par.font*`, inline `font_*` on title and the high-level plots, and axis, pie, legend, text and mtext fonts. Setters take R's numbers 1-5, names (`"bold"`, `"bold.italic"`, `"symbol"`, ...) or the enum.
- **Breaking:** `Par::xpd` and every inline `xpd` is an `Xpd` (`Plot`, `Figure`, `Device` = R's `FALSE`/`TRUE`/`NA`). `.xpd(true)` still works.
- **Breaking:** `abline()` drops `coef`; use `.a(..).b(..)`. `a` and `b` take several values, one line per pair, and one side may be a single value. `col`/`lty`/`lwd` recycle over the a/b lines, then `h`, then `v`. This departs from R on purpose.
- **Breaking:** options fields that R recycles are now `Vec`s: boxplot `col`/`border` (per box), hist `col`/`border` (per bar), pie `border` and mtext's options. `HistOptions::breaks` is a `HistBreaks`, and boxplot's `ann` is `Option<bool>`. Setters taking one value are unchanged.
- **Breaking:** on a device, `axis().draw()` returns the tick positions (R's return value). `FigureResult` gains `axes`.
- Calls that R rejects now panic with R's message instead of drawing something:
  - `plot_new()` when the regions don't fit ("figure margins too large", ...)
  - `x`/`y` of different lengths in `plot`/`points`/`lines`/`polygon`
  - `hist()` data outside the breaks ("some 'x' not counted"); before, it was dropped silently
  - an empty `legend()`
  - line type codes below 0, negative colour indices, font numbers outside 1-5 (before, drawn plain)
  - invalid strings given to setters
- Plotmath: `max(x)`, `min`, `lim`, `sum` and the like now draw as operators, as in R. A wrong number of arguments is a parse error.
- PNG: outlined plotting symbols (`pch` 1, 3, 8, 21, ...) draw faster on several threads (about 5 times for `pch` 1, 2 to 2.5 times for `pch` 3 and 8). A few edge pixels of stroked circles change slightly.
- Licence notice: NOTICE now lists every translated R file:
  - `main/plot.c` (GPL-3.0-or-later, unlike the rest of R), `main/graphics.c`, `main/format.c`, `main/util.c`, `nmath/fprec.c`
  - `quantile.R`
  - `nclass.*`, `xy.coords` and `checkIntFormat`
  - `polygon.R`'s hatching

  It also adds the copyright holders Ross Ihaka (`signif()`) and Kevin Buhr (polygon hatching). Programs that copy rs-graphics' R section into their own notices should update it.

| Before | After |
|---|---|
| `let mut dev = Device::pdf(7.0, 7.0);` … `dev.save("x.pdf")?;` | `let mut dev = Device::pdf("x.pdf").open()?;` … `dev.close()?;` |
| `Device::png(6.0, 4.0, 300.0)` | `Device::png("x.png").width(6).height(4).res(300).open()?` |
| `Device::for_path("x.pdf", 7.0, 7.0, 300.0)?` | `Device::format("pdf", "x.pdf").open()?` |
| `dev.plot(&x, &y)` | `dev.plot().x(&x).y(&y)` |
| `dev.points(&x, &y)` / `lines` / `polygon` | `dev.points().x(&x).y(&y)` |
| `dev.hist(&x)` | `dev.hist().x(&x)` |
| `dev.barplot([3.0, 5.0, 2.0])` | `dev.barplot().height([3, 5, 2])` |
| `dev.boxplot(&groups)` | `dev.boxplot().x(&groups)` |
| `dev.pie(&x)` | `dev.pie().x(&x)` |
| `dev.segments(&x0, &y0, &x1, &y1)` (also `arrows`) | `dev.segments().x0(&x0).y0(&y0).x1(&x1).y1(&y1)` |
| `dev.rect(&l, &b, &r, &t)` | `dev.rect().xleft(&l).ybottom(&b).xright(&r).ytop(&t)` |
| `dev.text(&x, &y, &["a", "b"])` | `dev.text().x(&x).y(&y).labels(&["a", "b"])` |
| `dev.axis(1)` | `dev.axis().side(1)` |
| `dev.mtext("Title", 3)` | `dev.mtext().text("Title").side(3)` |
| `dev.plot_window((0.0, 1.0), (0.0, 10.0))` | `dev.plot_window().xlim(0, 1).ylim(0, 10)` |
| `dev.legend(LegendPos::Keyword(LegendKeyword::TopLeft), &[LegendEntry::point("a", "black", 19), LegendEntry::line("b", "red", 1.0, LineType::DASHED)])` | `dev.legend().pos("topleft").labels(["a", "b"]).col(["black", "red"]).pch([Some(19), None]).lty([None, Some(2)])` |
| `if let Some(l) = dev.legend(..).draw()` | `let l = dev.legend()….draw();` |
| `dev.abline().coef(0.0, 1.0)` | `dev.abline().a(0).b(1)` |
| `dev.mfrow(1, 2);` | `dev.par().mfrow(1, 2).draw();` |
| `dev.mfg(1, 2);` / `dev.fig([..]);` / `dev.set_new(true);` | `dev.par().mfg(1, 2).draw();` / `.fig([..])` / `.new(true)` |
| `dev.par.cex = 0.8;` | `dev.par().cex(0.8).draw();` |
| `dev.par.cex` (read) | `dev.current_par().cex` |
| `XyOptions { pch: vec![19], xpd: Some(true), .. }` | `XyOptions { pch: vec![Some(19)], xpd: Some(Xpd::Figure), .. }` |
| `Par { font_main: 2, xpd: true, .. }` | `Par { font_main: FontFace::Bold, xpd: Xpd::Figure, .. }` |

### Added

- Files: one file per page when the file name holds a page number, as R's `png("Rplot%03d.png")` (format checked as R's `checkIntFormat()`). For PDF, use a page number plus `.onefile(false)`, as R's `pdf(onefile = FALSE)`.
- Numbers as in R:
  - Integers are accepted wherever R takes numbers (`.cex(2)`, `.xlim(0, 12)`, `.mar([5, 4, 4, 2])`).
  - Data accepts any number type and integer ranges (`.x(1..=10)` is R's `1:10`).
  - Data and option vectors can be passed by value or by reference (`.x(v)`, `.x(&v)`, `.h([1, 2])`), and `f64` data is never copied.
  - New module `numbers` (`Numbers`, `Groups`, `Pair`, in the prelude).
- Setters take R's strings, which also covers `par()` colour setters: `.type_("b")`, `.lty("dashed")`, `.lty(["dashed", "44"])`, `.xaxs("i")`, `.bty("l")`, `.order_h("decr")`, `.col_main("darkblue")`.
- `legend()` positions as R writes them: `"topright"` (or a unique prefix), `(x, y)` and `((xa, xb), (ya, yb))`.
- `par()`: `bty` (new `Bty`), `tck`, `srt`, `mai`, `omi`, `omd`, `pin`, `fin`, `usr`, `xlog`, `ylog` and `ylbias`, and `xpd = NA` (`Xpd::Device`, clipping to the device).
- `plot()`, `points()`, `lines()`: types `"h"`, `"s"` and `"S"`. Inline `font` for character symbols.
- `plot()`:
  - `asp`, `bty`, `frame_plot`
  - `panel_first`/`panel_last` (as `Figure`s)
  - inline `xaxs`/`yaxs`/`lab`, `font`, `fg`, `adj`, `lend`, `xpd`
  - title parameters (`cex/col/font` × `main/sub/lab`)
  - axis parameters (`las`, `cex_axis`, `col_axis`, `font_axis`, `mgp`, `tck`, `tcl`)

  `plot_window()` takes inline `xaxs`/`yaxs`/`lab`.
- `hist()`:
  - every form of R's `breaks` (`HistBreaks`): edges, a number of classes, `"Sturges"`/`"Scott"`/`"FD"`, or a function
  - `xlim`/`ylim`; R's binning with `right`, `include_lowest`, `fuzz`
  - `labels` (`HistLabels`)
  - per-bar `col`/`border`/`lty`; `density`/`angle`
  - `plot = FALSE`; `HistResult::equidist`
  - inline title and axis parameters
- `barplot()`:
  - `offset`, `order_h` (`OrderH`), `add`, `ann`
  - R's `args.legend` as `.legend_pos()` + `.args_legend()`
  - `axisnames`, `cex_names`, `axis_lty`, `xpd`
  - `plot = FALSE`; `density`/`angle`
  - inline title and axis parameters
- `boxplot()`:
  - `at`, `width`, `add`, `outline`
  - log axes (`xlog`/`ylog`), `xlim`/`ylim`
  - `boxwex`/`staplewex`/`outwex`
  - every `bxp()` style (`box*`, `med*`, `whisk*`, `staple*`, `out*`), recycled per box, plus `lty`/`lwd`/`pch`/`cex`/`bg`
  - `show_names`, `frame_plot`, `xaxt`/`yaxt`, `xaxp`/`yaxp`
  - inline title and axis parameters
- `pie()`: `srt`, `cex`, `font`, per-slice `border`, `lty`, `density`/`angle`, and inline `main` parameters.
- `legend()`: `box_lwd`/`box_lty`/`box_col`, `title_col`/`title_adj`/`title_font`, `plot = FALSE`, `density`/`angle`, `adj`/`vadj`. Character `pch`, including R's split of `"abc"` and non-ASCII characters. A `None` label keeps its key.
- `axis()`: inline `mgp`, `xpd`, `lend`.
- `title()`: `adj` (it also follows `par(adj)`), `line`, `outer`, `mgp`, `xpd`, and `cex/col/font` per title.
- `text()`: `pos` and `offset`. `None` labels are skipped.
- `mtext()`: text and arguments recycled over vectors, as in R. Inline `las` and `xpd`. `None` labels are skipped.
- `box()`: `bty` and `fg`.
- Inline `lend`/`ljoin`/`lmitre` on `points`, `lines`, `segments`, `arrows`, `rect`, `polygon`, `box` and `abline`. Inline `xpd` on `abline()`.
- Shading lines (R's `density`/`angle`) in `rect()`, `polygon()`, `barplot()`, `hist()`, `pie()` and legend boxes. `polygon()` also takes `fill_odd_even`.
- Plotmath:
  - fractions (`frac`, `over`, `atop`), `sqrt`/`root`
  - accents (`hat`, `tilde`, `bar`, `widehat`, `widetilde`, `ring`, `dot`)
  - `abs`, `underline`, `bgroup`, `integral`
  - big operators with limits (`sum`, `prod`, `union`, `intersect`, `lim`, `min`, `max`, ...)
  - `{}`

### Fixed

- PDF: semi-transparent colours were drawn opaque. They are now written with `ExtGState` alpha, as R's `pdf()` does.
- PDF: strings are kerned inside `TJ` arrays, as R's `pdf()` does. Where R skips a string's last kerning pair (an R bug), we kern it.
- `hist(breaks = 20)` took the single value as one bin edge. It is now a number of classes, as in R. Unsorted break edges are sorted.
- `title()` always centred its titles. It now follows `par(adj)`.
- Plotting symbols always used round caps and joins. They now follow `par(lend)`/`par(ljoin)`, as R's `GSymbol`.
- With `par(mex)` other than 1, axis labels and `title()`'s sub/xlab/ylab sat about 0.4 lines off.
- `text()` at `NaN` or non-positive log positions wrote `NaN` into the PDF, which made the file invalid. Such labels are now skipped.
- `polygon()` runs of fewer than two points are skipped, as in R.
- `plot()`/`points()`/`lines()`/`polygon()` with `x` and `y` of different lengths silently drew the shorter one. They now give R's error.
- `par(mar = )`/`par(pty = )` didn't reset an earlier `par(plt = )`, and `par(oma = )` didn't start a new page.
- Line type codes above 6 recycle as in R (7 is solid, 8 dashed). Spaces in colour names are ignored (`"Light Blue"`).
- PNG: a new page clears to the page background, so a transparent page no longer shows the previous one.

### Removed

- `Device::for_path`, `AnyBackend::for_path` and `Format::from_path`: the format is no longer taken from file extensions.
- `Device::save`.
- `Device::rect_inches` and `Device::backend_mut`.
- `LegendEntry`.
- `AblineOptions::coef`.
- The public `Device::{mfrow, mfcol, mfg, fig, set_new}` methods and the public `Device.par` field.

## [0.2.0-pre] - 2026-10-20

### Changed

- **Breaking:** every R graphics function is now a method of the new `Graphics` trait (in the prelude), and calls are builders. The method takes R's data arguments, chained setters take the rest (the same setters as before), and `.draw()` runs the call and returns what R's function returns. The high-level free functions are now methods too: `dev.plot()`, `hist()`, `barplot()`, `boxplot()` and `pie()` start a new plot (the next panel), and the other methods add to it. To reuse arguments, build an options struct and pass it with `.with(&opts)`. Calls are `#[must_use]`, so leaving out `.draw()` gives a compiler warning. `plot_new()` is now a `Graphics` method, so code that doesn't use the prelude needs `use rs_graphics::graphics::Graphics`.
- **Breaking:** `barplot_matrix()` is folded into `barplot()`, which takes a vector or a matrix (as columns) through `BarHeights`.
- **Breaking:** `mtext()` takes the side as an argument, and `MtextOptions.side` is gone. `Side` is now an enum (`Bottom`, `Left`, `Top`, `Right`). R's numbers 1 to 4 are still accepted wherever a side is taken, and any other number panics.
- **Breaking:** `legend()` now returns R's return value in `usr` units: `LegendLayout { left, top, w, h, text_x, text_y }`, where `text_x`/`text_y` are the label positions. Before, it returned the box only, in inches. `LegendLayout::rect()` is removed.
- **Breaking:** some names now follow R's: `PieOptions::init_angle_deg` is `init_angle`, `BoxplotOptions::range_coef` is `range`, `LegendOptions::bty_box: bool` is `bty: LegendBty` (`Box`/`None`, R's `"o"`/`"n"`), and `Layout::with_widths`/`with_heights`/`with_respect`/`with_respect_matrix` are `widths`/`heights`/`respect`/`respect_matrix`.
- **Breaking:** every argument R lets you omit is now an `Option`, or a `Vec` that is empty for the default, and each field's docs say what `None` gives. Setters are unchanged, but struct literals need `Some(..)`. This affects:
  - arrows `length`/`angle`/`code`
  - text `srt`
  - box `which`
  - axis `lty`/`lwd`
  - legend `cex`, `x_intersp`, `y_intersp`, `seg_len`, `ncol`, `inset`, `border`, and `LegendEntry::pt_bg`
  - mtext `line`
  - plot `type_`
  - hist `col`/`border`/`freq`
  - barplot and pie `col` (now a plain `Vec`) and `border`
  - boxplot `col`/`border`/`range`
  - pie `radius`/`edges`
- **Breaking:** barplot `names`/`legend_text`, boxplot `names` and pie `labels` are now `Vec<Label>`, so they can hold plotmath expressions. Setters taking strings still work.
- Changing `mar`/`oma` after a plot has started now takes effect at the next plot for `title()`, `mtext()` and outer axes too, as it already did for the data. R moves margin text at once but leaves the data where it was; this crate does not copy that.
- `hist()` and `boxplot()` now draw their titles whatever `par(ann)` is, as in R: their `ann` argument defaults to `TRUE`. `barplot()` still follows `par(ann)`, as R does.
- `hist()` now plots densities when the breaks have unequal widths, unless `freq` is set (R's `freq` default).
- The borders of `hist()`, `barplot()`, `boxplot()` and `pie()` now default to `par(fg)` instead of colour 1. `boxplot()` medians, staples and box outlines now use `par(lty)`, and its outliers use `par(pch)`.

| Before | After |
|---|---|
| `plot(&mut dev, &x, &y, &PlotOptions::default().pch(19))` | `dev.plot(&x, &y).pch(19).draw()` |
| `let h = hist(&mut dev, &x, &opts)` | `let h = dev.hist(&x).with(&opts).draw()` |
| `barplot(&mut dev, &heights, &opts)` | `dev.barplot(&heights).with(&opts).draw()` |
| `barplot_matrix(&mut dev, &columns, &opts)` | `dev.barplot(&columns).with(&opts).draw()` |
| `boxplot(&mut dev, &groups, &opts)` | `dev.boxplot(&groups).with(&opts).draw()` |
| `pie(&mut dev, &x, &PieOptions::default())` | `dev.pie(&x).draw()` |
| `dev.abline(&AblineOptions::default().h(6.0).col("grey"))` | `dev.abline().h(6.0).col("grey").draw()` |
| `dev.axis(1, &AxisOptions::default().las(1))` | `dev.axis(1).las(1).draw()` |
| `dev.points(&x, &y, &XyOptions::default().pch(19))` | `dev.points(&x, &y).pch(19).draw()` |
| `dev.text(&x, &y, &labels, &TextOptions::default())` | `dev.text(&x, &y, &labels).draw()` |
| `dev.segments(&x0, &y0, &x1, &y1, &opts)` | `dev.segments(&x0, &y0, &x1, &y1).with(&opts).draw()` |
| `dev.plot_window((0.0, 1.0), (0.0, 1.0), &PlotWindowOptions::default())` | `dev.plot_window((0.0, 1.0), (0.0, 1.0)).draw()` |
| `dev.box_(&BoxOptions::default())` | `dev.box_().draw()` |
| `dev.title(&TitleOptions::default().main("Title"))` | `dev.title().main("Title").draw()` |
| `dev.mtext("Note", &MtextOptions::default().side(4).line(1.0))` | `dev.mtext("Note", 4).line(1.0).draw()` |
| `dev.legend(pos, &entries, &LegendOptions::default())` | `dev.legend(pos, &entries).draw()` |
| `LegendOptions::default().bty_box(false)` | `LegendOptions::default().bty(LegendBty::None)` |
| `legend_layout.rect()` (inches) | `legend_layout.left`, `.top`, `.w`, `.h` (`usr` units) |
| `PieOptions::default().init_angle_deg(90.0)` | `PieOptions::default().init_angle(90.0)` |
| `BoxplotOptions::default().range_coef(3.0)` | `BoxplotOptions::default().range(3.0)` |
| `Layout::new(&mat)?.with_widths(&w).with_respect(true)` | `Layout::new(&mat)?.widths(&w).respect(true)` |
| `ArrowsOptions { length: 0.1, ..Default::default() }` | `ArrowsOptions { length: Some(0.1), ..Default::default() }` |
| `rs_graphics::highlevel::gray_colors(n)` | `rs_graphics::color::gray_colors(n)` (still in the prelude) |
| `dev.par.xlog` | `dev.xlog()` |

### Added

- `Figure`: records `Graphics` calls without a device, so a figure (a plot and its margins) can be built once and drawn later, on any device and in any panel, giving the same output as making the calls on the device. It is drawn with `Device::add_figure` or `add_figure_at(row, col, ..)`, which return a `FigureResult` whose `legends` say where each legend went. `hist`/`barplot`/`boxplot` results are worked out when the calls are recorded. `par` is restored after the figure is drawn.
- `dev.par()`: R's `par()` as a call (`ParOptions`: every `Par` field, plus `new`), for example `dev.par().las(1).mar([4.0, 4.0, 2.0, 1.0]).draw()`. In a `Figure`, it is the way to change graphical parameters. Assigning `dev.par.<field>` still works.
- You can write your own plots once, as functions or extension-trait methods generic over `G: Graphics`, and they then work on both a `Device` and a `Figure`.
- `hist()` and `boxplot()` take R's `ann` argument (`.ann(..)`).
- `Device::xlog()`/`ylog()` (R's `par("xlog")`/`par("ylog")`) and `Device::inches_to_user()`.
- `Side::ALL`, `Side::number()` and `Side::is_horizontal()`.
- The prelude now also exports `Graphics`, `Figure`, `FigureResult`, `ParOptions`, `Side`, `LegendBty`, `LegendLayout`, `BarHeights`, `Point` and `Rect`.
- The README is rewritten as worked usage examples: R side by side, multi-panel pages, reusing arguments, figures as values, and your own plots. The crate docs explain high- and low-level functions, multi-panel pages and writing your own plots.

### Fixed

- `plot()`'s axes now use `par(col.axis)` and the other axis parameters, as R's `localAxis` does. Before, the label colour was forced to `par(fg)`.
- `Device::text_metrics` now applies `par(lheight)` to the height of multi-line strings, as R's `strheight()` does.
- No more panics on input that should mean "use the default": an empty barplot `width`/`space` or an empty hist `breaks` now gives the default, and a missing pie label is skipped (R's `NA`).

### Removed

- **Breaking:** the free functions `plot`, `hist`, `barplot`, `barplot_matrix`, `boxplot` and `pie`, and the `dev.f(data, &Options)` methods. Use the `Graphics` methods instead (see the table above).
- **Breaking:** `Par::xlog`/`ylog` (it was unused; log axes are set by `plot_window`, `plot` and the other high-level calls; read them with `Device::xlog()`/`ylog()`). `Par::font_face()` is no longer public.

## [0.1.0-pre] - 2026-10-19

### Added

Initial release: a port of R's base graphics, aiming for visual and numerical fidelity with R's output.

- **Devices and output**
  - `Device::pdf`, `svg`, `png(w, h, dpi)` and `for_path` (which picks the format from the file extension), or `Device::new(backend, Par { .. })` for other starting parameters. `finish()` returns the file's bytes and `save(path)` writes them.
  - PDF: one page per R page; Flate-compressed by default (`PdfBackend::with_compression(false)` is R's `compress = FALSE`); a transparent background, as R's `pdf()` (`with_background` is R's `pdf(bg = )`); the standard Helvetica/Symbol fonts, so no font embedding is needed; Latin-1 text.
  - SVG: real `<text>` elements; same-style shapes grouped in one `<g>`; `SvgBackend::finish_svgz()` for gzip-compressed output.
  - PNG: drawn with tiny-skia and a bundled Liberation Sans, painted on several threads (`RasterBackend::with_threads`), with the resolution recorded in the file.
  - Text layout uses Helvetica AFM metrics, as R's `pdf()` does, and is the same in all three formats.
- **High-level plots** (free functions taking `&mut Device`, the data and an options struct)
  - `plot()`, with `type` `"p"`, `"l"`, `"b"`, `"c"`, `"o"` and `"n"`.
  - `hist()`: Sturges breaks by default, plus `breaks`, `freq` and `add = TRUE`.
  - `barplot()`/`barplot_matrix()`: stacked or `beside`, `horiz`, `legend.text`, `space`, `width`, `xlim`/`ylim`.
  - `boxplot()`: Tukey's `fivenum()` hinges, `range`, `notch`, `varwidth`, `horizontal`. Also `boxplot_stats()`.
  - `pie()`.
  - Log axes (`xlog`/`ylog`) for `plot()`, `hist()` and `barplot()`.
- **Low-level functions** (`Device` methods taking data and an options struct)
  - `plot_new()` and `plot_window()` (log axes, `asp`).
  - `points`, `lines`, `segments`, `arrows` (R's open heads, `length`/`angle`/`code`), `rect`, `polygon`, `text`, `box_` (R's `box()`) and `title`.
  - `abline`: `h`/`v` vectors, `coef`, `untf`, log axes.
  - `mtext()`: all four sides, outer margins, `line`/`at`/`adj`/`padj`.
  - `axis()`: `at`, `labels` (including `NA`), `line`, `pos`, `outer`, `tick`, `tck`/`tcl`, `hadj`/`padj`, `gap.axis`, line and colour arguments, `xaxt`/`yaxt = "n"`.
  - `legend()`: a port of R's layout, with keyword positions, `inset`, `ncol`/`horiz`, `title`, filled boxes and merged point-and-line keys.
- **Options structs** with chainable setters and public fields. Unset fields are R's defaults, mostly the current `par()` values. Arguments R recycles per element are `Vec`s, recycled the same way. `rs_graphics::prelude::*` imports everything.
- **`par()`**: `dev.par.<name> = ..`, with R's defaults: margins (`mar`, `oma`, `mgp`, `mex`); axis styles (`xaxs`/`yaxs`, `xaxt`/`yaxt`, `xaxp`/`yaxp`, `lab`, `las`, `tcl`); colours, `cex`, fonts and `ps`; lines (`lty`, `lwd`, `lend`, `ljoin`); `xpd`, `ann`, `pty = "s"` and `plt`.
- **Multi-panel pages**: `Device::mfrow`/`mfcol` (including R's automatic `cex`), `mfg`, `layout()` (a `Layout`) with spanning figures, relative and `lcm()` sizes and `respect`, `fig()` (R's `par(fig = )`) and `set_new(true)` (R's `par(new = TRUE)`).
- **Plotmath** (`Label::math("alpha[i]^2")`): super/subscripts, Greek letters and symbols, operators, relations, `paste()`, `list()`, `group()`, font styles, laid out as R's `plotmath.c` does. Used in axis labels, `title()`, `mtext()`, `text()`, `legend()` labels and title, and the titles of the high-level plots. Accents, fractions, `sqrt`, big operators and `bgroup` are not supported yet.
- **Symbols, colours and line types**: `pch` 0 to 25 and character symbols; R's 657 named colours, hex colours with alpha, and the R 4 default palette (`ColorSpec::Index`), colour 0 being `par("bg")`; `LineType` (R's `lty` codes and hex dash strings).
- R's `pretty()` and `GEPretty()` tick algorithms, R's log-axis ticks, and `Device::text_metrics` (R's `strwidth()`/`strheight()`).
- Licensed GPL-3.0-or-later. NOTICE lists R's copyright holders and the ported R sources.

[0.6.0]: https://github.com/MRCIEU/rs-graphics/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/MRCIEU/rs-graphics/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/MRCIEU/rs-graphics/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/MRCIEU/rs-graphics/compare/v0.2.0-pre...v0.3.0
[0.2.0-pre]: https://github.com/MRCIEU/rs-graphics/compare/v0.1.0-pre...v0.2.0-pre
[0.1.0-pre]: https://github.com/MRCIEU/rs-graphics/releases/tag/v0.1.0-pre
