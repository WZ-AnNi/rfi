//! RFI frequency-basis prototype.
//!
//! New container signature (incompatible with the old RFI bytes):
//!   RFI\r\n\x1A\n
//! The image is represented by per-tile 8x8 DCT coefficients, quantized with a
//! scalar step, zero-run encoded and zstd-compressed. No RGBA residual section
//! exists in this format.

pub mod analysis;
mod dct16;
pub mod metrics;
pub(crate) mod rans;
pub use dct16::{decode16, encode16, encode16_to_budget, encode16_to_budget_with, encode16_with};
use image::{Rgba, RgbaImage};

pub const MAGIC: &[u8; 8] = b"RFI\r\n\x1A\n\0";
pub const VERSION_MAJOR: u16 = 1;
pub const VERSION_MINOR: u16 = 0;
pub const HEADER_SIZE: usize = 96;
pub const SECTION_ENTRY: usize = 32;
pub const TILE_RECORD: usize = 32;
pub const TILE_SIZE: u32 = 64;
pub const BLOCK: u32 = 8;
pub const BLOCKS_PER_TILE: u32 = TILE_SIZE / BLOCK;
pub const BASIS_DCT8: u16 = 1;
pub const BASIS_DCT16: u16 = 3;

pub const SECTION_BASIS: u16 = 1;
pub const SECTION_TILES: u16 = 2;
pub const SECTION_COEFF_DC: u16 = 3;
pub const SECTION_DC_CHROMA: u16 = 8;
pub const SECTION_COEFF_AC: u16 = 4;
pub const SECTION_AC_COUNT: u16 = 4;
pub const SECTION_AC_POS: u16 = 5;
pub const SECTION_AC_VAL: u16 = 6;
pub const SECTION_AC_EXT: u16 = 7;
pub const SECTION_AC_VAL_CHROMA: u16 = 9;
pub const SECTION_COEFF: u16 = SECTION_COEFF_DC;
pub const SECTION_META: u16 = 10;
pub const SECTION_TILE_Q: u16 = 11;

/// Section codec ids stored in the section table.
pub(crate) const CODEC_ZSTD: u8 = 1;
pub(crate) const CODEC_RANS: u8 = 2;
/// Frame of independently-compressed sub-streams, each with its own codec.
pub(crate) const CODEC_BUCKETED: u8 = 4;

#[derive(Debug)]
pub struct Error(pub String);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Self(value.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;

fn crc32c(data: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0x82F63B78 & mask);
        }
    }
    !crc
}

fn put_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}
fn read_u16(data: &[u8], offset: usize) -> Result<u16> {
    data.get(offset..offset + 2)
        .and_then(|b| b.try_into().ok())
        .map(u16::from_le_bytes)
        .ok_or_else(|| Error("truncated u16".into()))
}
fn read_u32(data: &[u8], offset: usize) -> Result<u32> {
    data.get(offset..offset + 4)
        .and_then(|b| b.try_into().ok())
        .map(u32::from_le_bytes)
        .ok_or_else(|| Error("truncated u32".into()))
}
fn read_u64(data: &[u8], offset: usize) -> Result<u64> {
    data.get(offset..offset + 8)
        .and_then(|b| b.try_into().ok())
        .map(u64::from_le_bytes)
        .ok_or_else(|| Error("truncated u64".into()))
}

/// Orthonormal 1D DCT-II basis, with the 0.5 factor of the 2D form folded in
/// so a separable row/column pass reproduces the direct 2D transform exactly.
fn basis1d() -> &'static [[f32; 8]; 8] {
    use std::sync::OnceLock;
    static TABLE: OnceLock<[[f32; 8]; 8]> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut table = [[0.0f32; 8]; 8];
        for (u, row) in table.iter_mut().enumerate() {
            let cu = if u == 0 {
                (1.0 / 8f32).sqrt()
            } else {
                (2.0 / 8f32).sqrt()
            };
            for (x, value) in row.iter_mut().enumerate() {
                *value = cu * (((2 * x + 1) as f32) * u as f32 * std::f32::consts::PI / 16.0).cos();
            }
        }
        table
    })
}

/// Separable 8x8 forward DCT: 8 rows then 8 columns of 1D length 8.
/// Costs 2*8*8*8 = 1024 multiply-adds instead of the direct form's 4096.
fn dct8(block: &[f32; 64]) -> [f32; 64] {
    let basis = basis1d();
    let mut tmp = [0.0f32; 64];
    // Rows: spatial x -> frequency u.
    for y in 0..8 {
        for u in 0..8 {
            let mut sum = 0.0f32;
            for x in 0..8 {
                sum += basis[u][x] * block[y * 8 + x];
            }
            tmp[y * 8 + u] = sum;
        }
    }
    let mut out = [0.0f32; 64];
    // Columns: spatial y -> frequency v.
    for u in 0..8 {
        for v in 0..8 {
            let mut sum = 0.0f32;
            for y in 0..8 {
                sum += basis[v][y] * tmp[y * 8 + u];
            }
            out[v * 8 + u] = sum;
        }
    }
    out
}

fn idct8(coeff: &[f32; 64]) -> [f32; 64] {
    let basis = basis1d();
    let mut tmp = [0.0f32; 64];
    // Columns: frequency v -> spatial y.
    for u in 0..8 {
        for y in 0..8 {
            let mut sum = 0.0f32;
            for v in 0..8 {
                sum += basis[v][y] * coeff[v * 8 + u];
            }
            tmp[y * 8 + u] = sum;
        }
    }
    let mut out = [0.0f32; 64];
    // Rows: frequency u -> spatial x.
    for y in 0..8 {
        for x in 0..8 {
            let mut sum = 0.0f32;
            for u in 0..8 {
                sum += basis[u][x] * tmp[y * 8 + u];
            }
            out[y * 8 + x] = sum;
        }
    }
    out
}

const ZIGZAG: [usize; 64] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27, 20, 13, 6, 7,
    14, 21, 28, 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51, 58, 59, 52, 45, 38, 31, 39,
    46, 53, 60, 61, 54, 47, 55, 62, 63,
];

// Fixed JPEG-style frequency weighting, normalized around 16.
const QUANT_LUMA: [u16; 64] = [
    8, 13, 12, 16, 22, 33, 41, 48, 13, 13, 15, 18, 23, 45, 47, 43, 15, 14, 16, 22, 33, 45, 53, 44, 15, 17,
    20, 25, 41, 66, 61, 48, 17, 20, 31, 44, 52, 81, 77, 59, 22, 29, 43, 50, 62, 78, 84, 69, 39, 50, 59, 66,
    77, 90, 89, 76, 55, 69, 71, 73, 83, 75, 77, 74,
];
const QUANT_CHROMA: [u16; 64] = [
    9, 18, 24, 47, 99, 99, 99, 99, 18, 21, 26, 66, 99, 99, 99, 99, 24, 26, 56, 99, 99, 99, 99, 99, 47, 66,
    99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99,
    99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99,
];

