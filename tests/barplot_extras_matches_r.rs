// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-checks `barplot()`'s `offset` (vector, `beside` and stacked; the
//! stacked case with `ylim` given, since R's default limits there use a
//! mis-recycled `offset`), `orderH` (with tied heights), `add = TRUE`
//! (titles off by default, and `ann = TRUE`) and `args.legend` (keyword
//! position with `bty`/`inset`/`title`; `(x, y)` with `ncol`/`xjust`/`cex`)
//! against a real R `pdf()` device's uncompressed output. Compares every
//! painted path in order (vertices or, for curves, bounding boxes; fill,
//! stroke colour, width, cap, dash) and every text item's string and
//! origin, except labels with a minus sign (R's `pdf()` draws `-` as a
//! wider `/minus`; see CLAUDE.md). Skipped if `Rscript` isn't available.

mod common;

use std::process::Command;

use rs_graphics::prelude::*;

const PAGE_W: f64 = 7.0;
const PAGE_H: f64 = 5.0;

/// A 3 x 4 matrix as its columns; the second column has tied rows.
const M: [[f64; 3]; 4] = [
    [1.0, 4.0, 2.0],
    [3.0, 1.0, 3.0],
    [2.0, 2.0, 5.0],
    [4.0, 3.0, 1.0],
];

fn mat() -> Vec<Vec<f64>> {
    M.iter().map(|c| c.to_vec()).collect()
}

fn r_matrix() -> String {
    let cols: Vec<String> = M
        .iter()
        .map(|c| {
            let v: Vec<String> = c.iter().map(|x| x.to_string()).collect();
            format!("c({})", v.join(", "))
        })
        .collect();
    format!("cbind({})", cols.join(", "))
}

const ROWS: [&str; 3] = ["r1", "r2", "r3"];

type Draw = fn(&mut Device<PdfBackend>);

