# Generates the real-R equivalent of each plot in `examples/gallery.rs`,
# for visual side-by-side comparison against `gallery_output/`.
#
# Run with: Rscript examples/gen_reference.R
# (from the crate root; writes into reference_output/)
#
# Both this script and gallery.rs read the same sample data from
# examples/data/*.txt (generated once by examples/gen_data.R), so the two
# galleries are drawn from identical numbers and can be compared directly.

dir.create("reference_output", showWarnings = FALSE)

read_vec <- function(name) {
  scan(file.path("examples", "data", paste0(name, ".txt")), quiet = TRUE)
}

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
  y <- read_vec("scatter_y")
  plot(x, y, pch = 19, col = "#2297E6", main = "plot(): scatter", xlab = "x", ylab = "y")
})

save_all("plot_line", 6, 4, function() {
  x <- seq(0, 10, by = 0.25)
  y <- sin(x)
  plot(x, y, type = "l", col = "#DF536B", lwd = 2, main = "plot(): line", xlab = "x", ylab = "sin(x)")
})

save_all("legend", 6, 4, function() {
  x <- seq(0, 10, by = 0.25)
  plot(x, sin(x), type = "l", col = "#DF536B", lwd = 2, ylim = c(-1, 1.6),
       main = "legend(\"topright\")", xlab = "x", ylab = "value")
  lines(x, cos(x), col = "#2297E6", lwd = 2, lty = 2)
  legend("topright", legend = c("sin(x)", "cos(x)"), col = c("#DF536B", "#2297E6"),
         lwd = 2, lty = 1:2, title = "Functions")
})

save_all("hist", 6, 4, function() {
  x <- read_vec("hist_x") # named `x` so hist()'s default xlab is "x"
  hist(x, main = "hist(): approximately normal data")
})

save_all("barplot", 6, 4, function() {
  heights <- c(A = 23, B = 41, C = 12, D = 35, E = 28)
  barplot(heights, main = "barplot()", ylab = "value")
})

barplot_m <- matrix(c(3, 5, 2, 4, 1, 6, 2, 2, 7, 5, 3, 1), nrow = 3,
                    dimnames = list(c("x", "y", "z"), c("Q1", "Q2", "Q3", "Q4")))

save_all("barplot_stacked", 6, 4, function() {
  barplot(barplot_m, legend.text = TRUE, main = "barplot(): stacked")
})

save_all("barplot_beside", 6, 4, function() {
  barplot(barplot_m, beside = TRUE, legend.text = TRUE, main = "barplot(): beside")
})

save_all("boxplot", 6, 4, function() {
  ctrl <- read_vec("boxplot_ctrl")
  treatA <- read_vec("boxplot_treatA")
  treatB <- read_vec("boxplot_treatB")
  boxplot(ctrl, treatA, treatB, names = c("Ctrl", "Treat A", "Treat B"),
          main = "boxplot()", ylab = "value")
})

save_all("pie", 6, 6, function() {
  values <- c(18, 25, 12, 30, 15)
  labels <- c("Alpha", "Beta", "Gamma", "Delta", "Epsilon")
  pie(values, labels = labels, main = "pie()")
})

save_all("multipanel", 8, 6, function() {
  layout(matrix(c(1, 1, 2, 3), 2, byrow = TRUE))
  x <- 1:25
  y <- read_vec("scatter_y")
  plot(x, y, pch = 19, col = "#2297E6", main = "plot(): scatter", xlab = "x", ylab = "y")
  x <- seq(0, 10, by = 0.25)
  plot(x, sin(x), type = "l", col = "#DF536B", lwd = 2, main = "plot(): line", xlab = "x", ylab = "sin(x)")
  barplot(c(A = 23, B = 41, C = 12, D = 35, E = 28), main = "barplot()", ylab = "value")
})

cat("\nReference gallery complete. See reference_output/ for comparison.\n")