fn quantize(block: &[f32; 64], step: f32, matrix: &[u16; 64]) -> [i16; 64] {
    let mut out = [0i16; 64];
    for i in 0..64 {
        let quant_step = step * f32::from(matrix[i]) / 16.0;
        out[i] = (block[i] / quant_step).round().clamp(-32768.0, 32767.0) as i16;
    }
    out
}

fn dequantize(coeff: &[i16; 64], step: f32, matrix: &[u16; 64]) -> [f32; 64] {
    let mut out = [0.0f32; 64];
    for i in 0..64 {
        out[i] = f32::from(coeff[i]) * step * f32::from(matrix[i]) / 16.0;
    }
    out
}

fn put_uvarint(out: &mut Vec<u8>, mut value: u64) {
    while value >= 0x80 {
        out.push((value as u8) | 0x80);
        value >>= 7;
    }
    out.push(value as u8);
}

fn read_uvarint(data: &[u8], cursor: &mut usize) -> Result<u64> {
    let mut value = 0u64;
    let mut shift = 0u32;
    loop {
        let byte = *data
            .get(*cursor)
            .ok_or_else(|| Error("truncated varint".into()))?;
        *cursor += 1;
        value |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Ok(value);
        }
        shift += 7;
        if shift >= 64 {
            return Err(Error("varint overflow".into()));
        }
    }
}

fn uvarint_len(mut value: u64) -> usize {
    let mut len = 1;
    while value >= 0x80 {
        len += 1;
        value >>= 7;
    }
    len
}

fn dc_varint_len(dc: i16, previous_dc: i16) -> usize {
    uvarint_len(zigzag_i32(i32::from(dc) - i32::from(previous_dc)))
}

fn zigzag_i32(value: i32) -> u64 {
    ((value << 1) ^ (value >> 31)) as u32 as u64
}

fn unzigzag_i32(value: u64) -> i32 {
    let value = value as u32;
    ((value >> 1) as i32) ^ -((value & 1) as i32)
}

fn zigzag_i16(value: i16) -> u64 {
    (((i32::from(value) << 1) ^ (i32::from(value) >> 31)) as u32) as u64
}

fn unzigzag_i16(value: u64) -> i16 {
    let value = value as u32;
    (((value >> 1) as i32) ^ -((value & 1) as i32)) as i16
}

/// JPEG-LS MED predictor over the DC block grid.
fn med_predict(left: Option<i16>, top: Option<i16>, top_left: Option<i16>) -> i16 {
    match (left, top) {
        (None, None) => top_left.unwrap_or(0),
        (Some(left), None) => left,
        (None, Some(top)) => top,
        (Some(left), Some(top)) => {
            let top_left = top_left.unwrap_or(0);
            let max = left.max(top);
            let min = left.min(top);
            if top_left >= max {
                min
            } else if top_left <= min {
                max
            } else {
                left + top - top_left
            }
        }
    }
}

/// Buckets used when splitting the AC run stream by the previous zigzag position.
const POS_BUCKETS: usize = 8;

fn pos_bucket(previous: usize) -> usize {
    (previous / POS_BUCKETS).min(POS_BUCKETS - 1)
}

/// Frames `n` independently compressed sub-streams so each may pick its own codec.
fn pack_buckets(buckets: &[Vec<u8>], entropy: Entropy) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    out.push(buckets.len() as u8);
    for bucket in buckets {
        let (stored, codec) = pack_stream(bucket, entropy)?;
        out.push(codec);
        out.extend_from_slice(&(bucket.len() as u32).to_le_bytes());
        out.extend_from_slice(&(stored.len() as u32).to_le_bytes());
        out.extend_from_slice(&stored);
    }
    Ok(out)
}

fn unpack_buckets(data: &[u8]) -> Result<Vec<Vec<u8>>> {
    if data.is_empty() {
        return Err(Error("empty bucket frame".into()));
    }
    let count = data[0] as usize;
    let mut cursor = 1usize;
    let mut buckets = Vec::with_capacity(count);
    for _ in 0..count {
        if cursor + 9 > data.len() {
            return Err(Error("truncated bucket header".into()));
        }
        let codec = data[cursor];
        let raw = u32::from_le_bytes([
            data[cursor + 1],
            data[cursor + 2],
            data[cursor + 3],
            data[cursor + 4],
        ]) as usize;
        let stored_len = u32::from_le_bytes([
            data[cursor + 5],
            data[cursor + 6],
            data[cursor + 7],
            data[cursor + 8],
        ]) as usize;
        cursor += 9;
        if cursor + stored_len > data.len() {
            return Err(Error("bucket outside frame".into()));
        }
        let payload = &data[cursor..cursor + stored_len];
        cursor += stored_len;
        let decoded = match codec {
            CODEC_ZSTD => zstd::bulk::decompress(payload, raw).map_err(|e| Error(e.to_string()))?,
            CODEC_RANS => rans::decode(payload).map_err(Error)?,
            other => return Err(Error(format!("unknown bucket codec {other}"))),
        };
        buckets.push(decoded);
    }
    Ok(buckets)
}
fn encode_block_split(
    coeff: &[i16; 64],
    dc_out: &mut Vec<u8>,
    ac_count_out: &mut Vec<u8>,
    ac_pos_out: &mut [Vec<u8>; POS_BUCKETS],
    ac_val_out: &mut Vec<u8>,
    predicted_dc: i16,
) -> i16 {
    let mut ordered = [0i16; 64];
    for i in 0..64 {
        ordered[i] = coeff[ZIGZAG[i]];
    }
    put_uvarint(
        dc_out,
        zigzag_i32(i32::from(ordered[0]) - i32::from(predicted_dc)),
    );
    let mut nz = Vec::new();
    for (i, value) in ordered.iter().enumerate().skip(1) {
        if *value != 0 {
            nz.push((i, *value));
        }
    }
    ac_count_out.push(nz.len().min(63) as u8);
    let mut previous = 0usize;
    for (position, value) in nz.into_iter().take(63) {
        let run = position.saturating_sub(previous + 1);
        ac_pos_out[pos_bucket(previous)].push(run as u8);
        put_uvarint(ac_val_out, zigzag_i16(value));
        previous = position;
    }
    ordered[0]
}

