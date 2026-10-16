//! Minimal hand-rolled single-page PDF writer, using the 4 standard
//! Type1 Helvetica faces (`Helvetica`, `-Bold`, `-Oblique`,
//! `-BoldOblique`) that every PDF viewer/renderer bundles - so, like R's
//! own `pdf()` device default, no font embedding is needed at all, and
//! text metrics/rendering come from the exact same Adobe AFM data this
//! crate already uses for layout (`crate::afm`).
//!
//! PDF's native page coordinate system is already "inches (well, points)
//! from the bottom-left, y-up", matching this crate's internal `Backend`
//! convention exactly (see `backend/mod.rs`), so this backend applies no
//! coordinate flip - only a inches-to-points scale (`1 in = 72 pt`).
//!
//! Like R's `pdf()` (`compress = TRUE`), the page's content stream is
//! Flate-compressed by default; see [`PdfBackend::with_compression`].

use std::fmt::Write as _;

use super::{Backend, LineEnd, LineJoin, Stroke, TextStyle};
use crate::afm::FontFace;
use crate::color::Color;
use crate::geometry::{Point, Rect};
use crate::lty::LineType;

pub struct PdfBackend {
    width_in: f64,
    height_in: f64,
    content: String,
    clip_depth: u32,
    last_fill: Option<Color>,
    last_stroke_color: Option<Color>,
    compress: bool,
}

impl PdfBackend {
    pub fn new(width_in: f64, height_in: f64) -> Self {
        PdfBackend {
            width_in,
            height_in,
            content: String::new(),
            clip_depth: 0,
            last_fill: None,
            last_stroke_color: None,
            compress: true,
        }
    }

    /// Whether to Flate-compress the content stream (default `true`), like
    /// R's `pdf(compress = )`. Uncompressed output is much larger but its
    /// drawing operators can be read as text, as the tests comparing it
    /// with R's `pdf(compress = FALSE)` do.
    pub fn with_compression(mut self, compress: bool) -> Self {
        self.compress = compress;
        self
    }

    fn pt(&self, p: Point) -> (f64, f64) {
        (p.x * 72.0, p.y * 72.0)
    }

    /// Write the operator line `nums op`, each number with `prec` decimals.
    fn op(&mut self, nums: &[f64], prec: usize, op: &str) {
        for &v in nums {
            push_fixed(&mut self.content, v, prec);
            self.content.push(' ');
        }
        self.content.push_str(op);
        self.content.push('\n');
    }

    fn set_fill(&mut self, c: Color) {
        if self.last_fill != Some(c) {
            let rgb = [c.r, c.g, c.b].map(|v| v as f64 / 255.0);
            self.op(&rgb, 4, "rg");
            self.last_fill = Some(c);
        }
    }

    fn set_stroke_color(&mut self, c: Color) {
        if self.last_stroke_color != Some(c) {
            let rgb = [c.r, c.g, c.b].map(|v| v as f64 / 255.0);
            self.op(&rgb, 4, "RG");
            self.last_stroke_color = Some(c);
        }
    }

    fn set_stroke_style(&mut self, stroke: &Stroke) {
        self.set_stroke_color(stroke.color);
        self.op(&[stroke.width_pt()], 3, "w");
        let cap = match stroke.lend {
            LineEnd::Butt => 0,
            LineEnd::Round => 1,
            LineEnd::Square => 2,
        };
        let join = match stroke.ljoin {
            LineJoin::Mitre => 0,
            LineJoin::Round => 1,
            LineJoin::Bevel => 2,
        };
        let _ = write!(self.content, "{cap} J {join} j ");
        self.op(&[stroke.lmitre], 2, "M");
        match stroke
            .lty
            .dash_lengths_pt(stroke.lwd, stroke.lend == LineEnd::Round)
        {
            Some(dashes) => {
                self.content.push('[');
                for (i, &d) in dashes.iter().enumerate() {
                    if i > 0 {
                        self.content.push(' ');
                    }
                    push_fixed(&mut self.content, d, 3);
                }
                self.content.push_str("] 0 d\n");
            }
            None => {
                let _ = writeln!(self.content, "[] 0 d");
            }
        }
    }

    fn path_moveto_lineto(&mut self, points: &[Point], close: bool) {
        for (i, p) in points.iter().enumerate() {
            let (x, y) = self.pt(*p);
            self.op(&[x, y], 3, if i == 0 { "m" } else { "l" });
        }
        if close {
            self.content.push_str("h\n");
        }
    }