struct Case {
    name: &'static str,
    /// R code; `m` is the matrix above.
    r: &'static str,
    ours: Draw,
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "offset_vector",
            r: "barplot(c(3, 1, 2, 4), offset = c(1, 5, 2), names.arg = c('a', 'b', 'c', 'd'))",
            ours: |d| {
                d.barplot()
                    .height([3.0, 1.0, 2.0, 4.0])
                    .offset([1.0, 5.0, 2.0])
                    .names(["a", "b", "c", "d"])
                    .draw();
            },
        },
        Case {
            name: "offset_beside",
            r: "barplot(m, beside = TRUE, offset = c(1, 2, 0), legend.text = c('r1', 'r2', 'r3'))",
            ours: |d| {
                d.barplot()
                    .height(mat())
                    .beside(true)
                    .offset([1.0, 2.0, 0.0])
                    .legend_text(ROWS)
                    .draw();
            },
        },
        Case {
            name: "offset_stacked",
            r: "barplot(m, offset = c(0, 10, 0, 5), ylim = c(0, 25))",
            ours: |d| {
                d.barplot()
                    .height(mat())
                    .offset([0.0, 10.0, 0.0, 5.0])
                    .ylim(0.0, 25.0)
                    .draw();
            },
        },
        Case {
            name: "order_decr",
            r: "barplot(m, orderH = 'decr', col = c('red', 'green', 'blue'), \
                legend.text = c('r1', 'r2', 'r3'))",
            ours: |d| {
                d.barplot()
                    .height(mat())
                    .order_h(OrderH::Decr)
                    .col(["red", "green", "blue"])
                    .legend_text(ROWS)
                    .draw();
            },
        },
        Case {
            name: "order_incr_horiz",
            r: "barplot(m, orderH = 'incr', horiz = TRUE, col = c('red', 'green', 'blue'))",
            ours: |d| {
                d.barplot()
                    .height(mat())
                    .order_h(OrderH::Incr)
                    .horiz(true)
                    .col(["red", "green", "blue"])
                    .draw();
            },
        },
        Case {
            name: "add",
            r: "plot(c(0, 6), c(0, 12), type = 'n', ann = FALSE)\n\
                barplot(c(3, 8, 5), add = TRUE, space = 1, col = 'white', main = 'hidden', \
                names.arg = c('x', 'y', 'z'))",
            ours: |d| {
                d.plot()
                    .x(&[0.0, 6.0])
                    .y(&[0.0, 12.0])
                    .type_(PlotType::None)
                    .draw();
                d.barplot()
                    .height([3.0, 8.0, 5.0])
                    .add(true)
                    .space([1.0])
                    .col("white")
                    .main("hidden")
                    .names(["x", "y", "z"])
                    .draw();
            },
        },
        Case {
            name: "add_ann",
            r: "plot(c(0, 6), c(0, 12), type = 'n', ann = FALSE)\n\
                barplot(c(3, 8, 5), add = TRUE, ann = TRUE, axes = FALSE, main = 'title', \
                xlab = 'x label')",
            ours: |d| {
                d.plot()
                    .x(&[0.0, 6.0])
                    .y(&[0.0, 12.0])
                    .type_(PlotType::None)
                    .draw();
                d.barplot()
                    .height([3.0, 8.0, 5.0])
                    .add(true)
                    .ann(true)
                    .axes(false)
                    .main("title")
                    .xlab("x label")
                    .draw();
            },
        },
        Case {
            name: "args_legend_keyword",
            r: "barplot(m, legend.text = c('r1', 'r2', 'r3'), \
                args.legend = list(x = 'topleft', bty = 'n', inset = 0.05, title = 'Groups'))",
            ours: |d| {
                d.barplot()
                    .height(mat())
                    .legend_text(ROWS)
                    .legend_pos("topleft")
                    .args_legend(
                        LegendOptions::default()
                            .bty(LegendBty::None)
                            .inset((0.05, 0.05))
                            .title("Groups"),
                    )
                    .draw();
            },
        },
        Case {
            name: "args_legend_xy",
            r: "barplot(m, beside = TRUE, legend.text = c('r1', 'r2', 'r3'), ylim = c(0, 8), \
                args.legend = list(x = 2, y = 7.5, ncol = 3, xjust = 0, cex = 0.8))",
            ours: |d| {
                d.barplot()
                    .height(mat())
                    .beside(true)
                    .legend_text(ROWS)
                    .ylim(0.0, 8.0)
                    .legend_pos((2.0, 7.5))
                    .args_legend(LegendOptions::default().ncol(3usize).xjust(0.0).cex(0.8))
                    .draw();
            },
        },
    ]
}

// -------------------------------------------------------------------
// A minimal PDF content-stream interpreter (as in
// `boxplot_draw_matches_r.rs`), plus text origins.
// -------------------------------------------------------------------

#[derive(Clone, Debug)]
struct Gs {
    fill: [f64; 3],
    stroke: [f64; 3],
    lwd: f64,
    cap: i32,
    dash: Vec<f64>,
}

#[derive(Debug)]
struct Painted {
    vertices: Vec<(f64, f64)>,
    curved: bool,
    fill: Option<[f64; 3]>,
    /// `(colour, line width, line cap, dash array)`.
    stroke: Option<([f64; 3], f64, i32, Vec<f64>)>,
}

impl Painted {
    fn bbox(&self) -> [f64; 4] {
        let xs = self.vertices.iter().map(|p| p.0);
        let ys = self.vertices.iter().map(|p| p.1);
        [
            xs.clone().fold(f64::INFINITY, f64::min),
            ys.clone().fold(f64::INFINITY, f64::min),
            xs.fold(f64::NEG_INFINITY, f64::max),
            ys.fold(f64::NEG_INFINITY, f64::max),
        ]
    }
}

fn tokens(content: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut chars = content.chars().peekable();
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
        } else if c == '[' || c == ']' {
            chars.next();
            out.push(c.to_string());
        } else {
            let mut s = String::new();
            while let Some(&c) = chars.peek() {
                if c.is_whitespace() || c == '[' || c == ']' {
                    break;
                }
                s.push(c);
                chars.next();
            }
            out.push(s);
        }
    }
    out
}

