# rs-graphics: developer notes

What is implemented and what isn't, design notes, the example gallery,
testing, and maintaining the licence notices. For an introduction and
usage, see [README.md](README.md).

## What's implemented

**Graphics engine** (`src/`):
- `par()`-equivalent state (`src/par.rs`): margins (`mar`/`oma`), axis
  styles, colours, line/point styles, fonts, `tcl`/`mgp`/`las`, etc.,
  with R's exact default values.
- Coordinate systems and clipping (`src/device.rs`): `plot.new()`/
  `plot.window()` equivalents (`plot_new()` alone leaves R's default
  0..1 user coordinates), user/device coordinate transforms, log
  axes (R's default log ticks from `GLPretty()`/`CreateAtVector()`, `at`/
  `pos`/`abline(h=, v=)` in data units, `abline(a, b)` straight on the
  page or curved with `AblineOptions::untf`, and `xlog`/`ylog` for `plot()`,
  `hist()`, `barplot()` and `boxplot()`), `par(lab=)`, `par(xaxp=/yaxp=)` (set by
  `plot_window()` as R's `GAxisPars()`, read by `axis()` via
  `CreateAtVector()`, overridable inline), and R's `xpd` clip regions
  (`par::Xpd`: the plot region, the figure region or the whole device,
  R's `FALSE`/`TRUE`/`NA`).
- Drawing primitives: `points`, `lines`, `segments`, `rect`, `polygon`,
  `text`, `mtext`, `box`, `abline`, `axis`, `title`, `legend`, `arrows`
  - each a `Graphics` method (`src/graphics.rs`) returning a call
  (`dev.points().x(&x).y(&y).pch(19).draw()`): its data and other
  arguments are its setters, or an options struct passed with
  `.with(&opts)`, and `.draw()` runs it; the drawing code is in
  `src/device.rs` and `src/device/*.rs` (R's `box()` is `box_()`;
  `par(bty)`/`box_().bty(..)`/`plot().bty(..)` choose the sides of the
  plot box as R's `GBox()`). `title()` and the high-level plots take R's
  inline `cex.main`/`col.main`/`font.main` and the `sub`/`lab` ones
  (`boxplot()` only `cex`/`col`, `pie()` only `main`, as R).
  Numbers may be integers, as in R: data vectors are any
  `numbers::Numbers` (`&[f64]` borrowed as is; other number types,
  ranges `1..=10`, converted once; `Groups` for `boxplot()`, `Pair` for
  `(x, y)` arguments), and numeric setters take integers
  (`src/setters.rs`; `array` kind for `mgp`/`xaxp`, `pair` kinds for
  limits).
  `mtext()` (`Graphics::mtext` + `MtextOptions`, `src/device/mtext.rs`)
  takes the side as `.side()` (default 3, as R), and
  covers all four sides of the figure and outer (`oma`) margins with
  `line`/`at`/`adj`/`padj`/`cex`/`font` and `par(las)`; `arrows()` draws
  R's open heads with `length`/`angle`/`code` (so `angle = 90, code = 3`
  gives error-bar caps). `axis()` (`Graphics::axis` + `AxisOptions`,
  `src/device/axis.rs`) takes R's `line`/`pos`/`outer`, `tick`/`labels`
  (`None` entries are `NA`: tick drawn, label skipped; labels may be
  plotmath expressions, `Label::math("10^5")`),
  `tck`/`tcl`, `hadj`/`padj`, `gap.axis`, `lty`/`lwd`/`lwd.ticks` and
  `col`/`col.ticks`/`col.axis`/`cex.axis`/`font`/`las`, and
  `xaxt`/`yaxt = "n"` (`Par::xaxt`/`yaxt`, `AxisOptions::axt`,
  `PlotOptions::xaxt`/`yaxt`).
- Plotmath (`src/plotmath.rs`, `src/device/plotmath.rs`): expressions in
  R syntax (`"alpha[i]^2"`, `"-log[10](italic(p))"`, `"x %+-% y"`) parsed
  into R's call tree and laid out as `plotmath.c` does: super/subscripts,
  Greek letters and maths symbols from the Symbol font (`FontFace::Symbol`,
  R's `font = 5`; PDF uses the standard `/Symbol` font, SVG/PNG its
  Unicode equivalents), operators, relations, spacing, `paste()`,
  `list()`, parentheses, `group()` and `bgroup()` (delimiters grown from
  the Symbol font's pieces), `{}`, fractions (`frac`/`over`/`atop`, in
  TeX's cramped and numerator/denominator styles), `sqrt()` (with an
  index), accents (`hat`, `tilde`, `dot`, `ring`, `bar`, `widehat`,
  `widetilde`), `abs()`, `underline()`, big operators with limits
  (`sum`, `prod`, `integral`, `union`, `intersect`, `lim`, `min`, `max`,
  `inf`, `sup`, ...), font styles, and generic `f(x)`. In PNG output the
  Symbol glyphs that Liberation Sans lacks (delimiter and integral
  pieces, `%in%`, the double arrows, ...) are drawn as missing glyphs;
  bundling a fallback maths font (e.g. Noto Sans Math, OFL) for them
  would fix this, but the delimiter and integral pieces may not join
  cleanly at Symbol's metrics.
  Used by `axis()` (labels), `title()` (`TitleOptions`), `mtext()`,
  `text()` and `legend()` (entry labels and title), each
  placed as R's `GMMathText`/`GMathText` do; `legend()` sizes rows and
  columns by the expressions' boxes. The high-level plots' titles
  (`main`/`sub`/`xlab`/`ylab` in `PlotOptions`, `HistOptions`,
  `BarplotOptions`, `BoxplotOptions`, `PieOptions`) are `Label`s too.
  `legend()`
  (`Graphics::legend`, `src/device/legend.rs`) is a port of R's layout
  algorithm incl. keyword positions, `inset`, `ncol`/`horiz`, `title`,
  filled boxes and merged point+line keys, and returns R's `rect` and
  `text` positions (`LegendLayout`, in `par("usr")` units). Positions are
  written as in R (`"topright"` with `match.arg()` prefixes, `(x, y)`,
  `((xa, xb), (ya, yb))`) or as `LegendPos` variants.
- Coordinates: `Device::usr()` is R's `par("usr")` (log10 on log axes),
  `Device::xlog()`/`ylog()` tell which axes are logarithmic, and
  `user_to_inches`/`inches_to_user` convert between data units and
  device inches.
- Multi-panel layouts (`src/layout.rs`, the `layout()` builder,
  and `mfrow`/`mfcol`/`mfg` on a device's `par()` call):
  `par(mfrow=)`/`par(mfcol=)` (incl. R's automatic `cex` of
  0.83/0.66 for 2x2 / larger grids), `par(mfg=)`, `oma`, and
  `layout()` with spanning figures, relative / `lcm()` widths and
  heights, and `respect` (logical or matrix) - a port of `layoutRegions()`
  in `graphics.c`. `par(new = TRUE)` (`ParOptions::new`) overlays the
  next plot on the current figure, e.g. for a second y axis.
  `par(fig=)` (`ParCall::fig`) places a figure anywhere on the page and
  `par(fin=)` (`ParCall::fin`) gives it a size in inches, `par(plt=)`
  (`Par::plt`) places the plot region within the figure and `par(pin=)`
  (`Par::pin`) gives it a size in inches, and `par(pty = "s")`
  (`Par::pty`) makes it square - ports of `mapFigureRegion()`/
  `mapPlotRegion()`/`updateFigureRegion()`/`updatePlotRegion()`.
  Margins also come in inches (`mai`, `omi`) and the inner region as
  device fractions (`omd`). As in R, the unit set last wins: a `par()`
  call setting `mar` clears `mai` and puts the plot region back on the
  margins (also `pty`), `oma` clears `omi`/`omd`, and setting the outer
  margins starts a new page at the next `plot_new()` ("force eject").
- R's built-in plotting characters `pch = 0..25` (`src/pch.rs`) plus
  character symbols (`pch = "."`, `"a"`, ...), line
  types `lty` (`src/lty.rs`; `LineType::SOLID`, `DASHED`, ... for R's
  codes 0-6), the R4 default colour palette and all 657
  named colours (`src/color.rs`, `src/named_colors.rs`; setters take the
  names, `.col("red")`, and `ColorSpec::try_from("red")` converts one), and the exact
  `pretty()`/`GEPretty()` axis tick-placement algorithms - both of
  which R uses, with **different constants** (`src/pretty.rs`).
- Text layout (`src/afm.rs`): Helvetica and Symbol AFM metrics (the same tables
  R's own `pdf()`/`postscript()` devices use) drive *all* string
  width/height/justification math, shared identically across all three
  backends - see [Design notes](#design-notes) below.

**High-level plots** (`src/highlevel/`), each a fairly direct port of
the corresponding `*.default` R function:
- `plot()` - scatter/line plots (every `type`: `"p"/"l"/"b"/"c"/"o"/"h"/"s"/"S"/"n"`).
- `hist()` - histograms, including R's Sturges'-rule default binning and
  `add = TRUE`.
- `barplot()` - vectors and matrices (one method; a matrix is given as
  its columns, `Vec<Vec<f64>>`), stacked or `beside`, vertical or
  `horiz`, with `legend.text` and `args.legend`, `space`, `width`,
  `xlim`/`ylim`, `offset`, `orderH` and `add = TRUE`.
- `boxplot()` - Tukey boxplots (via `fivenum()`, not `quantile()` -
  see [Design notes](#design-notes)), including the 1.5*IQR outlier rule,
  `notch`, `varwidth`, `horizontal`, `at`, `width`, `add`, `outline`, log
  axes, `xlim`/`ylim` and `bxp()`'s drawing parameters (`boxwex`,
  `staplewex`, `outwex`, `lty`/`lwd`/`pch`/`cex`/`bg` and the `box*`,
  `med*`, `whisk*`, `staple*` and `out*` styles).
- `pie()` - pie charts.

**Backends** (`src/backend/`):
- `svg.rs` - hand-rolled SVG writer (real `<text>` elements, not
  outlined paths). Consecutive shapes with the same fill/stroke style are
  wrapped in one `<g>` carrying it, so a big scatter plot doesn't repeat
  the style on every point (about a third of the size).
  `Device::svg(file).compress(true)` writes the document gzip-compressed
  (`.svgz`, about 4-6x smaller again) instead of plain SVG. It isn't the
  default: browsers display `.svgz` only when served with
  `Content-Encoding: gzip`, though Inkscape and Illustrator open it.
- `pdf.rs` - hand-rolled PDF writer (one page per R page), using the 4 standard
  Type1 Helvetica faces (plus Symbol for plotmath) every PDF viewer bundles (no font embedding
  needed), matching R's own `pdf()` device's default font. Like R's
  `pdf()`, the content stream is Flate-compressed by default (deflated in
  pieces on all cores); `.compress(false)` is R's `compress = FALSE`,
  whose operators can be read as text (the `*_matches_r` tests use it,
  through `common::r::device`).
- `raster.rs` - PNG via [`tiny-skia`](https://docs.rs/tiny-skia) for
  shapes/paths/clipping and [`ab_glyph`](https://docs.rs/ab_glyph)
  (bundled Liberation Sans) for glyph rasterization.

## Out of scope

To keep this a tractable "core functionality" port, the following R
`graphics` features are **not implemented**: `split.screen()`,
`persp()`/3D plotting,
`mosaicplot()`, `stars()`, `coplot()`, `filled.contour()`,
`image()`/raster images (`rasterImage()`), `symbols()`,
`smoothScatter()`, `cdplot()`/`spineplot()`, `dotchart()`,
`fourfoldplot()`, `contour()`, `matplot()`, `pairs()`, `stripchart()`,
`sunflowerplot()`, `assocplot()`, `curve()`, `grid()`, `rug()`,
`xspline()`, `polypath()`, `clip()`, `layout.show()`, font families
(`par(family)`) and Hershey fonts, and interactive input
(`identify()`/`locator()`). These would be reasonable follow-ups but are
left for future work.

## Design notes

A few fidelity-relevant decisions worth calling out:

- **AFM-driven layout, shared across backends.** All string
  width/height/justification math (`strwidth()`/`strheight()`-
  equivalents, `adj`/`hadj`/`vadj`/`srt` resolution) is computed once,
  in `src/device.rs`/`src/afm.rs`, using the real Helvetica AFM metrics
  - the same tables R's own PostScript-family devices use. Backends
    never do their own text-layout math; they only paint a pre-resolved,
  baseline-anchored run of text. This is what keeps tick/label/legend
  placement identical across SVG, PDF, and PNG output.
- **Liberation Sans for PNG glyph shapes.** The raster backend
  rasterizes glyphs with `ab_glyph` and a bundled Liberation Sans
  (SIL OFL; licence in `assets/fonts/LICENSE-LiberationSans.txt`), since
  there's no portable way to rasterize the standard PDF Helvetica
  outlines without a real font file. Liberation Sans is metric-compatible
  with Arial, whose advance widths equal Helvetica's, so glyphs sit in
  the AFM-derived positions without collisions or gaps. Glyph *positions*
  still come from the Helvetica AFM metrics above, so strings land in
  the same place as the SVG/PDF output. Glyph shapes are Arial-like
  rather than literally Helvetica; SVG and PDF output use real
  Helvetica.
- **R4 default palette.** `src/color.rs`'s `DEFAULT_PALETTE` is R
  (>= 4.0.0)'s default `palette()`, used to resolve small-integer
  `col =` values (`col = 1` is black, `col = 2` is R's red-ish, etc).
- **`pretty()` vs. `GEPretty()`.** R has two different "pick nice axis
  breaks" routines sharing one C core (`R_pretty()`) but called with
  *different constants*: `GEPretty()` (used by `axis()`/
  `plot.window()`'s default ticks) and base R's `pretty.default()`
  (used by, e.g., `hist()`'s default bin edges). This crate ports both
  as separate entry points in `src/pretty.rs` (`ge_pretty` and
  `pretty_r`) - conflating them was an early bug caught by this crate's
  own `hist()` vs. real-R cross-check test.
- **`boxplot.stats()` uses `fivenum()`, not `quantile()`.** R's box
  hinges come from Tukey's five-number summary (`stats::fivenum()`),
  which is a different (and simpler) algorithm than
  `quantile(type = 7)`. `src/highlevel/boxplot.rs` ports `fivenum()`
  directly.
- **Margins in "line" units.** R measures `mar`/`oma`/`mgp` in units of
  "one line of text", derived here from first principles as `mex *
  par(cex) * 1.2 * startps / 72` inches (matching `cra[1] = 1.2 *
  pointsize`, `ipr[1] = 1/72` for the PDF/PostScript device family),
  where `startps` is the device's starting point size
  (`DeviceBuilder::pointsize`; 12 for `Device::new`). A later
  `par(ps)` doesn't change it, and it doesn't scale with the *current*
  `cex` of text drawn in the margins, only with `mex * par(cex)` (R's
  `cexbase`), which takes effect at the next `plot.new()`. This is
  cross-checked exactly against R in `tests/all/device_layout_matches_r.rs`,
  `tests/all/layout_matches_r.rs` and `tests/all/start_ps_matches_r.rs`.
- **Engine bugs are deliberately *not* replicated.** Per an explicit
  scoping decision, this crate does not chase R's own internal quirks
  when they'd add complexity without improving visual fidelity - e.g.
  a known off-by-one in `devPS.c`'s PostScript kerning-pair lookup
  table is intentionally not replicated in `src/afm.rs` (documented
  there, with a tolerance-based test instead of an exact-match one).
  Likewise `barplot()`'s legend on a log value axis goes in the top-right
  corner as intended, not where R's use of log10 `par("usr")` puts it,
  and its default limits for stacked bars use each column's `offset`
  (R recycles `offset` over the whole matrix of stacked sums there).
- **Pages.** PDF output has one page per R page, as R's `pdf()` does: a
  new page starts at each `plot_new()` on a one-figure page, when a
  multi-figure page wraps round, and after `par(fig = )` without
  `new = TRUE`. SVG and PNG write one file per page when the file name
  has a C integer format for the page number, as R's
  `png("Rplot%03d.png")` (`FileName` in `src/device/open.rs`, R's
  `checkIntFormat()` rules); otherwise they hold the last page. A PDF
  keeps every page in one file named for page 1 (R's `onefile = TRUE`),
  or with `DeviceBuilder::onefile(false)` writes one file per page as PNG
  and SVG do.
- **Page background.** As in R, `par("bg")` starts as the device's
  background: transparent for PDF (R's `pdf()`), white for SVG and
  PNG. Set one with `DeviceBuilder::bg`
  (`Device::pdf(file).bg("white")`, R's `pdf(bg = )`; the same on
  `Device::memory(..)`). New pages are filled with it, and `legend()`
  boxes by default.
- **Latin-1 text.** Strings may contain Latin-1 characters (`±`, `é`,
  `×`, ...): string widths use each glyph's AFM metrics looked up by its
  ISOLatin1 glyph name (as R's `pdf()` does via `ISOLatin1.enc`, not by the
  AFM's Standard Encoding codes), and the PDF backend writes them as
  WinAnsi bytes. Characters outside Latin-1 are drawn as `?` in PDF
  output (SVG/PNG draw them as-is). Kerning is applied to string widths
  and, as R's `pdf()` does, inside drawn PDF strings (a string with
  kerning pairs is a `TJ` array, `[(A) 70 (V)] TJ`; PNG places each
  glyph at its kerned position; SVG leaves it to the viewer's font). R
  never applies the last kerning pair of each first character (a bug in
  `devPS.c`'s lookup); we apply every pair, so such strings (`F r` in
  "Frequency") differ from R's by a fraction of a point.
- **Joint tick-label formatting.** Default axis labels are formatted as
  a vector, like `formatReal()` (`0.0 0.5 1.0`; scientific only when
  strictly narrower).
- **Options not in R (`src/extra.rs`).** A few setters do in one step
  what takes several calls in R (`label_srt` for rotated tick labels,
  `panel_bg` for a plot-region background). They are kept apart from the
  R ports so that every R-named setter still does exactly what R does.
  - **Placement.** An extra is a setter on the R call where users would
    look for it (`dev.axis().side(1).label_srt(45)`), not a separate namespace.
    All its code is in `src/extra.rs`. The `extras!` macro there
    declares the values and generates the setters on the options
    structs and builders, in impl blocks documented "**Not in R.**".
  - **Storage and behaviour.** The values live in a hidden `extra`
    field of the call's options struct, so `.with(&opts)` and `Figure`
    replay carry them with no further code. The R function's `*_impl`
    makes one call into `src/extra.rs` at the point where R users would
    write their workaround (for `panel_bg`, where R evaluates
    `panel.first`). Unset, that call changes nothing; the gallery must
    stay byte-identical.
  - **Equal to R code.** Each extra's doc names the R code it replaces,
    and `tests/all/extra_matches_r.rs` runs that code in R and compares
    the output path by path. Prefer an exact R equivalent: rotated
    labels are placed as R's `las = 2` labels are, so `label_srt(90)` on
    side 1 is `las = 2`.
  - **Documentation.** `tests/all/generated_docs.rs` reads the `extras!`
    blocks and lists the setters as *not in R* rows in the R reference.
    Add the extra to the table in `src/extra.rs`'s module doc and to
    "Beyond R" in `src/guide/coming_from_r.md`.
  - **What fits here.** An extra replaces an idiom that R users write
    by hand: value labels on bars, a legend outside the plot, margins
    fitted to the labels. R functions not yet ported, such as `grid()`
    and `rug()`, belong in the R-faithful API, not here.

## Example gallery

`examples/gallery.rs` renders `plot()`/`hist()`/`barplot()`/
`boxplot()`/`pie()` (and a `layout()` multi-panel figure) to SVG+PDF+PNG:

```sh
cargo run --example gallery
# -> gallery_output/{plot_scatter,plot_line,plot_log,plot_h_s,abline_log,legend,hist,barplot,barplot_stacked,barplot_beside,boxplot,boxplot_horizontal,pie,multipanel,pch}.{svg,pdf,png}
#    and gallery_output/multipage.pdf (3 pages)
```

`examples/gen_reference.R` produces the real-R equivalent of the same
plots. Both scripts read the same sample data from `examples/data/*.txt`
(committed; generated once by `examples/gen_data.R` with `set.seed(1)`),
so the two galleries are drawn from identical numbers and can be compared
directly - bar/box/point positions should agree, not just the visual
style:

```sh
Rscript examples/gen_reference.R
# -> reference_output/{...}.{pdf,png} (and .svg, if your R has cairo support)
```

R's `svg()` needs cairo, which on macOS needs XQuartz; without it the
script says so and skips the SVGs.

To use different sample data, rerun `Rscript examples/gen_data.R` and then
both of the commands above.

## Testing

```sh
cargo test                                  # everything
cargo test --test all legend_matches_r      # one file's tests
```

The integration tests are modules of one test binary, `tests/all/`
(`main.rs` lists them), so cargo builds and links a single binary and
runs every test in parallel threads. Add a new file there and a `mod`
line to `tests/all/main.rs`. Tests that run R name their temporary files
`rs_graphics_<module>_...` so parallel tests don't clash.

The built-in backends' constructors are crate-private (users open
devices through `DeviceBuilder`), so the tests that need a backend built
by hand are unit tests in the crate (`cargo test --lib`):
`src/backend/smoke_tests.rs` (each backend's output well formed;
compressed PDF, `.svgz` and the PNG `pHYs` chunk; PNG output independent
of the thread count; `AnyBackend` gives the wrapped backend's bytes) and
`src/device/open_tests.rs` (every `DeviceBuilder` option byte-identical
to the backend set up by hand, R's defaults, the format by name only,
`open()` creating the file and `close()` writing it, `Device::memory`
giving the file's bytes).

Most tests are self-contained. Several integration tests
(`tests/all/*_matches_r.rs`) additionally cross-check numeric output
against a real R installation by shelling out to `Rscript`:

- `tests/all/pretty_matches_r.rs` - `GEPretty()` vs. `par("xaxp")`.
- `tests/all/afm_matches_r.rs` - `strwidth()` metrics (1% tolerance, see
  the kerning-bug note above).
- `tests/all/device_layout_matches_r.rs` - plot region size (`par("pin")`),
  margins (`par("mai")`), and the padded user coordinate system
  (`par("usr")`) after `plot.window()`, to 1e-6 tolerance.
- `tests/all/hist_matches_r.rs` - bin edges/counts vs. `hist(x, plot =
  FALSE)`: Sturges, Scott and FD rules, a number of classes, explicit and
  unsorted edges, a breaks function, `right`/`include.lowest`/`fuzz`,
  single and equal values, and R's errors for invalid breaks.
- `tests/all/boxplot_matches_r.rs` - box/whisker/outlier stats vs.
  `boxplot.stats()`.
- `tests/all/boxplot_draw_matches_r.rs` - every painted path of `boxplot()`
  in drawing order (vertices, fill/stroke, colours, width, cap, dashes),
  incl. `notch`, `varwidth`, `horizontal` and all three together.
- `tests/all/boxplot_extras_matches_r.rs` - `boxplot()`'s `at`/`width`,
  `outline = FALSE`, drawing parameters, outlier lines, `add = TRUE`, log
  value and position axes, and `xlim`/`ylim` with `horizontal`: every
  painted path and every text item vs. R `pdf()`.
- `tests/all/barplot_matches_r.rs` - bar rectangles and text origins vs. an
  uncompressed R `pdf()` for vector, stacked, `beside`, and `horiz`
  scenarios.
- `tests/all/barplot_extras_matches_r.rs` - `barplot()`'s `offset` (vector,
  `beside`, stacked), `orderH`, `add = TRUE` (with `ann` off by default)
  and `args.legend` (keyword position, `bty`, `inset`, `ncol`, `title`,
  `xjust`): every painted path and every text item vs. R `pdf()`.
- `tests/all/layout_matches_r.rs` - bar rectangles and text origins for
  `mfrow`/`mfcol`/`mfg`/`oma`/`par(cex)` and `layout()` (spanning,
  `lcm()`, `respect`) vs. an uncompressed R `pdf()`.
- `tests/all/pie_matches_r.rs` - every slice/tick-line vertex and text origin
  of `pie()` (square, wide and tall pages, `clockwise`, `init.angle`,
  `radius`, `par(xaxs = "i")`) vs. an uncompressed R `pdf()`.
- `tests/all/text_matches_r.rs` - `text()` origins for single/two-element
  `adj`, rotation, `cex` and `font` vs. an uncompressed R `pdf()`.
- `tests/all/multiline_text_matches_r.rs` - text with newlines drawn
  line by line and measured line by line, as R's `GEText()`/
  `strwidth()`: `text()` (`adj`, `srt`, `cex`, `par(lheight)`),
  `mtext()`, `title()`, `legend()` labels and title, `axis()` labels
  (incl. `gap.axis`) and `barplot()` names: every path and text item.
- `tests/all/strwidth_matches_r.rs` - `Device::strwidth`/`strheight` vs.
  R's `strwidth()`/`strheight()` in inches, user (incl. a reversed axis)
  and figure units: `cex`, `font`, multi-line text, `par(ps)`,
  `par(lheight)` and a 2x2 `mfrow`.
- `tests/all/colour_zero_matches_r.rs` - colour 0 is `par("bg")` (with
  `pdf(bg = )`, the transparent default, and after `par(bg = )`): every
  painting operator and its colours.
- `tests/all/plotmath_axis_matches_r.rs` - expression labels on all four
  axis sides (rotation, `cex.axis`, `hadj`/`padj`, `las`), and in
  `title()`, `mtext()` (inner, outer, perpendicular), `text()` (`adj`,
  `srt`, `cex`, `col`) and `legend()` (labels incl. a tall one, title):
  every text item's font, size, origin and string, and every stroke.
- `tests/all/plotmath_extras_matches_r.rs` - fractions, radicals,
  accents, `abs`, `underline`, `bgroup`, big operators and `{}` in
  `text()`, in display and script styles, rotated and scaled: every text
  item and every stroke (fraction bars, radicals, accents).
- `tests/all/hatch_matches_r.rs` - shading lines (`density`/`angle`) in
  `polygon()`, `rect()`, `barplot()` (beside, stacked, `orderH`,
  horizontal, legend) and `legend()`: both hatching branches, winding
  (and `fillOddEven`), `NA`/0/negative densities, reversed and log axes;
  every painted path and text item.
- `tests/all/start_ps_matches_r.rs` - margins, symbols, `type = "b"` gaps and
  `legend()` spacing come from the device's starting point size, not a
  later `par(ps)`, as R's `cra`.
- `tests/all/pch_matches_r.rs` - every `pch = 0:25` plus character symbols,
  for several `col`/`bg`/`cex`/`lwd`/`par(cex)` combinations: each painted
  path's bounding box, fill/stroke operator, colours, line width and cap,
  and each character's text origin/size, vs. an uncompressed R `pdf()`.
- `tests/all/mtext_matches_r.rs` - `mtext()` on every side, inner and outer
  margins, with `line`/`at`/`adj`/`padj`/`cex`/`font` and `las = 0..3`, in
  a 2x2 `mfrow` page with `oma`: text origins, sizes and rotations.
- `tests/all/alpha_matches_r.rs` - semi-transparent colours on PDF:
  fills, borders, text, symbols and lines with alpha, the same alpha
  reused, opaque again after it, across a clip and a new page: every
  path and text item with its colour and alpha (read through `ExtGState`
  resources and `gs`).
- `tests/all/arrows_matches_r.rs` - every shaft and head vertex for heads at
  either/both ends, `angle = 90` caps, an `NA` segment and a zero-length
  arrow.
- `tests/all/axis_zero_tick_matches_r.rs` - default tick labels for axes that
  straddle zero (`CreateAtVector()`'s snapping of near-zero ticks to 0).
- `tests/all/axis_args_matches_r.rs` - `axis()` with `line`, `pos`, `outer`,
  `tck`/`tcl`, `hadj`/`padj`, `gap.axis`, line styles and colours, `NA`
  labels, and a reversed axis under `par(xpd = TRUE)`: every axis line and tick vertex,
  colour, width and dash, and every label origin, direction and colour.
- `tests/all/par_new_matches_r.rs` - `par(new = TRUE)`: a second y axis,
  overlays in a 2x2 page, `plot.new()` not resetting it, drawing and outer
  `mtext()` (not) resetting it, and setting it before plotting; every text
  origin and data-line vertex.
- `tests/all/log_axis_matches_r.rs` - `plot(..., log = )` over ranges that hit
  each of R's log tick rules (1-2-5, 1-5, decimated powers of ten, a
  linear small range, a reversed axis with `at` in data units), with
  non-positive values dropped: label strings and origins, axis and tick
  segments, and the number of points drawn.
- `tests/all/plot_new_matches_r.rs` - the 0..1 user coordinates `plot.new()`
  leaves without `plot.window()` (fresh, after a log plot, with `xaxs =
  "i"`): lines, points and labels.
- `tests/all/plot_type_b_matches_r.rs` - `plot(type = "b")`'s gapped line
  segments (short ones dropped, broken at `NA`), with `par(cex)` and a log
  axis, and `type = "c"` (the same lines, no points).
- `tests/all/plot_type_h_s_matches_r.rs` - `type = "h"` (lines from y = 0, or
  the bottom of the plot on a log y axis; colours recycled, `NA` colours
  and values skipped) and `"s"`/`"S"` steps (broken at `NA`, log axes,
  `lty`/`lwd`) in `plot()`, `lines()` and `points()`: every stroked
  path's vertices, colour, width and dash.
- `tests/all/log_plots_matches_r.rs` - `segments()` with ends <= 0 or `NA` on
  log axes, `abline()` on log-log and log-x plots
  (`h`/`v`, `a`/`b`, `untf`, `xpd = TRUE`), `hist()` with `log = "x"`/`"y"`,
  `barplot()` on log axes (vector, stacked with legend, horizontal beside
  with `xlim`), and `par(lab)` on linear and log axes: every painted path
  and text item in order.
- `tests/all/abline_matches_r.rs` - `abline()` with vectors of `h`/`v`,
  `a`/`b` (several lines, drawn as successive R calls; a fitted `lm()`
  line) and recycled `col`/`lty`/`lwd` (incl. `par()` defaults and log
  axes): every line's vertices, colour, width and dash pattern.
- `tests/all/par_page_layout.rs` (no R needed) - a device's `par()` page
  layout arguments: one call gives the same output as separate calls, the
  last grid wins, a `cex`/`mex` in the same call wins, `fig`/`fin`/`mfg`
  are applied before `new`, and `.with()` keeps the grid.
- `tests/all/page_background.rs` (no R needed) - the device-dependent page
  background and new pages discarding the old one.
- `tests/all/figure_legends_matches_r.rs` - recorded `Figure`s with legends
  drawn into multi-panel pages (`mfrow` with `add_figure_at`, a shared-key
  panel in a `layout()`, one figure added three times to a 3x2 page) vs.
  the same R code: every text item, every legend box, and each
  `FigureResult::legends` entry vs. R's `legend()` return value.
- `tests/all/figure_replay.rs` (no R needed) - recorded `Figure`s drawn with
  `add_figure`/`add_figure_at` give byte-identical output to direct calls
  (every kind of call, all backends, mixed in multi-panel pages), `par`
  is restored, and recorded results match the device's.
- `tests/all/tck_srt_matches_r.rs` - `par(tck)` (incl. `1` for a grid, an
  inline `axis(tck =)` over it, `NA` resetting it) and `par(srt)` on
  `text()`, `pie()` labels and `legend()` labels/title (R draws them
  with `text()`): paths and text (incl. rotation) vs. R `pdf()`.
- `tests/all/bty_title_pars_matches_r.rs` - every `bty` (through `plot()`,
  `par()`, `box(which = )` and `boxplot()`'s frame) and the inline title
  parameters of `plot()`, `title()`, `hist()`, `barplot()`, `boxplot()`
  and `pie()`: every painted path and every text item's string, origin,
  font, size and colour vs. R `pdf()`.
- `tests/all/integer_args.rs` (no R needed) - integers wherever R takes
  numbers (data vectors, ranges, recycled and single setters, limits,
  arrays, pairs, `layout()` sizes, `fig()`, device sizes and `res`) draw byte
  for byte what the same `f64`s do, on a device and in a `Figure`.
- `tests/all/frozen_margins.rs` (no R needed) - `mar`/`oma` changed after
  `plot_new()` move nothing until the next plot (R would move margin
  text but not the data).
- `tests/all/axp_matches_r.rs` - `par("xaxp")`/`par("yaxp")` after
  `plot.window()` (linear, log with each tick rule, reversed, `par(lab)`),
  and the ticks drawn with `xaxp`/`yaxp` changed afterwards, inline in
  `axis()` and in `plot()`.
- `tests/all/xpd_na_matches_r.rs` - `xpd = NA` on a two-panel page:
  `par(xpd = NA)` for lines/points/text, `abline()` and `axis()` limits,
  inline `xpd = NA` on `text()`/`legend()`, `TRUE` next to `NA`: paths
  and text with the clip rectangle in effect for each, vs. R `pdf()`.
- `tests/all/margins_regions_matches_r.rs` - `mai` (also with `cex`, and
  `mar` resetting it), `omi`/`omd` with outer `mtext()` (and `mfrow`),
  `pin` (and `mar` resetting it), `pty` resetting `plt`, `fin`, and
  `par(oma)` starting a new page: paths and text vs. R `pdf()`.
- `tests/all/line_pars_matches_r.rs` - inline `lend`/`ljoin`/`lmitre` on
  every primitive (incl. a `pch` symbol, which uses `par()`'s with a
  solid line): cap, join, mitre and width at each stroke vs R `pdf()`,
  and that they don't outlast the call.
- `tests/all/xy_lengths_matches_r.rs` - `plot()`/`points()`/`lines()`/
  `polygon()` with `x` and `y` of different lengths return R's
  `xy.coords()` error from `try_draw()` (on a device and in a `Figure`); `text()`
  recycles, as R.
- `tests/all/region_errors_matches_r.rs` - `try_plot_new()`'s errors with R's
  messages ("figure margins too large", "figure region too large", "plot
  region too large") for small pages, large `oma`/`mai`, many panels,
  `fin`, `pin`, `plt` and `lcm()` layouts, cases that just fit, and the
  figure counter after an error (retrying the same figure, as R).
- `tests/all/regions_matches_r.rs` - `par(pty = "s")` on wide and tall
  pages, `par(plt=)` (also over `pty`), and `par(fig=)` with `new = TRUE`,
  `oma` and after `mfrow`: every painted path and text item in order.
- `tests/all/multipage_matches_r.rs` - page count and each page's paths
  and text for separate plots, a wrapping `mfrow` and `fig` with and
  without `new = TRUE`, plus compressed pages inflating to the
  uncompressed ones.
- `tests/all/latin1_text_matches_r.rs` - bytes written and text origins for
  strings with Latin-1 characters (`±`, accents, `ß`, `©`, fractions).
- `tests/all/axis_matches_r.rs` - tick-label, `xlab`/`ylab` and `main` text
  origins for `las = 0..3`.
- `tests/all/legend_matches_r.rs` - legend box and label origins for keyword
  positions, `inset`, `ncol`/`horiz`, titles, filled boxes,
  `cex`/`intersp` and `par(cex)`, and `legend()`'s return value (also on
  log axes).
- `tests/all/legend_extras_matches_r.rs` - `legend()` on reversed axes
  (keyword, `(x, y)`, `ncol`, wide title, `text.width`), `box.lwd`/
  `box.lty`/`box.col` (also from `par()`, and `NA`), `title.col`/
  `title.adj`/`title.font`, per-entry `text.col`/`text.font`, `plot =
  FALSE`, keys mixing boxes, lines and points (`NA`s, per-key `border`),
  lines from `lwd` alone with `merge = FALSE`, character `pch` (incl. a
  non-ASCII one and R's split of `pch = "xyz"`), per-key `cex`/`pt.cex`/
  `x.intersp`/`seg.len` with per-row `y.intersp`, `text.width = NA` and
  per column, and `adj`: every painted path and text item, and the
  return value.
- `tests/all/inline_args_matches_r.rs` - `box(fg = , col = NA)`, inline
  `xpd` on `title()` (and `mtext()`'s, on its own: R always clips
  `mtext()` to the device, a bug not copied), inline `xpd`/`lend` on
  `axis()`, `lwd` on `hist()`/`barplot()` axes with shaded bars,
  `pie()`'s `border`/`lty`/`density`/`angle`, character-`pch` `font`, and
  `plot()`'s inline `fg`/`adj`/`lend`/`xpd`/`font`, `par(usr = )` (also
  on a log axis, whose tick settings R's `GSetupAxis()` gets wrong, a bug
  not copied), `par(xlog = )`, `par(ylbias = )`, and `NA` labels in
  `text()`, `mtext()` and `legend()`: every painted path and text item
  with its clip rectangle.
- `tests/all/kerning_matches_r.rs` - kerning inside PDF strings: `TJ`
  arrays (pieces and kerns) vs R's for strings in all four Helvetica faces,
  Latin-1, escaped characters, scaled and rotated text; R's skipped last
  pair of a character (`F r`) checked on our side only.
- `tests/all/page_files.rs` - one file per page: `%03d` names for PNG and
  SVG (each page's file as that page drawn alone), page breaks with
  `mfrow`, the last page without a number (also on a transparent
  background), PDF `onefile` (file names and page counts vs R), refused
  `%` conversions, and page numbers vs R's `sprintf()`.
- `tests/all/na_values_matches_r.rs` - `None` (R's `NA`) in the recycled
  style vectors of points, lines, segments, arrows, rect, polygon, text,
  mtext, abline, boxplot and legend: where R defines `NA`, R's drawing;
  otherwise R drawing the skipped element or the default.
- `tests/all/errors_matches_r.rs` - R's errors as values: `try_draw()`'s
  messages vs R's `conditionMessage()` (device and `Figure`), errors found
  while drawing, setter values that don't convert (kept for `try_draw()`,
  also through `.with()`), `?`, plotmath delimiters, log `axp`, `open()`.
- `tests/all/generated_docs.rs` (no R needed) - the generated files are up
  to date: the R-to-Rust table `src/guide/r_reference.md` (from the
  `call!`/`options!` declarations and `tests/data/r_formals.txt`, R's
  arguments; refresh that with `Rscript tests/data/r_formals.R >
  tests/data/r_formals.txt`), the chart SVGs `src/charts/*.svg` and
  `llms.txt`, and `tests/all/readme_examples.rs` (README.md's code blocks,
  compiled with the tests); `par(x)` mentions in docs are links.
  `RS_GRAPHICS_BLESS=1 cargo test --test all generated_docs` rewrites
  them (if a README fix leaves `readme_examples.rs` not compiling, empty
  it first).
- `tests/all/required_args.rs` (no R needed) - every required argument left
  out is an error with R's message (`argument "x" is missing, with no
  default`) and the call's name, on a device and in a `Figure`; `.draw()`
  panics with it; arguments with R defaults can be left out.
- `tests/all/primitives_matches_r.rs` - per-element recycling in `points()`,
  `segments()`, `arrows()`, `rect()`, `polygon()` and `text()`,
  `lines(type = )`, `par("fg")` defaults, polygons split at `NA`, default
  `text()` labels, `box(which = )`, `plot.window(asp = )` and `plot()`'s
  axes following `par(col.axis)` etc.: every painted path and text item in
  order.
- `tests/all/pch_strings_matches_r.rs` - `pch` given as strings in
  `points()`, `plot()` and `boxplot()` (first character, `""`/`NA` none,
  non-ASCII as text).
- `tests/all/label_na_matches_r.rs` - `NA` (`None`) in `barplot()` and
  `boxplot()` names, `barplot()`'s legend text and `pie()` labels.
- `tests/all/extra_matches_r.rs` - the options not in R (`label_srt`,
  `panel_bg`) against the R code each replaces, in R; also in a `Figure`
  and through `.with()`.
- `tests/all/figure_pages.rs` (no R needed) - a `Figure` holding a whole
  plot (page layout, `layout()`, several pages) draws what the same calls
  draw on a device, on every format; record-time errors.
- `tests/all/null_device.rs` (no R needed) - `Device::null()` gives the
  same values and errors as a PDF device of the same size, and writes
  nothing.

These are **skipped (not failed)** if `Rscript` isn't found on `PATH`,
so the suite stays green for contributors without R installed - but
for any change touching layout, tick placement, or text metrics, please
run with R available if you can. Setting `RS_GRAPHICS_REQUIRE_R=1` makes
them fail instead of skipping (`tests/all/common.rs`).

Most of them share their machinery: `tests/all/common/r.rs` runs the R
cases in one `Rscript` and draws ours (`run_r`, `ours`), and
`tests/all/common/pdf.rs` reads both uncompressed PDFs (`parse`: every
painted path and text item per page, with colours, alpha, line style,
clip and the text matrix) and compares them in order (`assert_same_pdf`,
with per-file tolerances and exclusions in `Compare`).

Scripts in `tools/` (Python 3; `-h` for usage) help check a change:
`verify.py` regenerates the generated docs (listing what changed), then
runs the tests with R, rustdoc, clippy, the Rust 1.85 build and the
gallery against a base revision; `fmt_own.py` runs rustfmt on
changed lines only; `mutate.py` checks that a test fails when the code
it covers is broken.

### Continuous integration

`.github/workflows/rust.yml` runs on pushes and pull requests to `main`
(and by hand, from the Actions tab), on Ubuntu:

- **test** - installs the current R release (`r-lib/actions/setup-r`)
  and runs `cargo test` with `RS_GRAPHICS_REQUIRE_R=1`, so every R
  cross-check runs;
- **msrv** - builds everything with Rust 1.85, `Cargo.toml`'s
  `rust-version`;
- **docs** - `cargo doc` with rustdoc warnings (e.g. broken intra-doc
  links) as errors.

All three use `--locked`, so commit `Cargo.lock` changes. Clippy and
rustfmt aren't checked: the code base isn't clean under either yet.

## Licence notices

NOTICE is generated by
[cargo-about](https://github.com/EmbarkStudios/cargo-about) from
`notice.hbs` and `about.toml`; regenerate it whenever the dependencies
change:

```
cargo about generate notice.hbs -o NOTICE
```
