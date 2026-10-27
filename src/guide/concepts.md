The words the docs use come from R. In short:

# The page

| Term | Meaning |
|---|---|
| device | Where drawing goes: a PDF, SVG or PNG file being written ([`Device`](crate::device::Device)). It has one or more pages. |
| plot region | The rectangle the data is drawn in. Axes run along its edges; drawing is clipped to it by default. |
| margins | The space around the plot region, for the axes, their labels and the titles. Measured in **lines** of text: [`par().mar([5.1, 4.1, 4.1, 2.1])`](crate::par::Par::mar) (bottom, left, top, right), or in inches with [`mai`](crate::par::Par::mai). |
| figure region | The plot region plus its margins: one panel. |
| outer margins | Space around all the panels of a page, e.g. for a shared title: [`oma`](crate::par::Par::oma) (lines), [`omi`](crate::par::Par::omi) (inches). Text goes there with `mtext().outer(true)`. |
| inner region | The page less the outer margins, which the panels share. |
| panels | Several figures on a page: [`par().mfrow(rows, cols)`](crate::graphics::ParCall::mfrow) fills them row by row, `mfcol` column by column, [`layout()`](crate::graphics::Graphics::layout) in any grid. Each high-level plot moves to the next panel. |
| user coordinates | The data's units inside the plot region, set by the plot's limits (`xlim`, `ylim`), read as [`par("usr")`] ([`Device::usr`](crate::device::Device::usr)). Most drawing calls take them. |
| clipping | What is cut off at a region's edge: [`xpd`](crate::par::Par::xpd) ([`Xpd`](crate::par::Xpd)): the plot region (the default), the figure region, or the whole device. |

# Calls

| Term | Meaning |
|---|---|
| high-level plot | A call that starts a new plot (next panel), sets up its coordinates and draws a whole chart: `plot()`, `hist()`, `barplot()`, `boxplot()`, `pie()`. |
| low-level call | A call that adds to the current plot: `points()`, `lines()`, `text()`, `abline()`, `legend()`, `axis()`, `title()`, `mtext()`, ... |
| graphical parameters | Settings that apply to everything drawn after them, set with [`par()`](crate::graphics::Graphics::par) and held in [`Par`](crate::par::Par). Most can also be given to one call, for that call only (`plot().cex_axis(0.8)`). |
| recycling | A vector argument shorter than the data is repeated: `.col(["red", "blue"])` on five points colours them red, blue, red, blue, red. |
| `NA` | R's missing value: `None` here. In a style vector it skips that element (`.pch([Some(19), None])`); a missing data value leaves a gap. |
| plotmath | R's notation for mathematical labels: `Label::math("beta[1]^2")` (see [`plotmath`](crate::plotmath)). |

# Sizes and styles

| R's name | Setter | What it is |
|---|---|---|
| `cex` | `.cex()` | Size relative to the default (1 is normal, 2 double), for symbols and text. `cex.axis`, `cex.lab`, `cex.main`, `cex.sub`: of the axis labels, axis titles, main title and subtitle. |
| `ps` | `.ps()` | Text size in points (default 12); `cex` multiplies it. |
| `pch` | `.pch()` | Plotting symbol: a number 0 to 25 or a character ([chart](crate::charts::pch)). |
| `lty` | `.lty()` | Line type: solid, dashed, ... ([chart](crate::charts::line_types)). |
| `lwd` | `.lwd()` | Line width, 1 being the default (3/4 of a point). |
| `lend`, `ljoin` | `.lend()`, `.ljoin()` | How line ends and corners are drawn ([chart](crate::charts::line_ends)). |
| `col` | `.col()` | Colour: a name ([chart](crate::charts::colors)), `"#RRGGBB"` or `"#RRGGBBAA"`, or a number into the palette ([chart](crate::charts::palette)). |
| `bg`, `fg` | `.bg()`, `.fg()` | Background (also the fill of symbols 21 to 25) and foreground (axes, boxes) colours. |
| `font` | `.font()` | Font face: plain, bold, italic, bold italic or symbol ([chart](crate::charts::fonts)). |
| `adj` | `.adj()` | Justification of text: 0 left, 0.5 centred, 1 right. |
| `las` | `.las()` | Orientation of axis labels: 0 parallel to the axis, 1 horizontal, 2 perpendicular, 3 vertical. |
| `srt` | `.srt()` | Rotation of text in degrees. |
| `mgp` | `.mgp()` | Margin lines of the axis title, tick labels and axis line (default 3, 1, 0). |
| `tcl`, `tck` | `.tcl()`, `.tck()` | Tick length, in lines or as a fraction of the plot region. |
| `xaxs`, `yaxs` | `.xaxs()`, `.yaxs()` | `"r"` (the default) adds 4% to each end of the axis range, `"i"` nothing. |
| `bty` | `.bty()` | Which sides of the box around the plot to draw (`"o"` all, `"l"` left and bottom, `"n"` none, ...). |
| `ann` | `.ann()` | Whether high-level plots draw their titles. |
| `type` | `.type_()` | What `plot()`, `points()` and `lines()` draw: `"p"` points, `"l"` lines, `"b"` both, `"h"` vertical lines, `"s"` steps, `"n"` nothing. |

[`par("usr")`]: crate::device::Device::usr
