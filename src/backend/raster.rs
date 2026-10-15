//! PNG rasterization backend, built on `tiny-skia` for path filling/
//! stroking and `ab_glyph` (with bundled Liberation Sans faces, see
//! `assets/fonts/`) for glyph rasterization.
//!
//! Text *layout* (advance widths, kerning) still always comes from
//! `crate::afm`'s Helvetica AFM tables - the same ones `pdf()`/`svg()`
//! effectively rely on - so a string's overall width/position matches R
//! exactly. Only individual glyph *shapes* come from Liberation Sans
//! here, since PNG is the one backend that has no choice but to rasterize
//! actual glyph outlines. Liberation Sans (SIL OFL) is metric-compatible
//! with Arial, whose advance widths are identical to Helvetica's, so
//! glyphs sit naturally in their AFM-derived positions and the shapes are
//! close to the Helvetica that R's own `png()` draws.
//!
//! # Multi-threaded drawing
//! Drawing calls are not painted immediately. Each is recorded as a
//! command (already in pixel space, with its clip state and pixel bounding
//! box), and the canvas is held as square tiles of [`TILE`] pixels. Every
//! [`FLUSH_AT`] commands, and in [`RasterBackend::finish`], the pending
//! commands are binned into the tiles they touch and the tiles are painted
//! on separate threads, each in command order. Paths are only translated
//! by whole pixels, so shapes are anti-aliased as on one big canvas, apart
//! from the small edge effects described at [`PAD`]. The output does not
//! depend on the number of threads (above one; see
//! [`RasterBackend::with_threads`]).

use std::sync::{Mutex, OnceLock};

use ab_glyph::{Font, FontArc, Glyph as AbGlyph};
use tiny_skia::{
    FillRule, Mask, Paint, Path, PathBuilder, Pixmap, PremultipliedColorU8, Rect as SkRect,
    Stroke as SkStroke, Transform,
};

use super::{Backend, LineEnd, LineJoin as BeLineJoin, Stroke, TextStyle};
use crate::afm::{self, FontFace};
use crate::color::Color;
use crate::geometry::{Point, Rect};
use crate::lty::LineType;

struct Fonts {
    plain: FontArc,
    bold: FontArc,
    italic: FontArc,
    bold_italic: FontArc,
}

fn fonts() -> &'static Fonts {
    static FONTS: OnceLock<Fonts> = OnceLock::new();
    FONTS.get_or_init(|| Fonts {
        plain: FontArc::try_from_slice(include_bytes!(
            "../../assets/fonts/LiberationSans-Regular.ttf"
        ))
        .expect("bundled LiberationSans-Regular.ttf must parse"),
        bold: FontArc::try_from_slice(include_bytes!("../../assets/fonts/LiberationSans-Bold.ttf"))
            .expect("bundled LiberationSans-Bold.ttf must parse"),
        italic: FontArc::try_from_slice(include_bytes!(
            "../../assets/fonts/LiberationSans-Italic.ttf"
        ))
        .expect("bundled LiberationSans-Italic.ttf must parse"),
        bold_italic: FontArc::try_from_slice(include_bytes!(
            "../../assets/fonts/LiberationSans-BoldItalic.ttf"
        ))
        .expect("bundled LiberationSans-BoldItalic.ttf must parse"),
    })
}

fn font_for(face: FontFace) -> &'static FontArc {
    let f = fonts();
    match face {
        FontFace::Plain => &f.plain,
        FontFace::Bold => &f.bold,
        FontFace::Italic => &f.italic,
        FontFace::BoldItalic => &f.bold_italic,
    }
}

/// Side of a canvas tile, in pixels, when painting on several threads.
/// (On one thread the whole canvas is one tile, which is fastest.)
const TILE: u32 = 64;

/// Extra pixels painted around each tile (and then discarded). tiny-skia
/// chops paths at the edge of the pixmap and re-subdivides the curves it
/// cuts, which shifts their anti-aliasing by up to ~1/8 of a pixel; this
/// keeps that away from the kept pixels for typical point sizes.
const PAD: u32 = 4;

/// Margin around a hairline's path that contains all its ink, in pixels.
const HAIRLINE_MARGIN: f32 = 4.0;

/// Pending commands that trigger painting, bounding the memory they take.
const FLUSH_AT: usize = 1 << 21;

