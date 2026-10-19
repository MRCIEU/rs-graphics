# Copyright (C) 2026 University of Bristol
# rs-graphics includes code translated from R, copyright (C) The R Core Team
# and others; see NOTICE.
#
# This file is part of rs-graphics, licensed under the GNU General Public
# License, version 3 or (at your option) any later version; see LICENSE.
# SPDX-License-Identifier: GPL-3.0-or-later

# Regenerates the sample data shared by `examples/gallery.rs` (Rust) and
# `examples/gen_reference.R` (real R), so both draw *identical* plots from
# identical numbers. The files are committed; you only need to rerun this
# if you want different sample data.
#
# Run with: Rscript examples/gen_data.R   (from the crate root)

set.seed(1)

dir.create("examples/data", showWarnings = FALSE)

# One value per line, full double precision (so Rust parses back the exact
# same numbers R used).
save_vec <- function(name, v) {
  writeLines(sprintf("%.17g", v), file.path("examples", "data", paste0(name, ".txt")))
}

x <- 1:25
save_vec("scatter_y", sin(x * 0.35) * 8 + x * 0.6 + rnorm(25, sd = 1.5))
save_vec("hist_x", rnorm(300, mean = 10, sd = 2))
save_vec("boxplot_ctrl", c(rnorm(40, 10, 2), 20))
save_vec("boxplot_treatA", rnorm(40, 14, 3))
save_vec("boxplot_treatB", c(rnorm(40, 9, 1.5), 2, 16))

cat("wrote examples/data/*.txt\n")