    fn paint_path(&mut self, fill: Option<Color>, stroke: Option<&Stroke>) {
        let do_fill = fill.is_some();
        let do_stroke = stroke.map_or(false, |s| !matches!(s.lty, LineType::Blank));
        if let Some(c) = fill {
            self.set_fill(c);
        }
        if let Some(s) = stroke {
            if do_stroke {
                self.set_stroke_style(s);
            }
        }
        let op = match (do_fill, do_stroke) {
            (true, true) => "B",
            (true, false) => "f",
            (false, true) => "S",
            (false, false) => "n",
        };
        self.content.push_str(op);
        self.content.push('\n');
    }

    /// Approximate a circle with 4 cubic Bezier arcs (the standard
    /// kappa = 0.5522847498 magic constant).
    fn circle_path(&mut self, center: Point, radius: f64) {
        const K: f64 = 0.552_284_749_8;
        let (cx, cy) = self.pt(center);
        let r = radius * 72.0;
        let k = K * r;
        self.op(&[cx + r, cy], 3, "m");
        self.op(&[cx + r, cy + k, cx + k, cy + r, cx, cy + r], 3, "c");
        self.op(&[cx - k, cy + r, cx - r, cy + k, cx - r, cy], 3, "c");
        self.op(&[cx - r, cy - k, cx - k, cy - r, cx, cy - r], 3, "c");
        self.op(&[cx + k, cy - r, cx + r, cy - k, cx + r, cy], 3, "c");
        self.content.push_str("h\n");
    }

    fn font_resource_name(face: FontFace) -> &'static str {
        match face {
            FontFace::Plain => "/F1",
            FontFace::Bold => "/F2",
            FontFace::Italic => "/F3",
            FontFace::BoldItalic => "/F4",
        }
    }

    fn base_font_name(face: FontFace) -> &'static str {
        match face {
            FontFace::Plain => "Helvetica",
            FontFace::Bold => "Helvetica-Bold",
            FontFace::Italic => "Helvetica-Oblique",
            FontFace::BoldItalic => "Helvetica-BoldOblique",
        }
    }

    /// A PDF string literal body for `s` in the fonts' WinAnsiEncoding.
    /// Like R's `pdf()` (whose default ISOLatin1 encoding agrees with WinAnsi
    /// above 0x9F), Latin-1 characters are written as their single byte, as
    /// an octal escape so the content stream stays ASCII. Characters with no
    /// byte in the encoding are drawn as `?`.
    fn escape_pdf_string(s: &str) -> String {
        let mut out = String::with_capacity(s.len());
        for c in s.chars() {
            match c {
                '(' => out.push_str("\\("),
                ')' => out.push_str("\\)"),
                '\\' => out.push_str("\\\\"),
                ' '..='~' => out.push(c),
                '\u{a0}'..='\u{ff}' => out.push_str(&format!("\\{:03o}", c as u32)),
                _ => out.push('?'),
            }
        }
        out
    }

    /// Serialize the completed single-page PDF as bytes.
    pub fn finish(self) -> Vec<u8> {
        let w_pt = self.width_in * 72.0;
        let h_pt = self.height_in * 72.0;

        let mut objects: Vec<Vec<u8>> = Vec::new();
        // 1: Catalog, 2: Pages, 3: Page, 4: Contents, 5-8: Fonts.
        objects.push(b"<< /Type /Catalog /Pages 2 0 R >>".to_vec());
        objects.push(b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec());
        objects.push(
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {w_pt:.3} {h_pt:.3}] \
                 /Resources << /Font << /F1 5 0 R /F2 6 0 R /F3 7 0 R /F4 8 0 R >> >> \
                 /Contents 4 0 R >>"
            )
            .into_bytes(),
        );
        let (stream, filter) = if self.compress {
            let data = zlib_compress(self.content.as_bytes(), DEFLATE_LEVEL);
            (data, " /Filter /FlateDecode")
        } else {
            (self.content.into_bytes(), "")
        };
        let mut contents = format!("<< /Length {}{filter} >>\nstream\n", stream.len()).into_bytes();
        contents.extend_from_slice(&stream);
        // Flate data may not end in a newline; a separator added before
        // `endstream` isn't counted in `/Length`.
        if stream.last() != Some(&b'\n') {
            contents.push(b'\n');
        }
        contents.extend_from_slice(b"endstream");
        objects.push(contents);
        for face in [
            FontFace::Plain,
            FontFace::Bold,
            FontFace::Italic,
            FontFace::BoldItalic,
        ] {
            objects.push(
                format!(
                    "<< /Type /Font /Subtype /Type1 /BaseFont /{} /Encoding /WinAnsiEncoding >>",
                    Self::base_font_name(face)
                )
                .into_bytes(),
            );
        }

        let mut out = Vec::new();
        out.extend_from_slice(b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n");
        let mut offsets = Vec::with_capacity(objects.len());
        for (i, obj) in objects.iter().enumerate() {
            offsets.push(out.len());
            let header = format!("{} 0 obj\n", i + 1);
            out.extend_from_slice(header.as_bytes());
            out.extend_from_slice(obj);
            out.extend_from_slice(b"\nendobj\n");
        }
        let xref_offset = out.len();
        let n = objects.len() + 1;
        out.extend_from_slice(format!("xref\n0 {n}\n").as_bytes());
        out.extend_from_slice(b"0000000000 65535 f \n");
        for off in &offsets {
            out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
        }
        out.extend_from_slice(
            format!("trailer\n<< /Size {n} /Root 1 0 R >>\nstartxref\n{xref_offset}\n%%EOF\n")
                .as_bytes(),
        );
        out
    }
}