/// What a recorded command paints. Coordinates are page pixels.
enum Op {
    Fill {
        path: Box<Path>,
        color: Color,
    },
    /// `stroke` indexes `RasterBackend::strokes`.
    Stroke {
        path: Box<Path>,
        color: Color,
        stroke: u32,
    },
    /// A filled circle, kept unbuilt: most commands in big scatter plots
    /// are these, and building the path on the painting thread is cheaper.
    Circle {
        cx: f32,
        cy: f32,
        r: f32,
        color: Color,
    },
    /// Glyph coverage samples `(x, y, coverage)`, blended pixel by pixel.
    Glyphs {
        samples: Box<[(i32, i32, f32)]>,
        color: Color,
    },
}

struct Command {
    op: Op,
    /// Index into `RasterBackend::clips`.
    clip: u32,
    /// Pixel bounding box `[x0, y0, x1, y1)`, already within the canvas.
    bbox: [i32; 4],
}

/// A `w` x `h` block of the canvas at `(x0, y0)`. `pixmap` also holds
/// `pad` pixels on every side, so its origin is `(x0 - pad, y0 - pad)`.
struct Tile {
    x0: u32,
    y0: u32,
    w: u32,
    h: u32,
    pad: u32,
    pixmap: Pixmap,
}

impl Tile {
    /// Page-pixel coordinates of `pixmap`'s top-left corner.
    fn origin(&self) -> (i32, i32) {
        (
            self.x0 as i32 - self.pad as i32,
            self.y0 as i32 - self.pad as i32,
        )
    }

    /// Whether `r` (page pixels) lies within `pixmap`.
    fn holds(&self, r: &SkRect) -> bool {
        let (ox, oy) = self.origin();
        r.left() >= ox as f32
            && r.top() >= oy as f32
            && r.right() <= (ox + self.pixmap.width() as i32) as f32
            && r.bottom() <= (oy + self.pixmap.height() as i32) as f32
    }
}

/// A hairline painted on its own: `pixmap` goes at page pixel `(x, y)`.
struct Prepainted {
    x: i32,
    y: i32,
    pixmap: Pixmap,
}

pub struct RasterBackend {
    width_in: f64,
    height_in: f64,
    dpi: f64,
    width_px: u32,
    height_px: u32,
    tiles: Vec<Tile>,
    /// Tile side and padding (see [`TILE`], [`PAD`]), and tiles per row.
    tile: u32,
    pad: u32,
    tiles_x: u32,
    threads: usize,
    commands: Vec<Command>,
    /// Stroke styles used by pending commands (consecutive repeats shared).
    strokes: Vec<SkStroke>,
    /// Distinct clip stacks (rectangles in pixels, intersected in order);
    /// entry 0 is "no clip".
    clips: Vec<Vec<SkRect>>,
    clip_stack: Vec<SkRect>,
    clip_id: u32,
}

impl RasterBackend {
    /// `dpi` is pixels-per-inch (R's `png()` default is `72` for its
    /// pixel-dimension convention, i.e. `width_px = width_in * 72` unless
    /// `res=` is overridden; pass a higher `dpi` for sharper output).
    pub fn new(width_in: f64, height_in: f64, dpi: f64, background: Color) -> Self {
        let w_px = (width_in * dpi).round().max(1.0) as u32;
        let h_px = (height_in * dpi).round().max(1.0) as u32;
        let mut canvas = Pixmap::new(w_px, h_px).expect("nonzero pixmap size");
        canvas.fill(to_sk_color(background));
        let mut backend = RasterBackend {
            width_in,
            height_in,
            dpi,
            width_px: w_px,
            height_px: h_px,
            tiles: Vec::new(),
            tile: 0,
            pad: 0,
            tiles_x: 0,
            threads: 1,
            commands: Vec::new(),
            strokes: Vec::new(),
            clips: vec![Vec::new()],
            clip_stack: Vec::new(),
            clip_id: 0,
        };
        let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
        backend.set_threads(threads, &canvas);
        backend
    }

    /// Paint with at most `threads` threads (default: the number of CPUs).
    /// With one thread the output is exactly that of drawing onto a single
    /// canvas; with more, edge pixels of some shapes can differ from it by
    /// a few levels (see [`PAD`]), but not between thread counts above one.
    pub fn with_threads(mut self, threads: usize) -> Self {
        self.flush();
        let canvas = self.canvas();
        self.set_threads(threads, &canvas);
        self
    }

