# Generates the real-R equivalent of each plot in `examples/gallery.rs`,
# for visual side-by-side comparison against `gallery_output/`.
#
# Run with: Rscript examples/gen_reference.R
# (from the crate root; writes into reference_output/)

set.seed(1) # note: R's RNG differs from the tiny PRNG gallery.rs uses,
            # so data won't match exactly - these are for *visual style*
            # comparison (margins, tick placement, fonts, colors), not
            # pixel-identical output.

dir.create("reference_output", showWarnings = FALSE)

save_all <- function(name, width, height, draw) {
  pdf(file.path("reference_output", paste0(name, ".pdf")), width = width, height = height)
  draw()
  dev.off()

  svg(file.path("reference_output", paste0(name, ".svg")), width = width, height = height)
  draw()
  dev.off()

  png(file.path("reference_output", paste0(name, ".png")), width = width, height = height,
      units = "in", res = 150)
  draw()
  dev.off()

  cat(sprintf("wrote reference_output/%s.{pdf,svg,png}\n", name))
}

save_all("plot_scatter", 6, 4, function() {
  x <- 1:25
  y <- sin(x * 0.35) * 8 + x * 0.6 + rnorm(25, sd = 1.5)
  plot(x, y, pch = 19, col = "#2297E6", main = "plot(): scatter", xlab = "x", ylab = "y")
})

save_all("plot_line", 6, 4, function() {
  x <- seq(0, 10, by = 0.25)
  y <- sin(x)
  plot(x, y, type = "l", col = "#DF536B", lwd = 2, main = "plot(): line", xlab = "x", ylab = "sin(x)")
})

save_all("hist", 6, 4, function() {
  data <- rnorm(300, mean = 10, sd = 2)
  hist(data, main = "hist(): approximately normal data")
})

save_all("barplot", 6, 4, function() {
  heights <- c(A = 23, B = 41, C = 12, D = 35, E = 28)
  barplot(heights, main = "barplot()", ylab = "value")
})

save_all("boxplot", 6, 4, function() {
  ctrl <- c(rnorm(40, 10, 2), 20)
  treatA <- rnorm(40, 14, 3)
  treatB <- c(rnorm(40, 9, 1.5), 2, 16)
  boxplot(ctrl, treatA, treatB, names = c("Ctrl", "Treat A", "Treat B"),
          main = "boxplot()", ylab = "value")
})

save_all("pie", 6, 6, function() {
  values <- c(18, 25, 12, 30, 15)
  labels <- c("Alpha", "Beta", "Gamma", "Delta", "Epsilon")
  pie(values, labels = labels, main = "pie()")
})

cat("\nReference gallery complete. See reference_output/ for comparison.\n")