/// Append `v` with `prec` decimals (at most 6) to `out`, exactly as
/// `write!(out, "{v:.prec$}")` would, but several times faster: the
/// coordinates of big scatter plots are most of a PDF's content.
///
/// Rust's formatting rounds `v`'s exact binary value to `prec` decimals
/// (ties to even). Here `|v| * 10^prec` is rounded to an integer instead,
/// which agrees unless that product is within its own rounding error of a
/// half; such values, and ones too large to scale accurately, NaN and
/// infinities, are left to `write!`.
fn push_fixed(out: &mut String, v: f64, prec: usize) {
    const SCALE: [f64; 7] = [1.0, 1e1, 1e2, 1e3, 1e4, 1e5, 1e6];
    // Below 2^31 the product's rounding error is at most 2^-22, well inside
    // the margin around a half.
    const LIMIT: f64 = 2_147_483_648.0;
    const MARGIN: f64 = 1e-6;
    let scaled = v.abs() * SCALE[prec];
    if scaled.is_nan() || scaled >= LIMIT || (scaled - scaled.floor() - 0.5).abs() < MARGIN {
        let _ = write!(out, "{v:.prec$}");
        return;
    }
    let n = scaled.round() as u64;
    // Like Rust's formatting, the sign is kept for -0.0 and for negative
    // values that round to zero.
    if v.is_sign_negative() {
        out.push('-');
    }
    // Digits from the right: `prec` decimals, the point, then the whole part.
    let mut digits = [0u8; 24];
    let mut at = digits.len();
    let mut rest = n;
    for i in 0.. {
        if i == prec && prec > 0 {
            at -= 1;
            digits[at] = b'.';
        }
        at -= 1;
        digits[at] = b'0' + (rest % 10) as u8;
        rest /= 10;
        if rest == 0 && i >= prec {
            break;
        }
    }
    out.push_str(std::str::from_utf8(&digits[at..]).expect("ASCII digits"));
}

/// Deflate level for compressed content streams.
const DEFLATE_LEVEL: u8 = 2;

/// Content is deflated in pieces of this many bytes, on separate threads.
const DEFLATE_CHUNK: usize = 1 << 22;