    /// Set the thread count and re-tile `canvas` to suit it.
    fn set_threads(&mut self, threads: usize, canvas: &Pixmap) {
        self.threads = threads.max(1);
        (self.tile, self.pad) = if self.threads == 1 {
            (self.width_px.max(self.height_px), 0)
        } else {
            (TILE, PAD)
        };
        let (tile, pad) = (self.tile, self.pad);
        self.tiles_x = self.width_px.div_ceil(tile);
        self.tiles.clear();
        for y0 in (0..self.height_px).step_by(tile as usize) {
            for x0 in (0..self.width_px).step_by(tile as usize) {
                let (w, h) = (tile.min(self.width_px - x0), tile.min(self.height_px - y0));
                let mut pixmap = Pixmap::new(w + 2 * pad, h + 2 * pad).expect("nonzero tile size");
                let rect = tiny_skia::IntRect::from_xywh(x0 as i32, y0 as i32, w, h)
                    .expect("nonzero tile size");
                let part = canvas.clone_rect(rect).expect("tile within canvas");
                pixmap.draw_pixmap(
                    pad as i32,
                    pad as i32,
                    part.as_ref(),
                    &tiny_skia::PixmapPaint {
                        blend_mode: tiny_skia::BlendMode::Source,
                        ..Default::default()
                    },
                    Transform::identity(),
                    None,
                );
                self.tiles.push(Tile {
                    x0,
                    y0,
                    w,
                    h,
                    pad,
                    pixmap,
                });
            }
        }
    }

    fn to_px(&self, p: Point) -> (f32, f32) {
        (
            (p.x * self.dpi) as f32,
            (self.height_in * self.dpi - p.y * self.dpi) as f32,
        )
    }

    fn tiny_skia_stroke(stroke: &Stroke, dpi: f64) -> SkStroke {
        let width_px = (stroke.width_pt() / 72.0 * dpi) as f32;
        let line_cap = match stroke.lend {
            LineEnd::Butt => tiny_skia::LineCap::Butt,
            LineEnd::Round => tiny_skia::LineCap::Round,
            LineEnd::Square => tiny_skia::LineCap::Square,
        };
        let line_join = match stroke.ljoin {
            BeLineJoin::Mitre => tiny_skia::LineJoin::Miter,
            BeLineJoin::Round => tiny_skia::LineJoin::Round,
            BeLineJoin::Bevel => tiny_skia::LineJoin::Bevel,
        };
        let dash = stroke
            .lty
            .dash_lengths_pt(stroke.lwd, stroke.lend == LineEnd::Round)
            .and_then(|pts| {
                let px: Vec<f32> = pts.iter().map(|d| (d / 72.0 * dpi) as f32).collect();
                tiny_skia::StrokeDash::new(px, 0.0)
            });
        SkStroke {
            width: width_px.max(0.05),
            miter_limit: stroke.lmitre as f32,
            line_cap,
            line_join,
            dash,
        }
    }

    /// Queue `op`, covering the pixel box `(x0, y0, x1, y1)`, under the
    /// current clip. Does nothing if it falls outside the canvas or clip.
    fn record(&mut self, op: Op, (mut x0, mut y0, mut x1, mut y1): (f32, f32, f32, f32)) {
        for r in &self.clip_stack {
            x0 = x0.max(r.left() - 1.0);
            y0 = y0.max(r.top() - 1.0);
            x1 = x1.min(r.right() + 1.0);
            y1 = y1.min(r.bottom() + 1.0);
        }
        let bbox = [
            (x0.floor().max(0.0) as i64).min(self.width_px as i64) as i32,
            (y0.floor().max(0.0) as i64).min(self.height_px as i64) as i32,
            (x1.ceil().max(0.0) as i64).min(self.width_px as i64) as i32,
            (y1.ceil().max(0.0) as i64).min(self.height_px as i64) as i32,
        ];
        // NaN coordinates fail these comparisons too.
        if !(bbox[0] < bbox[2] && bbox[1] < bbox[3]) {
            return;
        }
        self.commands.push(Command {
            op,
            clip: self.clip_id,
            bbox,
        });
        if self.commands.len() >= FLUSH_AT {
            self.flush();
        }
    }