// WHY: explicit stream cursors keep byte accounting unambiguous.
#[allow(clippy::too_many_arguments)]
fn decode_block_split(
    dc_data: &[u8],
    dc_cursor: &mut usize,
    count: u8,
    ac_pos_data: &[Vec<u8>],
    ac_pos_cursor: &mut [usize; POS_BUCKETS],
    ac_val_data: &[u8],
    ac_val_cursor: &mut usize,
    predicted_dc: i16,
) -> Result<([i16; 64], i16)> {
    let difference = unzigzag_i32(read_uvarint(dc_data, dc_cursor)?);
    let dc = (i32::from(predicted_dc) + difference).clamp(-32768, 32767) as i16;
    let mut coeff = [0i16; 64];
    coeff[0] = dc;
    let mut previous = 0usize;
    for _ in 0..count {
        let bucket = pos_bucket(previous);
        let cursor = &mut ac_pos_cursor[bucket];
        let run = *ac_pos_data[bucket]
            .get(*cursor)
            .ok_or_else(|| Error("truncated run".into()))? as usize;
        *cursor += 1;
        let position = previous + run + 1;
        let value = unzigzag_i16(read_uvarint(ac_val_data, ac_val_cursor)?);
        if position < 64 {
            coeff[ZIGZAG[position]] = value;
        }
        previous = position;
    }
    Ok((coeff, dc))
}

fn encode_block(coeff: &[i16; 64], out: &mut Vec<u8>, previous_dc: i16) -> i16 {
    let mut ordered = [0i16; 64];
    for i in 0..64 {
        ordered[i] = coeff[ZIGZAG[i]];
    }
    put_uvarint(out, zigzag_i32(i32::from(ordered[0]) - i32::from(previous_dc)));
    let mut nz = Vec::new();
    for (i, value) in ordered.iter().enumerate().skip(1) {
        if *value != 0 {
            nz.push((i, *value));
        }
    }
    out.push(nz.len().min(63) as u8);
    for (position, value) in nz.into_iter().take(63) {
        out.push(position as u8);
        put_uvarint(out, zigzag_i16(value));
    }
    ordered[0]
}

fn rgb_to_ycbcr(r: f32, g: f32, b: f32) -> [f32; 3] {
    let y = 0.299 * r + 0.587 * g + 0.114 * b;
    let cb = -0.168736 * r - 0.331264 * g + 0.5 * b + 128.0;
    let cr = 0.5 * r - 0.418688 * g - 0.081312 * b + 128.0;
    [y, cb, cr]
}

fn ycbcr_to_rgb(y: f32, cb: f32, cr: f32) -> [f32; 3] {
    let r = y + 1.402 * (cr - 128.0);
    let g = y - 0.344136 * (cb - 128.0) - 0.714136 * (cr - 128.0);
    let b = y + 1.772 * (cb - 128.0);
    [r, g, b]
}

fn tile_rect(x: u32, y: u32, width: u32, height: u32) -> (u32, u32, u32, u32) {
    let tx = x * TILE_SIZE;
    let ty = y * TILE_SIZE;
    (tx, ty, TILE_SIZE.min(width - tx), TILE_SIZE.min(height - ty))
}

#[derive(Debug, Clone, Copy)]
pub struct Info {
    pub width: u32,
    pub height: u32,
    pub tile_size: u32,
    pub basis: u16,
    pub step: f32,
    pub section_count: u16,
    pub file_size: u64,
}

pub fn inspect(data: &[u8]) -> Result<Info> {
    if data.len() < HEADER_SIZE || &data[0..8] != MAGIC {
        return Err(Error("not a new RFI file".into()));
    }
    let version_major = read_u16(data, 8)?;
    if version_major != VERSION_MAJOR {
        return Err(Error("unsupported RFI version".into()));
    }
    let width = read_u32(data, 16)?;
    let height = read_u32(data, 20)?;
    let section_count = read_u16(data, 24)?;
    let basis = read_u16(data, 26)?;
    let step = f32::from_le_bytes(
        data.get(28..32)
            .ok_or_else(|| Error("missing step".into()))?
            .try_into()
            .unwrap(),
    );
    let stored_crc = read_u32(data, 44)?;
    let mut header_copy = data[..HEADER_SIZE].to_vec();
    header_copy[44..48].fill(0);
    if crc32c(&header_copy) != stored_crc {
        return Err(Error("header CRC mismatch".into()));
    }
    if data.get(48).copied().unwrap_or(0) != 1 {
        return Err(Error("unsupported RFI color mode".into()));
    }
    let matrix = data.get(49).copied().unwrap_or(0);
    match basis {
        BASIS_DCT8 if matrix != 1 => {
            return Err(Error("unsupported RFI quantization matrix".into()));
        }
        BASIS_DCT16 if matrix != 0 => {
            return Err(Error("unsupported DCT16 matrix".into()));
        }
        BASIS_DCT8 | BASIS_DCT16 => {}
        _ => return Err(Error("unknown RFI basis".into())),
    }
    let table_len = HEADER_SIZE + section_count as usize * SECTION_ENTRY;
    if data.len() < table_len {
        return Err(Error("truncated section table".into()));
    }
    Ok(Info {
        width,
        height,
        tile_size: TILE_SIZE,
        basis,
        step,
        section_count,
        file_size: read_u64(data, 36)?,
    })
}

/// Measurement helper: runs the transform/quantization stage only and returns
/// the concatenated coefficient bytes, so benchmarks can separate transform cost
/// from entropy-coding cost.
pub fn bench_transform_only(image: &RgbaImage, step: f32) -> Result<Vec<u8>> {
    let width = image.width();
    let height = image.height();
    let tiles_x = width.div_ceil(TILE_SIZE);
    let tiles_y = height.div_ceil(TILE_SIZE);
    let mut out = Vec::new();
    for ty in 0..tiles_y {
        for tx in 0..tiles_x {
            let (px, py, _tw, _th) = tile_rect(tx, ty, width, height);
            for by in 0..BLOCKS_PER_TILE {
                for bx in 0..BLOCKS_PER_TILE {
                    let mut block = [0.0f32; 64];
                    for y in 0..8u32 {
                        for x in 0..8u32 {
                            let sx = (px + bx * 8 + x).min(width - 1);
                            let sy = (py + by * 8 + y).min(height - 1);
                            let p = image.get_pixel(sx, sy).0;
                            let ycc = rgb_to_ycbcr(f32::from(p[0]), f32::from(p[1]), f32::from(p[2]));
                            block[(y * 8 + x) as usize] = ycc[0] - 128.0;
                        }
                    }
                    let coeff = quantize(&dct8(&block), step, &QUANT_LUMA);
                    for value in coeff {
                        out.extend_from_slice(&value.to_le_bytes());
                    }
                }
            }
        }
    }
    Ok(out)
}