/// zlib-compress `data`. Like `pigz`, pieces of [`DEFLATE_CHUNK`] bytes are
/// deflated on separate threads, each but the last ending in a sync flush
/// (an empty stored block, so it ends on a byte boundary and isn't final),
/// so their raw deflate streams concatenate into one. Matches cost only the
/// back-references across piece boundaries, a negligible size difference.
fn zlib_compress(data: &[u8], level: u8) -> Vec<u8> {
    use miniz_oxide::deflate::core::{
        compress, create_comp_flags_from_zip_params, CompressorOxide, TDEFLFlush, TDEFLStatus,
    };
    let threads = std::thread::available_parallelism()
        .map_or(1, |n| n.get())
        .min(data.len().div_ceil(DEFLATE_CHUNK));
    if threads <= 1 {
        return miniz_oxide::deflate::compress_to_vec_zlib(data, level);
    }
    let pieces: Vec<&[u8]> = data.chunks(DEFLATE_CHUNK).collect();
    let deflate = |piece: &[u8], last: bool| -> Vec<u8> {
        // Negative window bits: raw deflate, no zlib header or checksum.
        let flags = create_comp_flags_from_zip_params(level.into(), -15, 0);
        let mut compressor = CompressorOxide::new(flags);
        let flush = if last {
            TDEFLFlush::Finish
        } else {
            TDEFLFlush::Sync
        };
        let mut out = vec![0; piece.len() / 2 + 1024];
        let (mut input, mut out_pos) = (piece, 0);
        loop {
            let (status, n_in, n_out) =
                compress(&mut compressor, input, &mut out[out_pos..], flush);
            input = &input[n_in..];
            out_pos += n_out;
            match status {
                TDEFLStatus::Done => break,
                // As with zlib, a flush is complete once all input is taken
                // and output space is left over.
                TDEFLStatus::Okay if input.is_empty() && out_pos < out.len() && !last => break,
                TDEFLStatus::Okay => {
                    if out.len() - out_pos < 1024 {
                        out.resize(out.len() * 2, 0);
                    }
                }
                _ => panic!("deflate failed: {status:?}"),
            }
        }
        out.truncate(out_pos);
        out
    };
    let per_thread = pieces.len().div_ceil(threads);
    let n = pieces.len();
    let (deflated, adler) = std::thread::scope(|s| {
        let handles: Vec<_> = pieces
            .chunks(per_thread)
            .enumerate()
            .map(|(t, group)| {
                s.spawn(move || {
                    group
                        .iter()
                        .enumerate()
                        .map(|(k, piece)| deflate(piece, t * per_thread + k == n - 1))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        let adler = adler2::adler32_slice(data);
        let deflated: Vec<Vec<u8>> = handles
            .into_iter()
            .flat_map(|h| h.join().expect("deflate thread panicked"))
            .collect();
        (deflated, adler)
    });
    // zlib header: deflate with a 32K window, no preset dictionary; the
    // level bits are only informative. (0x789C % 31 == 0, as required.)
    let mut out = vec![0x78, 0x9C];
    out.reserve(deflated.iter().map(Vec::len).sum::<usize>() + 4);
    for piece in &deflated {
        out.extend_from_slice(piece);
    }
    out.extend_from_slice(&adler.to_be_bytes());
    out
}

impl Backend for PdfBackend {
    fn size_inches(&self) -> (f64, f64) {
        (self.width_in, self.height_in)
    }

    /// R's `pdf()` device (`PDF_Text`, `PDF_StrWidth`, `PDF_MetricInfo` in
    /// `devPS.c`) uses `floor(cex * ps + 0.5)` points for drawing *and*
    /// for all string metrics - so e.g. `cex.main = 1.2` at 12pt is set in
    /// 14pt (not 14.4pt) and laid out with 14pt widths. Text whose rounded
    /// size is 0 is not drawn at all (callers check for a size `<= 0`).
    fn effective_font_size(&self, requested_pt: f64) -> f64 {
        (requested_pt + 0.5).floor()
    }

    fn transparent_default_bg(&self) -> bool {
        true
    }

    fn push_clip(&mut self, rect: Rect) {
        self.content.push_str("q\n");
        let (x0, y0) = self.pt(Point::new(rect.x0, rect.y0));
        let w = rect.width() * 72.0;
        let h = rect.height() * 72.0;
        self.op(&[x0, y0, w, h], 3, "re W n");
        self.clip_depth += 1;
        // Restoring `q`/`Q` also resets the fill/stroke color cache
        // assumptions, since PDF graphics state (including color) is
        // saved/restored by q/Q too.
        self.last_fill = None;
        self.last_stroke_color = None;
    }

    fn pop_clip(&mut self) {
        if self.clip_depth > 0 {
            self.content.push_str("Q\n");
            self.clip_depth -= 1;
            self.last_fill = None;
            self.last_stroke_color = None;
        }
    }

    fn draw_line(&mut self, p0: Point, p1: Point, stroke: &Stroke) {
        if matches!(stroke.lty, LineType::Blank) {
            return;
        }
        self.path_moveto_lineto(&[p0, p1], false);
        self.paint_path(None, Some(stroke));
    }

    fn draw_polyline(&mut self, points: &[Point], stroke: &Stroke) {
        if matches!(stroke.lty, LineType::Blank) || points.len() < 2 {
            return;
        }
        self.path_moveto_lineto(points, false);
        self.paint_path(None, Some(stroke));
    }

    fn draw_polygon(&mut self, points: &[Point], fill: Option<Color>, stroke: Option<&Stroke>) {
        if points.len() < 2 {
            return;
        }
        self.path_moveto_lineto(points, true);
        self.paint_path(fill, stroke);
    }

    fn draw_rect(&mut self, rect: Rect, fill: Option<Color>, stroke: Option<&Stroke>) {
        let (x0, y0) = self.pt(Point::new(rect.x0, rect.y0));
        let w = rect.width() * 72.0;
        let h = rect.height() * 72.0;
        self.op(&[x0, y0, w, h], 3, "re");
        self.paint_path(fill, stroke);
    }

    fn draw_circle(
        &mut self,
        center: Point,
        radius: f64,
        fill: Option<Color>,
        stroke: Option<&Stroke>,
    ) {
        self.circle_path(center, radius);
        self.paint_path(fill, stroke);
    }

    fn draw_text(&mut self, origin: Point, text: &str, style: &TextStyle) {
        if text.is_empty() {
            return;
        }
        self.set_fill(style.color);
        let (x, y) = self.pt(origin);
        let theta = style.rotation_deg.to_radians();
        let (c, s) = (theta.cos(), theta.sin());
        self.content.push_str("BT\n");
        self.content.push_str(Self::font_resource_name(style.face));
        self.content.push(' ');
        self.op(&[style.size_pt], 2, "Tf");
        for v in [c, s, -s, c] {
            push_fixed(&mut self.content, v, 6);
            self.content.push(' ');
        }
        self.op(&[x, y], 3, "Tm");
        let _ = writeln!(self.content, "({}) Tj", Self::escape_pdf_string(text));
        self.content.push_str("ET\n");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::svg::SvgBackend;

    #[test]
    fn push_fixed_matches_rust_formatting() {
        let mut state = 0x2545_F491_4F6C_DD1Du64;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        let mut values = vec![
            0.0,
            -0.0,
            -0.0001,
            -0.0004999,
            0.0005,
            0.125,
            0.375,
            -0.125,
            2.5,
            1e20,
            -1e20,
            2_147_483_647.5,
            f64::MIN_POSITIVE,
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
        ];
        for _ in 0..200_000 {
            let r = next();
            let unit = (r >> 11) as f64 / (1u64 << 53) as f64; // [0, 1)
            let sign = if r & 1 == 0 { 1.0 } else { -1.0 };
            let scale = [1e-4, 1.0, 72.0, 1e3, 1e5, 1e9][(r >> 1) as usize % 6];
            values.push(sign * unit * scale);
            // Exact binary ties at various decimal positions, and values a
            // hair either side of a decimal half.
            let k = (r >> 20) % 100_000;
            values.push(sign * (k as f64 + 0.5) / [1.0, 2.0, 4.0, 8.0][(r >> 3) as usize % 4]);
            for prec in [2, 3, 4, 6] {
                let half = (k as f64 + 0.5) / 10f64.powi(prec);
                // The neighbouring doubles (`half` is positive).
                let (up, down) = (
                    f64::from_bits(half.to_bits() + 1),
                    f64::from_bits(half.to_bits() - 1),
                );
                values.extend([half, up, down, -half]);
            }
        }
        for &v in &values {
            for prec in [0, 2, 3, 4, 6] {
                let mut ours = String::new();
                push_fixed(&mut ours, v, prec);
                assert_eq!(ours, format!("{v:.prec$}"), "v = {v:?}, prec = {prec}");
            }
        }
    }

    #[test]
    fn zlib_compress_in_pieces_round_trips() {
        // Drawing-operator-like text, several pieces long, ending mid-piece,
        // on a piece boundary, and one byte past it.
        let line = b"123.456 78.900 2.700 0 360 re f\n";
        let text: Vec<u8> = line
            .iter()
            .copied()
            .cycle()
            .take(3 * DEFLATE_CHUNK + 12345)
            .collect();
        for len in [text.len(), 2 * DEFLATE_CHUNK, 2 * DEFLATE_CHUNK + 1, 100] {
            let data = &text[..len];
            let z = zlib_compress(data, DEFLATE_LEVEL);
            assert!(z.len() < data.len());
            let back = miniz_oxide::inflate::decompress_to_vec_zlib(&z).expect("valid zlib");
            assert!(back == data, "length {len}");
        }
    }

    #[test]
    fn pdf_rounds_font_size_like_r_but_svg_does_not() {
        let pdf = PdfBackend::new(6.0, 4.0);
        assert_eq!(pdf.effective_font_size(14.4), 14.0);
        assert_eq!(pdf.effective_font_size(10.5), 11.0);
        assert_eq!(pdf.effective_font_size(0.4), 0.0);
        let svg = SvgBackend::new(6.0, 4.0);
        assert_eq!(svg.effective_font_size(14.4), 14.4);
    }
}