    fn stroke_shape(&mut self, path: Path, stroke: &Stroke) {
        if matches!(stroke.lty, LineType::Blank) {
            return;
        }
        let sk_stroke = Self::tiny_skia_stroke(stroke, self.dpi);
        // Generous: covers mitre joins up to the mitre limit and square caps.
        let pad = sk_stroke.width * sk_stroke.miter_limit.max(1.5) / 2.0 + 2.0;
        if self.strokes.last() != Some(&sk_stroke) {
            self.strokes.push(sk_stroke);
        }
        let b = path.bounds();
        let bounds = (
            b.left() - pad,
            b.top() - pad,
            b.right() + pad,
            b.bottom() + pad,
        );
        let op = Op::Stroke {
            path: Box::new(path),
            color: stroke.color,
            stroke: (self.strokes.len() - 1) as u32,
        };
        self.record(op, bounds);
    }

    fn fill_shape(&mut self, path: Path, color: Color) {
        let b = path.bounds();
        let bounds = (
            b.left() - 1.0,
            b.top() - 1.0,
            b.right() + 1.0,
            b.bottom() + 1.0,
        );
        self.record(
            Op::Fill {
                path: Box::new(path),
                color,
            },
            bounds,
        );
    }

    fn fill_circle(&mut self, cx: f32, cy: f32, r: f32, color: Color) {
        let bounds = (cx - r - 1.0, cy - r - 1.0, cx + r + 1.0, cy + r + 1.0);
        self.record(Op::Circle { cx, cy, r, color }, bounds);
    }

    fn path_from_points(&self, points: &[Point], close: bool) -> Option<Path> {
        let mut pb = PathBuilder::new();
        for (i, p) in points.iter().enumerate() {
            let (x, y) = self.to_px(*p);
            if i == 0 {
                pb.move_to(x, y);
            } else {
                pb.line_to(x, y);
            }
        }
        if close {
            pb.close();
        }
        pb.finish()
    }

    /// Paint all pending commands into the tiles, in parallel.
    fn flush(&mut self) {
        if self.commands.is_empty() {
            return;
        }
        let (w, h) = (self.width_px as f32, self.height_px as f32);
        let in_canvas =
            |r: &SkRect| r.left() >= 0.0 && r.top() >= 0.0 && r.right() <= w && r.bottom() <= h;
        let mut bins: Vec<Vec<u32>> = vec![Vec::new(); self.tiles.len()];
        // Hairlines that a tile would cut (see `Prepainted`), in order.
        let mut cut_hairlines: Vec<u32> = Vec::new();
        for (i, c) in self.commands.iter().enumerate() {
            // Every tile whose padded area the command touches.
            let (tile, pad) = (self.tile, self.pad);
            let [x0, y0, x1, y1] = c.bbox.map(|v| v as u32);
            let (tx0, ty0) = (x0.saturating_sub(pad) / tile, y0.saturating_sub(pad) / tile);
            let tx_end = ((x1 - 1 + pad) / tile).min(self.tiles_x - 1);
            let ty_end = ((y1 - 1 + pad) / tile).min(self.tiles.len() as u32 / self.tiles_x - 1);
            for ty in ty0..=ty_end {
                for tx in tx0..=tx_end {
                    bins[(ty * self.tiles_x + tx) as usize].push(i as u32);
                }
            }
            if self.tiles.len() > 1 {
                if let Some(r) = hairline_bounds(c, &self.strokes) {
                    let first = &self.tiles[(ty0 * self.tiles_x + tx0) as usize];
                    let one_tile = tx0 == tx_end && ty0 == ty_end;
                    if !(one_tile && first.holds(&r) && in_canvas(&r)) {
                        cut_hairlines.push(i as u32);
                    }
                }
            }
        }
        let prepainted = self.prepaint_hairlines(&cut_hairlines);
        // Busiest tiles first, so one isn't left running alone at the end.
        let mut work: Vec<(&mut Tile, &Vec<u32>)> = self
            .tiles
            .iter_mut()
            .zip(&bins)
            .filter(|(_, b)| !b.is_empty())
            .collect();
        work.sort_by_key(|(_, b)| std::cmp::Reverse(b.len()));
        let threads = self.threads.min(work.len());
        let ctx = PaintContext {
            commands: &self.commands,
            strokes: &self.strokes,
            clips: &self.clips,
            cut_hairlines: &cut_hairlines,
            prepainted: &prepainted,
        };
        if threads <= 1 {
            for (tile, bin) in work {
                ctx.paint_tile(tile, bin);
            }
        } else {
            let queue = Mutex::new(work.into_iter());
            std::thread::scope(|s| {
                for _ in 0..threads {
                    s.spawn(|| loop {
                        let next = queue.lock().unwrap().next();
                        let Some((tile, bin)) = next else { break };
                        ctx.paint_tile(tile, bin);
                    });
                }
            });
        }
        self.commands.clear();
        self.strokes.clear();
    }