/// Measurement helper: reports the size of each raw coefficient stream.
pub fn bench_stream_sizes(image: &RgbaImage, step: f32) -> Result<Vec<(&'static str, usize)>> {
    let width = image.width();
    let height = image.height();
    let tiles_x = width.div_ceil(TILE_SIZE);
    let tiles_y = height.div_ceil(TILE_SIZE);
    let luma_blocks_x = (tiles_x * BLOCKS_PER_TILE) as usize;
    let chroma_blocks_x = (tiles_x * 4) as usize;
    let mut luma_grid = vec![0i16; luma_blocks_x * (tiles_y * BLOCKS_PER_TILE) as usize];
    let mut cb_grid = vec![0i16; chroma_blocks_x * (tiles_y * 4) as usize];
    let mut cr_grid = vec![0i16; chroma_blocks_x * (tiles_y * 4) as usize];
    let mut dc_luma = Vec::new();
    let mut dc_chroma = Vec::new();
    let mut counts = Vec::new();
    let mut positions: [Vec<u8>; POS_BUCKETS] = std::array::from_fn(|_| Vec::new());
    let mut values = Vec::new();
    for ty in 0..tiles_y {
        for tx in 0..tiles_x {
            let (px, py, _tw, _th) = tile_rect(tx, ty, width, height);
            for by in 0..BLOCKS_PER_TILE {
                for bx in 0..BLOCKS_PER_TILE {
                    let mut block = [0.0f32; 64];
                    for y in 0..8u32 {
                        for x in 0..8u32 {
                            let sx = (px + bx * 8 + x).min(width - 1);
                            let sy = (py + by * 8 + y).min(height - 1);
                            let p = image.get_pixel(sx, sy).0;
                            let ycc = rgb_to_ycbcr(f32::from(p[0]), f32::from(p[1]), f32::from(p[2]));
                            block[(y * 8 + x) as usize] = ycc[0] - 128.0;
                        }
                    }
                    let gx = (tx * BLOCKS_PER_TILE + bx) as usize;
                    let gy = (ty * BLOCKS_PER_TILE + by) as usize;
                    let left = (gx > 0).then(|| luma_grid[gy * luma_blocks_x + gx - 1]);
                    let top = (gy > 0).then(|| luma_grid[(gy - 1) * luma_blocks_x + gx]);
                    let tl = (gx > 0 && gy > 0).then(|| luma_grid[(gy - 1) * luma_blocks_x + gx - 1]);
                    let predicted = med_predict(left, top, tl);
                    let coeff = quantize(&dct8(&block), step, &QUANT_LUMA);
                    let actual = encode_block_split(
                        &coeff,
                        &mut dc_luma,
                        &mut counts,
                        &mut positions,
                        &mut values,
                        predicted,
                    );
                    luma_grid[gy * luma_blocks_x + gx] = actual;
                }
            }
            for channel in 1..3 {
                let grid = if channel == 1 { &mut cb_grid } else { &mut cr_grid };
                for by in 0..4u32 {
                    for bx in 0..4u32 {
                        let mut block = [0.0f32; 64];
                        for y in 0..8u32 {
                            for x in 0..8u32 {
                                let mut sum = 0.0f32;
                                for oy in 0..2u32 {
                                    for ox in 0..2u32 {
                                        let sx = (px + bx * 16 + x * 2 + ox).min(width - 1);
                                        let sy = (py + by * 16 + y * 2 + oy).min(height - 1);
                                        let p = image.get_pixel(sx, sy).0;
                                        let ycc =
                                            rgb_to_ycbcr(f32::from(p[0]), f32::from(p[1]), f32::from(p[2]));
                                        sum += ycc[channel];
                                    }
                                }
                                block[(y * 8 + x) as usize] = sum / 4.0 - 128.0;
                            }
                        }
                        let gx = (tx * 4 + bx) as usize;
                        let gy = (ty * 4 + by) as usize;
                        let left = (gx > 0).then(|| grid[gy * chroma_blocks_x + gx - 1]);
                        let top = (gy > 0).then(|| grid[(gy - 1) * chroma_blocks_x + gx]);
                        let tl = (gx > 0 && gy > 0).then(|| grid[(gy - 1) * chroma_blocks_x + gx - 1]);
                        let predicted = med_predict(left, top, tl);
                        let coeff = quantize(&dct8(&block), step, &QUANT_CHROMA);
                        let actual = encode_block_split(
                            &coeff,
                            &mut dc_chroma,
                            &mut counts,
                            &mut positions,
                            &mut values,
                            predicted,
                        );
                        grid[gy * chroma_blocks_x + gx] = actual;
                    }
                }
            }
        }
    }
    let pos_raw: usize = positions.iter().map(|b| b.len()).sum();
    Ok(vec![
        ("dc_luma", dc_luma.len()),
        ("dc_chroma", dc_chroma.len()),
        ("counts", counts.len()),
        ("positions", pos_raw),
        ("values", values.len()),
    ])
}
fn pack_counts(counts: &[u8]) -> (Vec<u8>, Vec<u8>) {
    let mut packed = Vec::with_capacity(counts.len().div_ceil(2));
    let mut ext = Vec::new();
    for pair in counts.chunks(2) {
        let low = if pair[0] >= 15 {
            ext.push(pair[0]);
            15
        } else {
            pair[0]
        };
        let high = if pair.len() > 1 {
            if pair[1] >= 15 {
                ext.push(pair[1]);
                15
            } else {
                pair[1]
            }
        } else {
            0
        };
        packed.push(low | (high << 4));
    }
    (packed, ext)
}

fn unpack_counts(packed: &[u8], ext: &[u8], n: usize) -> Result<Vec<u8>> {
    let mut counts = Vec::with_capacity(n);
    let mut ext_cursor = 0usize;
    for index in 0..n {
        let byte = packed
            .get(index / 2)
            .ok_or_else(|| Error("truncated AC counts".into()))?;
        let nibble = if index % 2 == 0 { byte & 0x0f } else { byte >> 4 };
        let count = if nibble == 15 {
            let value = *ext
                .get(ext_cursor)
                .ok_or_else(|| Error("truncated AC count extension".into()))?;
            ext_cursor += 1;
            value
        } else {
            nibble
        };
        counts.push(count);
    }
    Ok(counts)
}

