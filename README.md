# rs-graphics

A Rust port of the core functionality of R's `graphics` package (see
`r-source/src/library/graphics/` for the reference implementation this
crate is ported from), producing **SVG**, **PNG**, and **PDF** output.

The design priority throughout is **visual and numerical fidelity with
R's actual output** - not an idiomatic Rust API. Where possible, this
crate ports R's own algorithms line-by-line (tick placement, margin
sizing, symbol geometry, dash patterns, ...) rather than reimplementing
"a" reasonable version of them, and several of the trickiest numeric
pieces are cross-checked directly against a real R installation in this
crate's test suite (see [Testing](#testing) below).

## What's implemented

**Graphics engine** (`src/`):
- `par()`-equivalent state (`src/par.rs`): margins (`mar`/`oma`), axis
  styles, colors, line/point styles, fonts, `tcl`/`mgp`/`las`, etc.,
  with R's exact default values.
- Coordinate systems and clipping (`src/device.rs`): `plot.new()`/
  `plot.window()` equivalents, user/device coordinate transforms, log
  axes, and R's `xpd`-style clip regions (plot region vs. figure
  region).
- Drawing primitives: `points`, `lines`, `segments`, `rect`, `polygon`,
  `text`, `box`, `abline`, `axis`, `title`, `legend`, `arrows` - all
  implemented as `Device` methods (see `src/device.rs`). `legend()`
  (`Device::legend_with`, `src/device/legend.rs`) is a port of R's layout
  algorithm incl. keyword positions, `inset`, `ncol`/`horiz`, `title`,
  filled boxes and merged point+line keys.
- Multi-panel layouts (`src/layout.rs`, `Device::{mfrow, mfcol, layout,
  mfg}`): `par(mfrow=)`/`par(mfcol=)` (incl. R's automatic `cex` of
  0.83/0.66 for 2x2 / larger grids), `par(mfg=)`, `oma`, and
  `layout()` with spanning figures, relative / `lcm()` widths and
  heights, and `respect` (logical or matrix) - a port of `layoutRegions()`
  in `graphics.c`.
- R's built-in plotting characters `pch = 0..25` (`src/pch.rs`) plus
  character symbols (`pch = "."`, `"a"`, ...), line
  types `lty` (`src/lty.rs`), the R4 default color palette and all 657
  named colors (`src/color.rs`, `src/named_colors.rs`), and the exact
  `pretty()`/`GEPretty()` axis tick-placement algorithms - both of
  which R uses, with **different constants** (`src/pretty.rs`).