    /// Paint each of the hairline commands `cut` on its own pixmap, covering
    /// its ink within the canvas, so that only the canvas edges cut it, as
    /// when painting the canvas whole. In parallel.
    fn prepaint_hairlines(&self, cut: &[u32]) -> Vec<Option<Prepainted>> {
        let prepaint = |&i: &u32| -> Option<Prepainted> {
            let c = &self.commands[i as usize];
            let Op::Stroke {
                path,
                color,
                stroke,
            } = &c.op
            else {
                unreachable!("only strokes are hairlines")
            };
            let r = hairline_bounds(c, &self.strokes)?;
            let x = r.left().floor().max(0.0) as i32;
            let y = r.top().floor().max(0.0) as i32;
            let x1 = (r.right().ceil() as i32).min(self.width_px as i32);
            let y1 = (r.bottom().ceil() as i32).min(self.height_px as i32);
            let mut pixmap = Pixmap::new((x1 - x).max(0) as u32, (y1 - y).max(0) as u32)?;
            let to_pixmap = Transform::from_translate(-x as f32, -y as f32);
            let stroke = &self.strokes[*stroke as usize];
            pixmap.stroke_path(path, &paint_for(*color), stroke, to_pixmap, None);
            Some(Prepainted { x, y, pixmap })
        };
        let threads = self.threads.min(cut.len());
        if threads <= 1 {
            return cut.iter().map(prepaint).collect();
        }
        let per_thread = cut.len().div_ceil(threads);
        std::thread::scope(|s| {
            let handles: Vec<_> = cut
                .chunks(per_thread)
                .map(|chunk| s.spawn(move || chunk.iter().map(prepaint).collect::<Vec<_>>()))
                .collect();
            handles
                .into_iter()
                .flat_map(|h| h.join().expect("hairline painting thread panicked"))
                .collect()
        })
    }

    /// The tiles (without padding) assembled into one pixmap. Pending
    /// commands must have been flushed.
    fn canvas(&self) -> Pixmap {
        let mut canvas = Pixmap::new(self.width_px, self.height_px).expect("nonzero pixmap size");
        let row_bytes = self.width_px as usize * 4;
        for tile in &self.tiles {
            let tile_row = tile.pixmap.width() as usize * 4;
            let rows = tile.pixmap.data().chunks(tile_row);
            for (r, src) in rows
                .skip(tile.pad as usize)
                .take(tile.h as usize)
                .enumerate()
            {
                let src = &src[tile.pad as usize * 4..(tile.pad + tile.w) as usize * 4];
                let at = (tile.y0 as usize + r) * row_bytes + tile.x0 as usize * 4;
                canvas.data_mut()[at..at + src.len()].copy_from_slice(src);
            }
        }
        canvas
    }

    /// Encode the current canvas as PNG bytes.
    pub fn finish(mut self) -> Vec<u8> {
        self.flush();
        self.canvas().encode_png().expect("PNG encoding failed")
    }
}

/// The pixel bounds of command `c`'s ink if it is a hairline stroke, one
/// at most a pixel wide. tiny-skia draws those with an algorithm whose
/// output changes slightly along the whole segment wherever the pixmap
/// edge cuts it, unlike wider strokes and fills.
fn hairline_bounds(c: &Command, strokes: &[SkStroke]) -> Option<SkRect> {
    let Op::Stroke { path, stroke, .. } = &c.op else {
        return None;
    };
    if strokes[*stroke as usize].width > 1.0 {
        return None;
    }
    let b = path.bounds();
    SkRect::from_ltrb(
        b.left() - HAIRLINE_MARGIN,
        b.top() - HAIRLINE_MARGIN,
        b.right() + HAIRLINE_MARGIN,
        b.bottom() + HAIRLINE_MARGIN,
    )
}