/// Entropy backend for the coefficient sections.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Entropy {
    Zstd,
    Rans,
    /// Choose zstd or rANS independently for every coefficient section.
    Best,
}

/// zstd level 19 via the one-shot API. Multithreaded mode was measured and
/// rejected: these streams are ~190 KB, so job splitting costs more than it
/// saves (348 ms vs 244 ms for a 1920x1080 encode).
fn zstd_compress(data: &[u8], level: i32) -> Result<Vec<u8>> {
    zstd::bulk::compress(data, level).map_err(|e| Error(e.to_string()))
}
fn pack_stream(data: &[u8], entropy: Entropy) -> Result<(Vec<u8>, u8)> {
    match entropy {
        Entropy::Zstd => Ok((zstd_compress(data, 19)?, CODEC_ZSTD)),
        Entropy::Rans => Ok((rans::encode(data), CODEC_RANS)),
        Entropy::Best => pack_best(data),
    }
}

/// Per-section backend choice: pick whichever backend produces fewer stored bytes.
/// The section table already carries a codec id per section, so a single file may
/// freely mix zstd and rANS sections.
fn pack_best(data: &[u8]) -> Result<(Vec<u8>, u8)> {
    let zstd = zstd_compress(data, 19)?;
    let rans = rans::encode(data);
    if rans.len() < zstd.len() {
        Ok((rans, CODEC_RANS))
    } else {
        Ok((zstd, CODEC_ZSTD))
    }
}

pub fn encode(image: &RgbaImage, step: f32) -> Result<Vec<u8>> {
    encode_with(image, step, Entropy::Zstd)
}

