// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Compression for the vector backends' output: zlib for PDF content
//! streams (on by default), gzip for opt-in `.svgz`. Big scatter plots
//! produce hundreds of megabytes of markup, so pieces are deflated on all
//! cores.

/// Deflate level: nearly all of level 6's size gain at a fraction of the
/// time (on PDF content, 4.2x vs 4.5x smaller, in a tenth of the time).
pub(crate) const DEFLATE_LEVEL: u8 = 2;

/// Data is deflated in pieces of this many bytes, on separate threads.
const DEFLATE_CHUNK: usize = 1 << 22;

/// Threads to deflate `len` bytes with.
fn threads_for(len: usize) -> usize {
    std::thread::available_parallelism()
        .map_or(1, |n| n.get())
        .min(len.div_ceil(DEFLATE_CHUNK))
}

/// Raw deflate of `data`, with `checksum(data)` computed on this thread
/// meanwhile. Like `pigz`, pieces of [`DEFLATE_CHUNK`] bytes are deflated on
/// separate threads, each but the last ending in a sync flush (an empty
/// stored block, so it ends on a byte boundary and isn't final), so their
/// raw deflate streams concatenate into one. Matches cost only the
/// back-references across piece boundaries, a negligible size difference.
fn deflate_raw(data: &[u8], level: u8, checksum: impl FnOnce() -> u32) -> (Vec<u8>, u32) {
    use miniz_oxide::deflate::core::{
        CompressorOxide, TDEFLFlush, TDEFLStatus, compress, create_comp_flags_from_zip_params,
    };
    let threads = threads_for(data.len());
    if threads <= 1 {
        return (
            miniz_oxide::deflate::compress_to_vec(data, level),
            checksum(),
        );
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
    std::thread::scope(|s| {
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
        let sum = checksum();
        let mut out = Vec::new();
        for h in handles {
            for piece in h.join().expect("deflate thread panicked") {
                out.extend_from_slice(&piece);
            }
        }
        (out, sum)
    })
}

/// zlib-compress `data` (see [`deflate_raw`]).
pub(crate) fn zlib_compress(data: &[u8], level: u8) -> Vec<u8> {
    if threads_for(data.len()) <= 1 {
        return miniz_oxide::deflate::compress_to_vec_zlib(data, level);
    }
    let (deflated, adler) = deflate_raw(data, level, || adler2::adler32_slice(data));
    // zlib header: deflate with a 32K window, no preset dictionary; the
    // level bits are only informative. (0x789C % 31 == 0, as required.)
    let mut out = Vec::with_capacity(deflated.len() + 6);
    out.extend_from_slice(&[0x78, 0x9C]);
    out.extend_from_slice(&deflated);
    out.extend_from_slice(&adler.to_be_bytes());
    out
}

/// gzip-compress `data` (see [`deflate_raw`]), as for a `.svgz` file.
pub(crate) fn gzip_compress(data: &[u8], level: u8) -> Vec<u8> {
    let (deflated, crc) = deflate_raw(data, level, || crc32fast::hash(data));
    // RFC 1952 header: magic, deflate, no flags, no modification time, no
    // extra flags, unknown OS.
    let mut out = Vec::with_capacity(deflated.len() + 18);
    out.extend_from_slice(&[0x1F, 0x8B, 8, 0, 0, 0, 0, 0, 0, 255]);
    out.extend_from_slice(&deflated);
    out.extend_from_slice(&crc.to_le_bytes());
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drawing-operator-like text, several pieces long; tested ending
    /// mid-piece, on a piece boundary, one byte past it, and short.
    fn sample() -> Vec<u8> {
        let line = b"123.456 78.900 2.700 0 360 re f\n";
        line.iter()
            .copied()
            .cycle()
            .take(3 * DEFLATE_CHUNK + 12345)
            .collect()
    }

    const LENGTHS: [usize; 4] = [
        3 * DEFLATE_CHUNK + 12345,
        2 * DEFLATE_CHUNK,
        2 * DEFLATE_CHUNK + 1,
        100,
    ];

    #[test]
    fn zlib_compress_in_pieces_round_trips() {
        let text = sample();
        for len in LENGTHS {
            let data = &text[..len];
            let z = zlib_compress(data, DEFLATE_LEVEL);
            assert!(z.len() < data.len());
            let back = miniz_oxide::inflate::decompress_to_vec_zlib(&z).expect("valid zlib");
            assert!(back == data, "length {len}");
        }
    }

    #[test]
    fn gzip_compress_in_pieces_round_trips() {
        let text = sample();
        for len in LENGTHS {
            let data = &text[..len];
            let gz = gzip_compress(data, DEFLATE_LEVEL);
            assert!(gz.len() < data.len());
            assert_eq!(&gz[..10], &[0x1F, 0x8B, 8, 0, 0, 0, 0, 0, 0, 255]);
            let (body, trailer) = gz[10..].split_at(gz.len() - 18);
            let back = miniz_oxide::inflate::decompress_to_vec(body).expect("valid deflate");
            assert!(back == data, "length {len}");
            assert_eq!(&trailer[..4], &crc32fast::hash(data).to_le_bytes());
            assert_eq!(&trailer[4..], &(len as u32).to_le_bytes());
        }
    }
}