/// The read-only state shared by the painting threads.
#[derive(Clone, Copy)]
struct PaintContext<'a> {
    commands: &'a [Command],
    strokes: &'a [SkStroke],
    clips: &'a [Vec<SkRect>],
    /// Hairline commands painted on their own, sorted, and their pixmaps.
    cut_hairlines: &'a [u32],
    prepainted: &'a [Option<Prepainted>],
}

impl PaintContext<'_> {
    /// Paint the commands `bin` (indices, in drawing order) into `tile`.
    fn paint_tile(&self, tile: &mut Tile, bin: &[u32]) {
        let (w, h) = (tile.pixmap.width(), tile.pixmap.height());
        let (ox, oy) = tile.origin();
        let to_tile = Transform::from_translate(-ox as f32, -oy as f32);
        // Clip masks built for this tile so far: (clip id, mask or None if
        // the clip covers the whole tile).
        let mut masks: Vec<(u32, Option<Mask>)> = Vec::new();
        for &i in bin {
            let cmd = &self.commands[i as usize];
            let mi = match masks.iter().position(|(id, _)| *id == cmd.clip) {
                Some(mi) => mi,
                None => {
                    masks.push((cmd.clip, self.clip_mask(cmd.clip, tile, to_tile)));
                    masks.len() - 1
                }
            };
            let mask = masks[mi].1.as_ref();
            let pixmap = &mut tile.pixmap;
            match &cmd.op {
                Op::Fill { path, color } => {
                    pixmap.fill_path(path, &paint_for(*color), FillRule::Winding, to_tile, mask);
                }
                Op::Stroke {
                    path,
                    color,
                    stroke,
                } => {
                    if let Ok(k) = self.cut_hairlines.binary_search(&i) {
                        if let Some(p) = &self.prepainted[k] {
                            let paint = tiny_skia::PixmapPaint::default();
                            let (x, y) = (p.x - ox, p.y - oy);
                            pixmap.draw_pixmap(
                                x,
                                y,
                                p.pixmap.as_ref(),
                                &paint,
                                Transform::identity(),
                                mask,
                            );
                        }
                        continue;
                    }
                    let stroke = &self.strokes[*stroke as usize];
                    pixmap.stroke_path(path, &paint_for(*color), stroke, to_tile, mask);
                }
                Op::Circle { cx, cy, r, color } => {
                    if let Some(path) = PathBuilder::from_circle(*cx, *cy, *r) {
                        pixmap.fill_path(
                            &path,
                            &paint_for(*color),
                            FillRule::Winding,
                            to_tile,
                            mask,
                        );
                    }
                }
                Op::Glyphs { samples, color } => {
                    for &(x, y, coverage) in samples {
                        let (lx, ly) = (x - ox, y - oy);
                        if lx >= 0 && ly >= 0 && (lx as u32) < w && (ly as u32) < h {
                            blend_pixel(pixmap, mask, lx as u32, ly as u32, *color, coverage);
                        }
                    }
                }
            }
        }
    }

    /// The anti-aliased mask of clip stack `id` over `tile`, or `None` if
    /// it is the whole tile.
    fn clip_mask(&self, id: u32, tile: &Tile, to_tile: Transform) -> Option<Mask> {
        let rects = &self.clips[id as usize];
        let (x0, y0) = tile.origin();
        let (x0, y0) = (x0 as f32, y0 as f32);
        let (x1, y1) = (
            x0 + tile.pixmap.width() as f32,
            y0 + tile.pixmap.height() as f32,
        );
        if rects
            .iter()
            .all(|r| r.left() <= x0 && r.top() <= y0 && r.right() >= x1 && r.bottom() >= y1)
        {
            return None;
        }
        let mut mask =
            Mask::new(tile.pixmap.width(), tile.pixmap.height()).expect("nonzero mask size");
        mask.data_mut().fill(255);
        for r in rects {
            mask.intersect_path(
                &PathBuilder::from_rect(*r),
                FillRule::Winding,
                true,
                to_tile,
            );
        }
        Some(mask)
    }
}

fn paint_for(color: Color) -> Paint<'static> {
    let mut paint = Paint::default();
    paint.set_color(to_sk_color(color));
    paint.anti_alias = true;
    paint
}

