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

use std::sync::OnceLock;

use ab_glyph::{Font, FontArc, Glyph as AbGlyph};
use tiny_skia::{
    FillRule, Mask, Paint, Path, PathBuilder, Pixmap, Rect as SkRect, Stroke as SkStroke, Transform,
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

pub struct RasterBackend {
    width_in: f64,
    height_in: f64,
    dpi: f64,
    pixmap: Pixmap,
    clip_masks: Vec<Mask>,
}

impl RasterBackend {
    /// `dpi` is pixels-per-inch (R's `png()` default is `72` for its
    /// pixel-dimension convention, i.e. `width_px = width_in * 72` unless
    /// `res=` is overridden; pass a higher `dpi` for sharper output).
    pub fn new(width_in: f64, height_in: f64, dpi: f64, background: Color) -> Self {
        let w_px = (width_in * dpi).round().max(1.0) as u32;
        let h_px = (height_in * dpi).round().max(1.0) as u32;
        let mut pixmap = Pixmap::new(w_px, h_px).expect("nonzero pixmap size");
        pixmap.fill(to_sk_color(background));
        RasterBackend {
            width_in,
            height_in,
            dpi,
            pixmap,
            clip_masks: Vec::new(),
        }
    }

    fn width_px(&self) -> u32 {
        self.pixmap.width()
    }

    fn height_px(&self) -> u32 {
        self.pixmap.height()
    }

    fn to_px(&self, p: Point) -> (f32, f32) {
        (
            (p.x * self.dpi) as f32,
            (self.height_in * self.dpi - p.y * self.dpi) as f32,
        )
    }

    fn current_mask(&self) -> Option<&Mask> {
        self.clip_masks.last()
    }

    fn full_mask(&self) -> Mask {
        let mut m = Mask::new(self.width_px(), self.height_px()).expect("nonzero mask size");
        m.data_mut().fill(255);
        m
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

    fn paint_for(color: Color) -> Paint<'static> {
        let mut paint = Paint::default();
        paint.set_color(to_sk_color(color));
        paint.anti_alias = true;
        paint
    }

    fn stroke_shape(&mut self, path: &Path, stroke: &Stroke) {
        if matches!(stroke.lty, LineType::Blank) {
            return;
        }
        let paint = Self::paint_for(stroke.color);
        let sk_stroke = Self::tiny_skia_stroke(stroke, self.dpi);
        let mask = self.current_mask().cloned();
        self.pixmap.stroke_path(
            path,
            &paint,
            &sk_stroke,
            Transform::identity(),
            mask.as_ref(),
        );
    }

    fn fill_shape(&mut self, path: &Path, fill: Color) {
        let paint = Self::paint_for(fill);
        let mask = self.current_mask().cloned();
        self.pixmap.fill_path(
            path,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            mask.as_ref(),
        );
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

    /// Blend a single glyph-rasterizer coverage sample into the pixmap at
    /// `(px, py)` (pixel coordinates, may be fractional/out-of-range -
    /// callers should have already floored/bounds-checked).
    fn blend_pixel(&mut self, x: i32, y: i32, color: Color, coverage: f32) {
        if x < 0 || y < 0 || x as u32 >= self.width_px() || y as u32 >= self.height_px() {
            return;
        }
        let mut cov = coverage.clamp(0.0, 1.0);
        if let Some(mask) = self.current_mask() {
            let idx = y as usize * mask.width() as usize + x as usize;
            let m = mask.data().get(idx).copied().unwrap_or(0);
            cov *= m as f32 / 255.0;
        }
        if cov <= 0.0 {
            return;
        }
        let idx = y as usize * self.width_px() as usize + x as usize;
        let src_a = cov * (color.a as f32 / 255.0);
        if src_a <= 0.0 {
            return;
        }
        let pixels = self.pixmap.pixels_mut();
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
        pixels[idx] = tiny_skia::PremultipliedColorU8::from_rgba(r8, g8, b8, a8)
            .unwrap_or(tiny_skia::PremultipliedColorU8::TRANSPARENT);
    }

    /// Encode the current canvas as PNG bytes.
    pub fn finish(self) -> Vec<u8> {
        self.pixmap.encode_png().expect("PNG encoding failed")
    }
}

fn to_sk_color(c: Color) -> tiny_skia::Color {
    tiny_skia::Color::from_rgba8(c.r, c.g, c.b, c.a)
}

impl Backend for RasterBackend {
    fn size_inches(&self) -> (f64, f64) {
        (self.width_in, self.height_in)
    }

    fn push_clip(&mut self, rect: Rect) {
        let (x0, y1) = self.to_px(Point::new(rect.x0, rect.y1));
        let (x1, y0) = self.to_px(Point::new(rect.x1, rect.y0));
        let sk_rect = SkRect::from_ltrb(x0.min(x1), y1.min(y0), x0.max(x1), y1.max(y0))
            .unwrap_or(SkRect::from_ltrb(0.0, 0.0, 1.0, 1.0).unwrap());
        let path = PathBuilder::from_rect(sk_rect);
        let mut mask = self
            .clip_masks
            .last()
            .cloned()
            .unwrap_or_else(|| self.full_mask());
        mask.intersect_path(&path, FillRule::Winding, true, Transform::identity());
        self.clip_masks.push(mask);
    }

    fn pop_clip(&mut self) {
        self.clip_masks.pop();
    }

    fn draw_line(&mut self, p0: Point, p1: Point, stroke: &Stroke) {
        if let Some(path) = self.path_from_points(&[p0, p1], false) {
            self.stroke_shape(&path, stroke);
        }
    }

    fn draw_polyline(&mut self, points: &[Point], stroke: &Stroke) {
        if points.len() < 2 {
            return;
        }
        if let Some(path) = self.path_from_points(points, false) {
            self.stroke_shape(&path, stroke);
        }
    }

    fn draw_polygon(&mut self, points: &[Point], fill: Option<Color>, stroke: Option<&Stroke>) {
        if points.len() < 2 {
            return;
        }
        if let Some(path) = self.path_from_points(points, true) {
            if let Some(c) = fill {
                self.fill_shape(&path, c);
            }
            if let Some(s) = stroke {
                self.stroke_shape(&path, s);
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
        let r = (radius * self.dpi) as f32;
        if let Some(path) = PathBuilder::from_circle(cx, cy, r.max(0.01)) {
            if let Some(c) = fill {
                self.fill_shape(&path, c);
            }
            if let Some(s) = stroke {
                self.stroke_shape(&path, s);
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
        let (cos_t, sin_t) = (theta.cos() as f64, theta.sin() as f64);
        let dpi = self.dpi;
        let height_px = self.height_in * dpi;

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
            let color = style.color;

            let mut samples: Vec<(i32, i32, f32)> = Vec::new();
            outlined.draw(|gx, gy, coverage| {
                if coverage > 0.0 {
                    samples.push((
                        bounds.min.x as i32 + gx as i32,
                        bounds.min.y as i32 + gy as i32,
                        coverage,
                    ));
                }
            });

            for (lx, ly, coverage) in samples {
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
                self.blend_pixel(
                    final_px_x.round() as i32,
                    final_px_y.round() as i32,
                    color,
                    coverage,
                );
            }
        }
    }
}