pub fn encode_with(image: &RgbaImage, step: f32, entropy: Entropy) -> Result<Vec<u8>> {
    let width = image.width();
    let height = image.height();
    if width == 0 || height == 0 {
        return Err(Error("empty image".into()));
    }
    let tiles_x = width.div_ceil(TILE_SIZE);
    let tiles_y = height.div_ceil(TILE_SIZE);
    let luma_blocks_x = (tiles_x * BLOCKS_PER_TILE) as usize;
    let luma_blocks_y = (tiles_y * BLOCKS_PER_TILE) as usize;
    let chroma_blocks_x = (tiles_x * 4) as usize;
    let chroma_blocks_y = (tiles_y * 4) as usize;
    let mut luma_grid = vec![0i16; luma_blocks_x * luma_blocks_y];
    let mut cb_grid = vec![0i16; chroma_blocks_x * chroma_blocks_y];
    let mut cr_grid = vec![0i16; chroma_blocks_x * chroma_blocks_y];
    let mut dc_luma = Vec::new();
    let mut dc_chroma = Vec::new();
    let mut ac_counts = Vec::new();
    let mut ac_positions: [Vec<u8>; POS_BUCKETS] = std::array::from_fn(|_| Vec::new());
    let mut ac_values = Vec::new();
    for ty in 0..tiles_y {
        for tx in 0..tiles_x {
            let (px, py, _tw, _th) = tile_rect(tx, ty, width, height);
            let tile_step = step;
            for by in 0..BLOCKS_PER_TILE {
                for bx in 0..BLOCKS_PER_TILE {
                    let mut block = [0.0f32; 64];
                    for y in 0..8 {
                        for x in 0..8 {
                            let sx = (px + bx * 8 + x).min(width - 1);
                            let sy = (py + by * 8 + y).min(height - 1);
                            let p = image.get_pixel(sx, sy).0;
                            let ycc = rgb_to_ycbcr(f32::from(p[0]), f32::from(p[1]), f32::from(p[2]));
                            block[(y * 8 + x) as usize] = ycc[0] - 128.0;
                        }
                    }
                    let gx = (tx * BLOCKS_PER_TILE + bx) as usize;
                    let gy = (ty * BLOCKS_PER_TILE + by) as usize;
                    let left = (gx > 0).then(|| luma_grid[gy * luma_blocks_x + gx - 1]);
                    let top = (gy > 0).then(|| luma_grid[(gy - 1) * luma_blocks_x + gx]);
                    let top_left = (gx > 0 && gy > 0).then(|| luma_grid[(gy - 1) * luma_blocks_x + gx - 1]);
                    let predicted = med_predict(left, top, top_left);
                    let coeff = quantize(&dct8(&block), tile_step, &QUANT_LUMA);
                    let actual = encode_block_split(
                        &coeff,
                        &mut dc_luma,
                        &mut ac_counts,
                        &mut ac_positions,
                        &mut ac_values,
                        predicted,
                    );
                    luma_grid[gy * luma_blocks_x + gx] = actual;
                }
            }
            for channel in 1..3 {
                let grid = if channel == 1 { &mut cb_grid } else { &mut cr_grid };
                for by in 0..4 {
                    for bx in 0..4 {
                        let mut block = [0.0f32; 64];
                        for y in 0..8 {
                            for x in 0..8 {
                                let mut sum = 0.0f32;
                                for oy in 0..2 {
                                    for ox in 0..2 {
                                        let sx = (px + bx * 16 + x * 2 + ox).min(width - 1);
                                        let sy = (py + by * 16 + y * 2 + oy).min(height - 1);
                                        let p = image.get_pixel(sx, sy).0;
                                        let ycc =
                                            rgb_to_ycbcr(f32::from(p[0]), f32::from(p[1]), f32::from(p[2]));
                                        sum += ycc[channel];
                                    }
                                }
                                block[(y * 8 + x) as usize] = sum / 4.0 - 128.0;
                            }
                        }
                        let gx = (tx * 4 + bx) as usize;
                        let gy = (ty * 4 + by) as usize;
                        let left = (gx > 0).then(|| grid[gy * chroma_blocks_x + gx - 1]);
                        let top = (gy > 0).then(|| grid[(gy - 1) * chroma_blocks_x + gx]);
                        let top_left = (gx > 0 && gy > 0).then(|| grid[(gy - 1) * chroma_blocks_x + gx - 1]);
                        let predicted = med_predict(left, top, top_left);
                        let coeff = quantize(&dct8(&block), tile_step, &QUANT_CHROMA);
                        let actual = encode_block_split(
                            &coeff,
                            &mut dc_chroma,
                            &mut ac_counts,
                            &mut ac_positions,
                            &mut ac_values,
                            predicted,
                        );
                        grid[gy * chroma_blocks_x + gx] = actual;
                    }
                }
            }
        }
    }
    let basis = {
        let mut data = Vec::new();
        put_u16(&mut data, BASIS_DCT8);
        put_u16(&mut data, 8);
        put_u16(&mut data, 8);
        put_u16(&mut data, 0);
        data
    };
    let (packed_counts, count_ext) = pack_counts(&ac_counts);
    let ac_pos_raw: usize = ac_positions.iter().map(|bucket| bucket.len()).sum();
    // Every section is independent, so compress them in parallel.
    let (
        (dc_luma_data, dc_luma_codec),
        (dc_chroma_data, dc_chroma_codec),
        (ac_count_data, ac_count_codec),
        (ac_ext_data, ac_ext_codec),
        (ac_val_data, ac_val_codec),
    ) = {
        use rayon::prelude::*;
        let sources: [&[u8]; 5] = [&dc_luma, &dc_chroma, &packed_counts, &count_ext, &ac_values];
        let packed: Result<Vec<(Vec<u8>, u8)>> = sources
            .par_iter()
            .map(|data| pack_stream(data, entropy))
            .collect();
        let packed = packed?;
        let mut iter = packed.into_iter();
        (
            iter.next().expect("section 0"),
            iter.next().expect("section 1"),
            iter.next().expect("section 2"),
            iter.next().expect("section 3"),
            iter.next().expect("section 4"),
        )
    };
    let ac_pos_frame = pack_buckets(&ac_positions, entropy)?;
    let basis_compressed = zstd::bulk::compress(&basis, 19).map_err(|e| Error(e.to_string()))?;
    // Tile coordinates/basis are derivable from the fixed tile grid; no
    // per-tile side data is required in this format.
    let sections = [
        (
            SECTION_BASIS,
            basis_compressed,
            basis.len() as u64,
            &basis[..],
            CODEC_ZSTD,
        ),
        (
            SECTION_COEFF_DC,
            dc_luma_data,
            dc_luma.len() as u64,
            &dc_luma[..],
            dc_luma_codec,
        ),
        (
            SECTION_DC_CHROMA,
            dc_chroma_data,
            dc_chroma.len() as u64,
            &dc_chroma[..],
            dc_chroma_codec,
        ),
        (
            SECTION_AC_COUNT,
            ac_count_data,
            packed_counts.len() as u64,
            &packed_counts[..],
            ac_count_codec,
        ),
        (
            SECTION_AC_EXT,
            ac_ext_data,
            count_ext.len() as u64,
            &count_ext[..],
            ac_ext_codec,
        ),
        (
            SECTION_AC_POS,
            ac_pos_frame,
            ac_pos_raw as u64,
            &[] as &[u8],
            CODEC_BUCKETED,
        ),
        (
            SECTION_AC_VAL,
            ac_val_data,
            ac_values.len() as u64,
            &ac_values[..],
            ac_val_codec,
        ),
    ];
    let table_len = HEADER_SIZE + sections.len() * SECTION_ENTRY;
    let mut table = vec![0u8; table_len];
    let mut offset = table_len;
    for (index, (kind, data, raw, _, section_codec)) in sections.iter().enumerate() {
        let entry = HEADER_SIZE + index * SECTION_ENTRY;
        table[entry..entry + 2].copy_from_slice(&kind.to_le_bytes());
        table[entry + 2] = *section_codec;
        table[entry + 3] = 0;
        table[entry + 8..entry + 16].copy_from_slice(&(offset as u64).to_le_bytes());
        table[entry + 16..entry + 24].copy_from_slice(&(data.len() as u64).to_le_bytes());
        table[entry + 24..entry + 32].copy_from_slice(&raw.to_le_bytes());
        offset += data.len();
    }
    for (index, (_, data, _, raw_bytes, section_codec)) in sections.iter().enumerate() {
        let entry = HEADER_SIZE + index * SECTION_ENTRY;
        // A bucketed frame has no single uncompressed form, so it is checksummed
        // over its stored bytes; every other section is checksummed over raw bytes.
        let payload = if *section_codec == CODEC_BUCKETED {
            &data[..]
        } else {
            raw_bytes
        };
        table[entry + 4..entry + 8].copy_from_slice(&crc32c(payload).to_le_bytes());
    }
    let mut file = table;
    for (_, data, _, _, _) in &sections {
        file.extend_from_slice(data);
    }
    // Header
    let mut header = vec![0u8; HEADER_SIZE];
    header[0..8].copy_from_slice(MAGIC);
    header[8..10].copy_from_slice(&VERSION_MAJOR.to_le_bytes());
    header[10..12].copy_from_slice(&VERSION_MINOR.to_le_bytes());
    header[12..14].copy_from_slice(&(HEADER_SIZE as u16).to_le_bytes());
    header[14] = 4;
    header[15] = 6; // tile_size_log2 64
    header[16..20].copy_from_slice(&width.to_le_bytes());
    header[20..24].copy_from_slice(&height.to_le_bytes());
    header[24..26].copy_from_slice(&(sections.len() as u16).to_le_bytes());
    header[26..28].copy_from_slice(&BASIS_DCT8.to_le_bytes());
    header[28..32].copy_from_slice(&step.to_le_bytes());
    header[32..36].copy_from_slice(&(HEADER_SIZE as u32).to_le_bytes());
    header[36..44].copy_from_slice(&(file.len() as u64).to_le_bytes());
    header[48] = 1; // YCbCr 4:2:0, opaque alpha
    header[49] = 1; // fixed Y/chroma quantization matrices
    header[44..48].fill(0);
    let crc = crc32c(&header);
    header[44..48].copy_from_slice(&crc.to_le_bytes());
    file[0..HEADER_SIZE].copy_from_slice(&header);
    Ok(file)
}

/// Encode at the smallest qstep whose complete serialized file fits the target.
pub fn encode_to_budget(image: &RgbaImage, target_bytes: usize) -> Result<(f32, Vec<u8>)> {
    encode_to_budget_with(image, target_bytes, Entropy::Zstd)
}