/// Blend one glyph coverage sample into `pixmap` at `(x, y)` (in range).
fn blend_pixel(
    pixmap: &mut Pixmap,
    mask: Option<&Mask>,
    x: u32,
    y: u32,
    color: Color,
    coverage: f32,
) {
    let idx = y as usize * pixmap.width() as usize + x as usize;
    let mut cov = coverage.clamp(0.0, 1.0);
    if let Some(mask) = mask {
        cov *= mask.data()[idx] as f32 / 255.0;
    }
    if cov <= 0.0 {
        return;
    }
    let src_a = cov * (color.a as f32 / 255.0);
    if src_a <= 0.0 {
        return;
    }
    let pixels = pixmap.pixels_mut();
    let dst = pixels[idx];
    let inv = 1.0 - src_a;
    let src_r = color.r as f32 / 255.0 * src_a;
    let src_g = color.g as f32 / 255.0 * src_a;
    let src_b = color.b as f32 / 255.0 * src_a;
    let dst_a = dst.alpha() as f32 / 255.0;
    let dst_r = dst.red() as f32 / 255.0;
    let dst_g = dst.green() as f32 / 255.0;
    let dst_b = dst.blue() as f32 / 255.0;
    let out_a = (src_a + dst_a * inv).clamp(0.0, 1.0);
    let to_u8 = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    let a8 = to_u8(out_a);
    let r8 = to_u8(src_r + dst_r * inv).min(a8);
    let g8 = to_u8(src_g + dst_g * inv).min(a8);
    let b8 = to_u8(src_b + dst_b * inv).min(a8);
    pixels[idx] = PremultipliedColorU8::from_rgba(r8, g8, b8, a8)
        .unwrap_or(PremultipliedColorU8::TRANSPARENT);
}

fn to_sk_color(c: Color) -> tiny_skia::Color {
    tiny_skia::Color::from_rgba8(c.r, c.g, c.b, c.a)
}

impl Backend for RasterBackend {
    fn size_inches(&self) -> (f64, f64) {
        (self.width_in, self.height_in)
    }

    fn device_unit_inches(&self) -> f64 {
        1.0 / self.dpi
    }

    fn push_clip(&mut self, rect: Rect) {
        let (x0, y1) = self.to_px(Point::new(rect.x0, rect.y1));
        let (x1, y0) = self.to_px(Point::new(rect.x1, rect.y0));
        let sk_rect = SkRect::from_ltrb(x0.min(x1), y1.min(y0), x0.max(x1), y1.max(y0))
            .unwrap_or(SkRect::from_ltrb(0.0, 0.0, 1.0, 1.0).unwrap());
        self.clip_stack.push(sk_rect);
        self.clip_id = match self.clips.iter().position(|c| *c == self.clip_stack) {
            Some(id) => id as u32,
            None => {
                self.clips.push(self.clip_stack.clone());
                (self.clips.len() - 1) as u32
            }
        };
    }

    fn pop_clip(&mut self) {
        self.clip_stack.pop();
        self.clip_id = self
            .clips
            .iter()
            .position(|c| *c == self.clip_stack)
            .expect("every clip stack prefix was pushed before") as u32;
    }

    fn draw_line(&mut self, p0: Point, p1: Point, stroke: &Stroke) {
        if let Some(path) = self.path_from_points(&[p0, p1], false) {
            self.stroke_shape(path, stroke);
        }
    }

    fn draw_polyline(&mut self, points: &[Point], stroke: &Stroke) {
        if points.len() < 2 {
            return;
        }
        if let Some(path) = self.path_from_points(points, false) {
            self.stroke_shape(path, stroke);
        }
    }

    fn draw_polygon(&mut self, points: &[Point], fill: Option<Color>, stroke: Option<&Stroke>) {
        if points.len() < 2 {
            return;
        }
        if let Some(path) = self.path_from_points(points, true) {
            match (fill, stroke) {
                (Some(c), Some(s)) => {
                    self.fill_shape(path.clone(), c);
                    self.stroke_shape(path, s);
                }
                (Some(c), None) => self.fill_shape(path, c),
                (None, Some(s)) => self.stroke_shape(path, s),
                (None, None) => {}
            }
        }
    }

    fn draw_rect(&mut self, rect: Rect, fill: Option<Color>, stroke: Option<&Stroke>) {
        let pts = [
            Point::new(rect.x0, rect.y0),
            Point::new(rect.x1, rect.y0),
            Point::new(rect.x1, rect.y1),
            Point::new(rect.x0, rect.y1),
        ];
        self.draw_polygon(&pts, fill, stroke);
    }

