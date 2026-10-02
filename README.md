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
  implemented as `Device` methods (see `src/device.rs`).
- R's built-in plotting characters `pch = 0..25` (`src/pch.rs`), line
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
- `barplot()` - single-series vertical bar charts.
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
  (bundled DejaVu Sans) for glyph rasterization.

## Out of scope

To keep this a tractable "core functionality" port, the following R
`graphics` features are **not implemented**: multi-panel layouts
(`par(mfrow=)`/`layout()`/`split.screen()`), `persp()`/3D plotting,
`mosaicplot()`, `stars()`, `coplot()`, `filled.contour()`,
`image()`/raster images, `symbols()`, `smoothScatter()`,
`cdplot()`/`spineplot()`, `dotchart()`, `fourfoldplot()`, and
interactive input (`identify()`/`locator()`). `barplot()` is also
restricted to a single (non-stacked, non-`beside`) series, and
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
- **DejaVu-vs-Helvetica tradeoff for PNG only.** The raster backend
  rasterizes glyphs with `ab_glyph`'s bundled DejaVu Sans (there's no
  portable way to rasterize the standard PDF Helvetica outlines without
  a real font file), but glyph *positions* still come from the
  Helvetica AFM metrics above, so multi-character strings land in the
  same place as the SVG/PDF output even though individual glyph shapes
  look slightly different. SVG and PDF output use real Helvetica.
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
  with `cex`, only `ps`, matching R. This is cross-checked exactly
  against R in `tests/device_layout_matches_r.rs`.
- **Engine bugs are deliberately *not* replicated.** Per an explicit
  scoping decision, this crate does not chase R's own internal quirks
  when they'd add complexity without improving visual fidelity - e.g.
  a known off-by-one in `devPS.c`'s PostScript kerning-pair lookup
  table is intentionally not replicated in `src/afm.rs` (documented
  there, with a tolerance-based test instead of an exact-match one).
  Similarly, `GScale()`'s extensive edge-case handling for near-zero/
  infinite axis ranges isn't ported in full - only the common `xaxs =
  "r"`/`"i"` 4%-expansion behavior is.
- **Single-panel only.** There's no `par(mfrow=)`/`layout()` support, so
  `par::Par`'s `cexbase`/`mex`/`scale` (R's multi-panel scaling
  factors) are always effectively `1.0`.

## Example gallery

`examples/gallery.rs` renders `plot()`/`hist()`/`barplot()`/
`boxplot()`/`pie()` to SVG+PDF+PNG:

```sh
cargo run --example gallery
# -> gallery_output/{plot_scatter,plot_line,hist,barplot,boxplot,pie}.{svg,pdf,png}
```

`examples/gen_reference.R` produces the real-R equivalent of the same
plots (using R's own RNG, so the sample data won't match exactly -
this is for comparing *visual style*: margins, tick placement, fonts,
colors, not pixel-identical output):

```sh
Rscript examples/gen_reference.R
# -> reference_output/{...}.{pdf,png} (and .svg, if your R has cairo support)
```

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

These are **skipped (not failed)** if `Rscript` isn't found on `PATH`,
so the suite stays green for contributors without R installed - but
for any change touching layout, tick placement, or text metrics, please
run with R available if you can.

## License

GPL-2.0-or-later, matching R itself (this crate ports R's own
algorithms closely enough that it's derived from `r-source`, which is
GPL-2.0-or-later).