pub fn encode_to_budget_with(
    image: &RgbaImage,
    target_bytes: usize,
    entropy: Entropy,
) -> Result<(f32, Vec<u8>)> {
    let mut low = 0.25f32;
    let mut high = 2_000.0f32;
    let high_bytes = encode_with(image, high, entropy)?;
    if high_bytes.len() > target_bytes {
        return Ok((high, high_bytes));
    }
    let mut best_step = high;
    let mut best = high_bytes;
    for _ in 0..18 {
        let mid = 0.5 * (low + high);
        let bytes = encode_with(image, mid, entropy)?;
        if bytes.len() <= target_bytes {
            best_step = mid;
            best = bytes;
            high = mid;
        } else {
            low = mid;
        }
    }
    Ok((best_step, best))
}
pub fn decode(data: &[u8]) -> Result<RgbaImage> {
    if data.len() < HEADER_SIZE || &data[0..8] != MAGIC {
        return Err(Error("not a new RFI file (old RFI is rejected)".into()));
    }
    if read_u16(data, 26)? == BASIS_DCT16 {
        return dct16::decode16(data);
    }
    let width = read_u32(data, 16)?;
    let height = read_u32(data, 20)?;
    let section_count = read_u16(data, 24)? as usize;
    let step = f32::from_le_bytes(
        data.get(28..32)
            .ok_or_else(|| Error("missing step".into()))?
            .try_into()
            .unwrap(),
    );
    if data.get(48).copied().unwrap_or(0) != 1 {
        return Err(Error("unsupported RFI color mode".into()));
    }
    if data.get(49).copied().unwrap_or(0) != 1 {
        return Err(Error("unsupported RFI quantization matrix".into()));
    }
    let stored_crc = read_u32(data, 44)?;
    let mut header_copy = data[..HEADER_SIZE].to_vec();
    header_copy[44..48].fill(0);
    if crc32c(&header_copy) != stored_crc {
        return Err(Error("header CRC mismatch".into()));
    }
    let table_len = HEADER_SIZE + section_count * SECTION_ENTRY;
    if data.len() < table_len {
        return Err(Error("truncated section table".into()));
    }
    let mut dc_luma_section = None;
    let mut dc_chroma_section = None;
    let mut ac_count_section = None;
    let mut ac_ext_section = None;
    let mut ac_val_section = None;
    let mut ac_pos_buckets: Option<Vec<Vec<u8>>> = None;
    for index in 0..section_count {
        let entry = HEADER_SIZE + index * SECTION_ENTRY;
        let kind = read_u16(data, entry)?;
        let offset = read_u32(data, entry + 8)? as usize;
        let len = read_u32(data, entry + 16)? as usize;
        let raw = read_u64(data, entry + 24)? as usize;
        if offset + len > data.len() {
            return Err(Error("section outside file".into()));
        }
        let section = &data[offset..offset + len];
        let codec = data.get(entry + 2).copied().unwrap_or(CODEC_ZSTD);
        // Bucketed frames carry their own per-bucket codecs and are unpacked by
        // the reader that consumes that section, so they bypass the flat path.
        if codec == CODEC_BUCKETED {
            let buckets = unpack_buckets(section)?;
            if kind == SECTION_AC_POS {
                ac_pos_buckets = Some(buckets);
            }
            continue;
        }
        let decoded = match codec {
            CODEC_RANS => rans::decode(section).map_err(Error)?,
            CODEC_ZSTD => zstd::bulk::decompress(section, raw).map_err(|e| Error(e.to_string()))?,
            other => return Err(Error(format!("unknown section codec {other}"))),
        };
        if crc32c(&decoded) != read_u32(data, entry + 4)? {
            return Err(Error("section CRC mismatch".into()));
        }
        if kind == SECTION_COEFF_DC {
            dc_luma_section = Some(decoded);
        } else if kind == SECTION_DC_CHROMA {
            dc_chroma_section = Some(decoded);
        } else if kind == SECTION_AC_COUNT {
            ac_count_section = Some(decoded);
        } else if kind == SECTION_AC_EXT {
            ac_ext_section = Some(decoded);
        } else if kind == SECTION_AC_VAL {
            ac_val_section = Some(decoded);
        }
    }
    let dc_luma = dc_luma_section.ok_or_else(|| Error("missing luma DC".into()))?;
    let dc_chroma = dc_chroma_section.ok_or_else(|| Error("missing chroma DC".into()))?;
    let packed_counts = ac_count_section.ok_or_else(|| Error("missing AC counts".into()))?;
    let count_ext = ac_ext_section.ok_or_else(|| Error("missing AC count extension".into()))?;
    let ac_positions = ac_pos_buckets.ok_or_else(|| Error("missing AC position buckets".into()))?;
    let ac_values = ac_val_section.ok_or_else(|| Error("missing AC values".into()))?;
    let mut image = RgbaImage::new(width, height);
    let mut dc_luma_cursor = 0usize;
    let mut dc_chroma_cursor = 0usize;
    let mut ac_val_cursor = 0usize;
    let tiles_x = width.div_ceil(TILE_SIZE);
    let tiles_y = height.div_ceil(TILE_SIZE);
    let blocks_per_tile = (BLOCKS_PER_TILE * BLOCKS_PER_TILE + 32) as usize;
    let block_count = (tiles_x * tiles_y) as usize * blocks_per_tile;
    let ac_counts = unpack_counts(&packed_counts, &count_ext, block_count)?;
    let mut count_index = 0usize;
    let mut ac_pos_cursor: [usize; POS_BUCKETS] = [0; POS_BUCKETS];
    let luma_blocks_x = (tiles_x * BLOCKS_PER_TILE) as usize;
    let luma_blocks_y = (tiles_y * BLOCKS_PER_TILE) as usize;
    let chroma_blocks_x = (tiles_x * 4) as usize;
    let chroma_blocks_y = (tiles_y * 4) as usize;
    let mut luma_grid = vec![0i16; luma_blocks_x * luma_blocks_y];
    let mut cb_grid = vec![0i16; chroma_blocks_x * chroma_blocks_y];
    let mut cr_grid = vec![0i16; chroma_blocks_x * chroma_blocks_y];
    for ty in 0..tiles_y {
        for tx in 0..tiles_x {
            let (px, py, tw, th) = tile_rect(tx, ty, width, height);
            let tile_step = step;
            let mut y_plane = [128.0f32; 64 * 64];
            let mut cb_plane = [128.0f32; 32 * 32];
            let mut cr_plane = [128.0f32; 32 * 32];
            for by in 0..BLOCKS_PER_TILE {
                for bx in 0..BLOCKS_PER_TILE {
                    let gx = (tx * BLOCKS_PER_TILE + bx) as usize;
                    let gy = (ty * BLOCKS_PER_TILE + by) as usize;
                    let left = (gx > 0).then(|| luma_grid[gy * luma_blocks_x + gx - 1]);
                    let top = (gy > 0).then(|| luma_grid[(gy - 1) * luma_blocks_x + gx]);
                    let top_left = (gx > 0 && gy > 0).then(|| luma_grid[(gy - 1) * luma_blocks_x + gx - 1]);
                    let predicted = med_predict(left, top, top_left);
                    let count = ac_counts[count_index];
                    count_index += 1;
                    let (coeff, dc_next) = decode_block_split(
                        &dc_luma,
                        &mut dc_luma_cursor,
                        count,
                        &ac_positions,
                        &mut ac_pos_cursor,
                        &ac_values,
                        &mut ac_val_cursor,
                        predicted,
                    )?;
                    luma_grid[gy * luma_blocks_x + gx] = dc_next;
                    let block = idct8(&dequantize(&coeff, tile_step, &QUANT_LUMA));
                    for y in 0..8 {
                        for x in 0..8 {
                            y_plane[((by * 8 + y) * 64 + bx * 8 + x) as usize] =
                                block[(y * 8 + x) as usize] + 128.0;
                        }
                    }
                }
            }
            for (channel, plane) in [&mut cb_plane, &mut cr_plane].into_iter().enumerate() {
                let grid = if channel == 1 { &mut cb_grid } else { &mut cr_grid };
                for by in 0..4 {
                    for bx in 0..4 {
                        let gx = (tx * 4 + bx) as usize;
                        let gy = (ty * 4 + by) as usize;
                        let left = (gx > 0).then(|| grid[gy * chroma_blocks_x + gx - 1]);
                        let top = (gy > 0).then(|| grid[(gy - 1) * chroma_blocks_x + gx]);
                        let top_left = (gx > 0 && gy > 0).then(|| grid[(gy - 1) * chroma_blocks_x + gx - 1]);
                        let predicted = med_predict(left, top, top_left);
                        let count = ac_counts[count_index];
                        count_index += 1;
                        let (coeff, dc_next) = decode_block_split(
                            &dc_chroma,
                            &mut dc_chroma_cursor,
                            count,
                            &ac_positions,
                            &mut ac_pos_cursor,
                            &ac_values,
                            &mut ac_val_cursor,
                            predicted,
                        )?;
                        grid[gy * chroma_blocks_x + gx] = dc_next;
                        let block = idct8(&dequantize(&coeff, tile_step, &QUANT_CHROMA));
                        for y in 0..8 {
                            for x in 0..8 {
                                plane[((by * 8 + y) * 32 + bx * 8 + x) as usize] =
                                    block[(y * 8 + x) as usize] + 128.0;
                            }
                        }
                    }
                }
                let _ = channel;
            }
            for y in 0..th {
                for x in 0..tw {
                    let yy = y_plane[(y * 64 + x) as usize];
                    let cx = ((x as f32 + 0.5) / 2.0 - 0.5).clamp(0.0, 31.0);
                    let cy = ((y as f32 + 0.5) / 2.0 - 0.5).clamp(0.0, 31.0);
                    let x0 = cx.floor() as u32;
                    let y0 = cy.floor() as u32;
                    let x1 = (x0 + 1).min(31);
                    let y1 = (y0 + 1).min(31);
                    let fx = cx - x0 as f32;
                    let fy = cy - y0 as f32;
                    let sample = |plane: &[f32; 1024]| {
                        let a = plane[(y0 * 32 + x0) as usize];
                        let b = plane[(y0 * 32 + x1) as usize];
                        let c = plane[(y1 * 32 + x0) as usize];
                        let d = plane[(y1 * 32 + x1) as usize];
                        let top = a + (b - a) * fx;
                        let bottom = c + (d - c) * fx;
                        top + (bottom - top) * fy
                    };
                    let cb = sample(&cb_plane);
                    let cr = sample(&cr_plane);
                    let rgb = ycbcr_to_rgb(yy, cb, cr);
                    image.put_pixel(
                        px + x,
                        py + y,
                        Rgba([
                            rgb[0].clamp(0.0, 255.0) as u8,
                            rgb[1].clamp(0.0, 255.0) as u8,
                            rgb[2].clamp(0.0, 255.0) as u8,
                            255,
                        ]),
                    );
                }
            }
        }
    }
    Ok(image)
}