    fn draw_circle(
        &mut self,
        center: Point,
        radius: f64,
        fill: Option<Color>,
        stroke: Option<&Stroke>,
    ) {
        let (cx, cy) = self.to_px(center);
        // An opaque circle filled and stroked (solid) in one colour has the
        // same ink as a fill half a line width larger. Stroking is slow, so
        // paint that instead: plots of millions of pch 19/20 points are
        // ~2.4x faster.
        if let (Some(c), Some(s)) = (fill, stroke) {
            if c == s.color && c.a == 255 && matches!(s.lty, LineType::Pattern(p) if p.is_empty()) {
                let r = ((radius + s.width_pt() / 72.0 / 2.0) * self.dpi) as f32;
                self.fill_circle(cx, cy, r.max(0.01), c);
                return;
            }
        }
        let r = (radius * self.dpi) as f32;
        if let Some(c) = fill {
            self.fill_circle(cx, cy, r.max(0.01), c);
        }
        if let Some(s) = stroke {
            if let Some(path) = PathBuilder::from_circle(cx, cy, r.max(0.01)) {
                self.stroke_shape(path, s);
            }
        }
    }

    fn draw_text(&mut self, origin: Point, text: &str, style: &TextStyle) {
        if text.is_empty() {
            return;
        }
        let font = font_for(style.face);
        let px_per_em = style.size_pt as f32 * (self.dpi as f32 / 72.0);
        let units_per_em = font.units_per_em().unwrap_or(1000.0);
        let height = font.height_unscaled();
        let scale = ab_glyph::PxScale::from(px_per_em * height / units_per_em);

        // Cumulative x-offsets from R's Helvetica AFM tables, in inches -
        // this is what actually determines each glyph's position, so
        // multi-character strings line up exactly where the SVG/PDF
        // backends would place them.
        let offsets = afm::char_x_offsets_inches(text, style.face, style.size_pt, 1.0);
        let theta = style.rotation_deg.to_radians();
        let (cos_t, sin_t) = (theta.cos(), theta.sin());
        let dpi = self.dpi;
        let height_px = self.height_in * dpi;

        let mut samples: Vec<(i32, i32, f32)> = Vec::new();
        for (i, ch) in text.chars().enumerate() {
            let dx_in = offsets[i];
            // Anchor for this glyph, in inches, rotated about `origin`.
            let rx = dx_in * cos_t;
            let ry = dx_in * sin_t;
            let anchor_in = Point::new(origin.x + rx, origin.y + ry);
            let (anchor_px_x, anchor_px_y) = self.to_px(anchor_in);

            let glyph_id = font.glyph_id(ch);

            let glyph: AbGlyph = glyph_id.with_scale_and_position(scale, ab_glyph::point(0.0, 0.0));
            let Some(outlined) = font.outline_glyph(glyph) else {
                continue;
            };
            let bounds = outlined.px_bounds();

            outlined.draw(|gx, gy, coverage| {
                if coverage <= 0.0 {
                    return;
                }
                let lx = bounds.min.x as i32 + gx as i32;
                let ly = bounds.min.y as i32 + gy as i32;
                // Local unrotated pixel offset from the glyph run's pen
                // origin (which sits at `anchor_in`/`(anchor_px_x,
                // anchor_px_y)`), converted to an inches-space, y-up
                // delta, rotated the same way every other primitive in
                // this crate is (CCW about the anchor), then converted
                // back to final pixel space.
                let delta_x_in = lx as f64 / dpi;
                let delta_y_in = -(ly as f64) / dpi;
                let rot_x = delta_x_in * cos_t - delta_y_in * sin_t;
                let rot_y = delta_x_in * sin_t + delta_y_in * cos_t;
                let final_px_x = anchor_px_x as f64 + rot_x * dpi;
                let final_px_y = height_px - ((height_px - anchor_px_y as f64) + rot_y * dpi);
                samples.push((
                    final_px_x.round() as i32,
                    final_px_y.round() as i32,
                    coverage,
                ));
            });
        }
        if samples.is_empty() {
            return;
        }
        let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
        for &(x, y, _) in &samples {
            (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
        }
        let bounds = (x0 as f32, y0 as f32, x1 as f32 + 1.0, y1 as f32 + 1.0);
        self.record(
            Op::Glyphs {
                samples: samples.into_boxed_slice(),
                color: style.color,
            },
            bounds,
        );
    }
}