- Text layout (`src/afm.rs`): Helvetica AFM metrics (the same tables
  R's own `pdf()`/`postscript()` devices use) drive *all* string
  width/height/justification math, shared identically across all three
  backends - see [Design notes](#design-notes) below.

**High-level plots** (`src/highlevel/`), each a fairly direct port of
the corresponding `*.default` R function:
- `plot()` - scatter/line plots (`type = "p"/"l"/"b"/"o"/"n"`).
- `hist()` - histograms, including R's Sturges'-rule default binning.
- `barplot()` - vectors and matrices (stacked or `beside`), vertical or
  `horiz`, with `legend.text`, `space`, `width`, `xlim`/`ylim`.
- `boxplot()` - Tukey boxplots (via `fivenum()`, not `quantile()` -
  see [Design notes](#design-notes)), including the 1.5*IQR outlier rule.
- `pie()` - pie charts.

**Backends** (`src/backend/`):
- `svg.rs` - hand-rolled SVG writer (real `<text>` elements, not
  outlined paths).
- `pdf.rs` - hand-rolled single-page PDF writer, using the 4 standard
  Type1 Helvetica faces every PDF viewer bundles (no font embedding
  needed), matching R's own `pdf()` device's default font.
- `raster.rs` - PNG via [`tiny-skia`](https://docs.rs/tiny-skia) for
  shapes/paths/clipping and [`ab_glyph`](https://docs.rs/ab_glyph)
  (bundled Liberation Sans) for glyph rasterization.

## Out of scope

To keep this a tractable "core functionality" port, the following R
`graphics` features are **not implemented**: `split.screen()`/`par(fig=)`/
`par(new=)`/`mtext(outer = TRUE)`, `persp()`/3D plotting,
`mosaicplot()`, `stars()`, `coplot()`, `filled.contour()`,
`image()`/raster images, `symbols()`, `smoothScatter()`,
`cdplot()`/`spineplot()`, `dotchart()`, `fourfoldplot()`, and
interactive input (`identify()`/`locator()`). `barplot()` doesn't
support log axes, `density`/`angle` hatching, `offset`, or `add`, and
`boxplot()` doesn't support `notch`/`varwidth`. These would all be
reasonable follow-ups but are left for future work.

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
  "one line of text", derived here from first principles as `1.2 *
  par(ps) / 72` inches (matching `cra[1] = 1.2 * pointsize`, `ipr[1] =
  1/72` for the PDF/PostScript device family) - this does *not* scale
  with the *current* `cex` of text drawn in them, only with `ps` and
  `mex * par(cex)` (R's `cexbase`), which takes effect at the next
  `plot.new()`. This is cross-checked exactly against R in
  `tests/device_layout_matches_r.rs` and `tests/layout_matches_r.rs`.
- **Engine bugs are deliberately *not* replicated.** Per an explicit
  scoping decision, this crate does not chase R's own internal quirks
  when they'd add complexity without improving visual fidelity - e.g.
  a known off-by-one in `devPS.c`'s PostScript kerning-pair lookup
  table is intentionally not replicated in `src/afm.rs` (documented
  there, with a tolerance-based test instead of an exact-match one).
  Similarly, `GScale()`'s extensive edge-case handling for near-zero/
  infinite axis ranges isn't ported in full - only the common `xaxs =
  "r"`/`"i"` 4%-expansion behavior is.
- **One page per backend.** Backends hold a single page, so moving to a
  "new page" in a multi-panel sequence (the figure counter wrapping
  round) is emulated by repainting the background over earlier content,
  rather than starting a fresh page as R's multi-page devices do.
- **Joint tick-label formatting.** Default axis labels are formatted as
  a vector, like `formatReal()` (`0.0 0.5 1.0`; scientific only when
  strictly narrower).

## Example gallery

`examples/gallery.rs` renders `plot()`/`hist()`/`barplot()`/
`boxplot()`/`pie()` (and a `layout()` multi-panel figure) to SVG+PDF+PNG:

```sh
cargo run --example gallery
# -> gallery_output/{plot_scatter,plot_line,legend,hist,barplot,barplot_stacked,barplot_beside,boxplot,pie,multipanel}.{svg,pdf,png}
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

To use different sample data, rerun `Rscript examples/gen_data.R` and then
both of the commands above.

## Testing

```sh
cargo test
```

Most tests are self-contained. Several integration tests
(`tests/*_matches_r.rs`) additionally cross-check numeric output
against a real R installation by shelling out to `Rscript`:

- `tests/pretty_matches_r.rs` - `GEPretty()` vs. `par("xaxp")`.
- `tests/afm_matches_r.rs` - `strwidth()` metrics (1% tolerance, see
  the kerning-bug note above).
- `tests/device_layout_matches_r.rs` - plot region size (`par("pin")`),
  margins (`par("mai")`), and the padded user coordinate system
  (`par("usr")`) after `plot.window()`, to 1e-6 tolerance.
- `tests/hist_matches_r.rs` - bin edges/counts vs. `hist(x, plot =
  FALSE)`.
- `tests/boxplot_matches_r.rs` - box/whisker/outlier stats vs.
  `boxplot.stats()`.
- `tests/barplot_matches_r.rs` - bar rectangles and text origins vs. an
  uncompressed R `pdf()` for vector, stacked, `beside`, and `horiz`
  scenarios.
- `tests/layout_matches_r.rs` - bar rectangles and text origins for
  `mfrow`/`mfcol`/`mfg`/`oma`/`par(cex)` and `layout()` (spanning,
  `lcm()`, `respect`) vs. an uncompressed R `pdf()`.
- `tests/text_matches_r.rs` - `text()` origins for single/two-element
  `adj`, rotation, `cex` and `font` vs. an uncompressed R `pdf()`.
- `tests/pch_matches_r.rs` - every `pch = 0:25` plus character symbols,
  for several `col`/`bg`/`cex`/`lwd`/`par(cex)` combinations: each painted
  path's bounding box, fill/stroke operator, colours, line width and cap,
  and each character's text origin/size, vs. an uncompressed R `pdf()`.

These are **skipped (not failed)** if `Rscript` isn't found on `PATH`,
so the suite stays green for contributors without R installed - but
for any change touching layout, tick placement, or text metrics, please
run with R available if you can.

## License

GPL-2.0-or-later, matching R itself (this crate ports R's own
algorithms closely enough that it's derived from `r-source`, which is
GPL-2.0-or-later).
