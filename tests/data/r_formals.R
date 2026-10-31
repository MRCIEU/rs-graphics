# Copyright (C) 2026 University of Bristol
# rs-graphics includes code translated from R, copyright (C) The R Core Team
# and others; see NOTICE.
#
# This file is part of rs-graphics, licensed under the GNU General Public
# License, version 3 or (at your option) any later version; see LICENSE.
# SPDX-License-Identifier: GPL-3.0-or-later

# The arguments of each R function rs-graphics ports, and the par() names,
# for the generated R-to-Rust table (tests/all/generated_docs.rs). Run:
#   Rscript tests/data/r_formals.R > tests/data/r_formals.txt

f <- list(plot = "plot.default", hist = c("hist.default", "plot.histogram"),
  barplot = "barplot.default", boxplot = c("boxplot.default", "bxp"), pie = "pie",
  plot.window = "plot.window", points = "points.default", lines = "lines.default",
  segments = "segments", arrows = "arrows", rect = "rect", polygon = "polygon",
  text = "text.default", box = "box", axis = "axis", abline = "abline",
  legend = "legend", mtext = "mtext", title = "title", plot.new = "plot.new",
  layout = "layout", strwidth = "strwidth", strheight = "strheight",
  pdf = "pdf", svg = "svg", png = "png")
ns <- function(g) if (g %in% c("pdf", "svg", "png")) "grDevices" else "graphics"
for (k in names(f)) {
  a <- unique(unlist(lapply(f[[k]], function(g) names(formals(get(g, envir = asNamespace(ns(g))))))))
  cat(k, ": ", paste(a, collapse = " "), "\n", sep = "")
}
pdf(NULL) # par() opens a device; this one writes no file
cat("par: ", paste(sort(names(par())), collapse = " "), "\n", sep = "")
cat("# ", R.version.string, "\n", sep = "")