/// Every painted path of the page's content stream, in order, minus clips
/// and full-page backgrounds.
fn interpret(pdf: &str) -> (Vec<Painted>, Vec<Text>) {
    let mut painted = Vec::new();
    let mut texts = Vec::new();
    let mut rest = pdf;
    while let Some(i) = rest.find("stream\n") {
        let body = &rest[i + 7..];
        let Some(j) = body.find("endstream") else {
            break;
        };
        let content = &body[..j];
        rest = &body[j + 9..];
        if !content.contains(" m") && !content.contains(" re") {
            continue; // ICC profile, fonts, ...
        }
        let mut gs = Gs {
            fill: [0.0; 3],
            stroke: [0.0; 3],
            lwd: 1.0,
            cap: 0,
            dash: Vec::new(),
        };
        let mut stack: Vec<Gs> = Vec::new();
        let mut nums: Vec<f64> = Vec::new();
        let mut array: Option<Vec<f64>> = None;
        let mut path: Vec<(f64, f64)> = Vec::new();
        let mut curved = false;
        let mut origin = (0.0, 0.0);
        for t in tokens(content) {
            if t.starts_with('(') {
                origin_text(&mut texts, origin, &t);
                continue;
            }
            if let Ok(v) = t.parse::<f64>() {
                match &mut array {
                    Some(a) => a.push(v),
                    None => nums.push(v),
                }
                continue;
            }
            let last = |k: usize| nums[nums.len() - k..].to_vec();
            match t.as_str() {
                "[" => array = Some(Vec::new()),
                "]" => {
                    // Only dash arrays appear here; `d` follows with a phase.
                    gs.dash = array.take().unwrap();
                    continue;
                }
                "q" => stack.push(gs.clone()),
                "Q" => gs = stack.pop().unwrap(),
                "rg" | "scn" => gs.fill = last(3).try_into().unwrap(),
                "RG" | "SCN" => gs.stroke = last(3).try_into().unwrap(),
                "g" => gs.fill = [nums[nums.len() - 1]; 3],
                "G" => gs.stroke = [nums[nums.len() - 1]; 3],
                "w" => gs.lwd = nums[nums.len() - 1],
                "J" => gs.cap = nums[nums.len() - 1] as i32,
                "m" | "l" => path.push((nums[nums.len() - 2], nums[nums.len() - 1])),
                "c" => {
                    let n = last(6);
                    path.extend([(n[0], n[1]), (n[2], n[3]), (n[4], n[5])]);
                    curved = true;
                }
                "re" => {
                    // Normalised to a positive width and height: R writes
                    // the legend box with a negative height.
                    let mut n = last(4);
                    if n[2] < 0.0 {
                        n[0] += n[2];
                        n[2] = -n[2];
                    }
                    if n[3] < 0.0 {
                        n[1] += n[3];
                        n[3] = -n[3];
                    }
                    path.extend([
                        (n[0], n[1]),
                        (n[0] + n[2], n[1]),
                        (n[0] + n[2], n[1] + n[3]),
                        (n[0], n[1] + n[3]),
                    ]);
                }
                "Tm" => origin = (nums[nums.len() - 2], nums[nums.len() - 1]),
                "W" | "W*" | "h" => {}
                "n" => {
                    path.clear();
                    curved = false;
                }
                "S" | "s" | "f" | "F" | "f*" | "B" | "B*" | "b" | "b*" => {
                    let fill = t.starts_with(['f', 'F', 'B', 'b']);
                    let stroke = t.starts_with(['S', 's', 'B', 'b']);
                    let p = Painted {
                        vertices: std::mem::take(&mut path),
                        curved: std::mem::take(&mut curved),
                        fill: fill.then_some(gs.fill),
                        stroke: stroke.then(|| (gs.stroke, gs.lwd, gs.cap, gs.dash.clone())),
                    };
                    let b = p.bbox();
                    if b[2] - b[0] < 0.9 * PAGE_W * 72.0 {
                        painted.push(p);
                    }
                }
                _ => {}
            }
            nums.clear();
        }
    }
    (painted, texts)
}

/// A drawn string and its text-matrix origin.
#[derive(Debug)]
struct Text {
    string: String,
    origin: (f64, f64),
}