#[cfg(test)]
mod tests {
    use image::{Rgba, RgbaImage};

    fn image(width: u32, height: u32) -> RgbaImage {
        let mut image = RgbaImage::new(width, height);
        for y in 0..height {
            for x in 0..width {
                image.put_pixel(
                    x,
                    y,
                    Rgba([
                        (x * 255 / width.max(1)) as u8,
                        (y * 255 / height.max(1)) as u8,
                        ((x + y) * 127 / width.max(1)) as u8,
                        255,
                    ]),
                );
            }
        }
        image
    }

    #[test]
    fn dct_round_trip_split() {
        let source = image(64, 64);
        let bytes = super::encode(&source, 20.0).expect("encode");
        let decoded = super::decode(&bytes).expect("decode");
        assert_eq!(decoded.dimensions(), source.dimensions());
        let mut sum = 0.0f64;
        for (a, b) in source.pixels().zip(decoded.pixels()) {
            for c in 0..4 {
                let diff = f64::from(a.0[c]) - f64::from(b.0[c]);
                sum += diff * diff;
            }
        }
        let mse = sum / (64.0 * 64.0 * 4.0);
        let psnr = 10.0 * (255.0f64 * 255.0 / mse).log10();
        assert!(psnr > 30.0, "psnr {psnr}");
    }

    #[test]
    fn old_magic_rejected() {
        assert!(super::decode(b"RFI placeholder").is_err());
    }

    #[test]
    fn rans_round_trip() {
        let source = image(48, 48);
        for entropy in [super::Entropy::Zstd, super::Entropy::Rans] {
            let bytes = super::encode_with(&source, 8.0, entropy).expect("encode");
            let decoded = super::decode(&bytes).expect("decode");
            assert_eq!(decoded.dimensions(), source.dimensions());
        }
    }

    #[test]
    fn auto_backend_mixes_sections() {
        let source = image(64, 64);
        let bytes = super::encode_with(&source, 8.0, super::Entropy::Best).expect("encode");
        let info = super::inspect(&bytes).expect("inspect");
        assert!(info.section_count >= 6);
        let decoded = super::decode(&bytes).expect("decode");
        assert_eq!(decoded.dimensions(), source.dimensions());
        let mut codecs = Vec::new();
        for index in 0..info.section_count as usize {
            let entry = super::HEADER_SIZE + index * super::SECTION_ENTRY;
            codecs.push(bytes[entry + 2]);
        }
        assert!(
            codecs
                .iter()
                .all(|codec| *codec == 1 || *codec == 2 || *codec == 4)
        );
    }
}
