// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! All integration tests, as modules of one test binary, so cargo builds
//! and links one binary and runs every test in parallel threads (separate
//! files in `tests/` each become their own binary, run one after another).
//! Run one file's tests with `cargo test --test all <module>`, e.g.
//! `cargo test --test all legend_matches_r`.
//!
//! Tests that run R write their temporary files under names starting
//! `rs_graphics_<module>`, so they don't clash when run in parallel.

mod common;

mod abline_matches_r;
mod afm_matches_r;
mod alpha_matches_r;
mod arrows_matches_r;
mod axis_args_matches_r;
mod axis_matches_r;
mod axis_zero_tick_matches_r;
mod axp_matches_r;
mod backend_smoke;
mod barplot_extras_matches_r;
mod barplot_matches_r;
mod boxplot_draw_matches_r;
mod boxplot_extras_matches_r;
mod boxplot_matches_r;
mod bty_title_pars_matches_r;
mod colour_zero_matches_r;
mod device_layout_matches_r;
mod device_open;
mod figure_legends_matches_r;
mod figure_replay;
mod frozen_margins;
mod hatch_matches_r;
mod hist_matches_r;
mod inline_args_matches_r;
mod integer_args;
mod kerning_matches_r;
mod latin1_text_matches_r;
mod layout_matches_r;
mod legend_extras_matches_r;
mod legend_matches_r;
mod line_pars_matches_r;
mod log_axis_matches_r;
mod log_plots_matches_r;
mod margins_regions_matches_r;
mod mtext_matches_r;
mod multipage_matches_r;
mod na_values_matches_r;
mod page_background;
mod page_files;
mod par_new_matches_r;
mod par_page_layout;
mod pch_matches_r;
mod pie_matches_r;
mod plot_new_matches_r;
mod plot_type_b_matches_r;
mod plot_type_h_s_matches_r;
mod plotmath_axis_matches_r;
mod plotmath_extras_matches_r;
mod pretty_matches_r;
mod primitives_matches_r;
mod region_errors_matches_r;
mod regions_matches_r;
mod required_args;
mod start_ps_matches_r;
mod tck_srt_matches_r;
mod text_matches_r;
mod text_metrics_matches_r;
mod xpd_na_matches_r;
mod xy_lengths_matches_r;