fn origin_text(texts: &mut Vec<Text>, origin: (f64, f64), token: &str) {
    texts.push(Text {
        string: token
            .trim_start_matches('(')
            .trim_end_matches(')')
            .to_string(),
        origin,
    });
}

fn close(a: &[f64], b: &[f64], tol: f64) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < tol)
}

#[test]
fn barplot_extras_match_r() {
    if !common::r_available() {
        return;
    }
    let dir = std::env::temp_dir();
    let cases = cases();
    let mut script = format!("m <- {}\n", r_matrix());
    for c in &cases {
        let path = dir.join(format!("rs_graphics_bar_extras_{}.pdf", c.name));
        script.push_str(&format!(
            "pdf(file = '{}', width = {PAGE_W}, height = {PAGE_H}, compress = FALSE)\n\
             suppressWarnings({{\n{}\n}})\ninvisible(dev.off())\n",
            path.display(),
            c.r
        ));
    }
    let script_path = dir.join("rs_graphics_bar_extras.R");
    std::fs::write(&script_path, script).unwrap();
    let out = Command::new("Rscript")
        .arg(&script_path)
        .output()
        .expect("run Rscript");
    assert!(
        out.status.success(),
        "Rscript failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    for c in &cases {
        let path = dir.join(format!("rs_graphics_bar_extras_{}.pdf", c.name));
        let r_pdf = String::from_utf8_lossy(&std::fs::read(&path).unwrap()).into_owned();
        let (r_paths, r_texts) = interpret(&r_pdf);
        let mut dev = Device::new(
            PdfBackend::new(PAGE_W, PAGE_H).with_compression(false),
            Par::default(),
        );
        (c.ours)(&mut dev);
        let ours = String::from_utf8_lossy(&dev.into_backend().finish()).into_owned();
        let (our_paths, our_texts) = interpret(&ours);
        let name = c.name;

        assert_eq!(
            our_paths.len(),
            r_paths.len(),
            "{name}: painted path count\nours={our_paths:#?}\nr={r_paths:#?}"
        );
        for (k, (a, b)) in our_paths.iter().zip(&r_paths).enumerate() {
            let ctx = format!("{name}, path {k}:\n ours={a:?}\n r=   {b:?}");
            assert_eq!(a.curved, b.curved, "curvedness differs. {ctx}");
            if a.curved {
                assert!(close(&a.bbox(), &b.bbox(), 0.05), "bbox differs. {ctx}");
            } else {
                let flat = |p: &Painted| -> Vec<f64> {
                    p.vertices.iter().flat_map(|&(x, y)| [x, y]).collect()
                };
                assert!(close(&flat(a), &flat(b), 0.05), "vertices differ. {ctx}");
            }
            assert_eq!(a.fill.is_some(), b.fill.is_some(), "fill differs. {ctx}");
            assert_eq!(
                a.stroke.is_some(),
                b.stroke.is_some(),
                "stroke differs. {ctx}"
            );
            if let (Some(fa), Some(fb)) = (a.fill, b.fill) {
                assert!(close(&fa, &fb, 0.005), "fill colour differs. {ctx}");
            }
            if let (Some(sa), Some(sb)) = (&a.stroke, &b.stroke) {
                assert!(close(&sa.0, &sb.0, 0.005), "stroke colour differs. {ctx}");
                assert!((sa.1 - sb.1).abs() < 0.005, "line width differs. {ctx}");
                assert_eq!(sa.2, sb.2, "line cap differs. {ctx}");
                assert!(close(&sa.3, &sb.3, 0.005), "dash pattern differs. {ctx}");
            }
        }

        let strings = |t: &[Text]| t.iter().map(|t| t.string.clone()).collect::<Vec<_>>();
        assert_eq!(strings(&our_texts), strings(&r_texts), "{name}: text items");
        for (a, b) in our_texts.iter().zip(&r_texts) {
            if a.string.contains('-') {
                continue;
            }
            assert!(
                close(&[a.origin.0, a.origin.1], &[b.origin.0, b.origin.1], 0.05),
                "{name}: text origin differs\n ours={a:?}\n r=   {b:?}"
            );
        }
    }
}
