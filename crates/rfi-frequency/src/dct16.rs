//! Experimental 16x16 DCT basis (basis id 3).

use image::{Rgba, RgbaImage};

use super::{
    BASIS_DCT16, Error, HEADER_SIZE, MAGIC, Result, SECTION_AC_COUNT, SECTION_AC_POS, SECTION_AC_VAL,
    SECTION_BASIS, SECTION_COEFF_DC, SECTION_DC_CHROMA, SECTION_ENTRY, TILE_SIZE, VERSION_MAJOR,
    VERSION_MINOR, crc32c, put_u16, put_uvarint, read_u16, read_u32, read_u64, read_uvarint, rgb_to_ycbcr,
    unzigzag_i16, unzigzag_i32, ycbcr_to_rgb, zigzag_i16, zigzag_i32,
};

const N: usize = 16;
const BLOCK: u32 = 16;

fn cos_table() -> &'static [[f32; N]; N] {
    use std::sync::OnceLock;
    static TABLE: OnceLock<[[f32; N]; N]> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut table = [[0.0f32; N]; N];
        for (u, row) in table.iter_mut().enumerate() {
            for (x, value) in row.iter_mut().enumerate() {
                *value = (((2 * x + 1) as f32) * u as f32 * std::f32::consts::PI / (2.0 * N as f32)).cos();
            }
        }
        table
    })
}

fn dct16(block: &[f32; N * N]) -> [f32; N * N] {
    let cos = cos_table();
    let mut out = [0.0f32; N * N];
    for v in 0..N {
        for u in 0..N {
            let mut sum = 0.0f32;
            for y in 0..N {
                for x in 0..N {
                    sum += block[y * N + x] * cos[u][x] * cos[v][y];
                }
            }
            let cu = if u == 0 { 1.0 / 2f32.sqrt() } else { 1.0 };
            let cv = if v == 0 { 1.0 / 2f32.sqrt() } else { 1.0 };
            out[v * N + u] = 0.125 * cu * cv * sum;
        }
    }
    out
}

fn idct16(coeff: &[f32; N * N]) -> [f32; N * N] {
    let cos = cos_table();
    let mut out = [0.0f32; N * N];
    for y in 0..N {
        for x in 0..N {
            let mut sum = 0.0f32;
            for v in 0..N {
                for u in 0..N {
                    let cu = if u == 0 { 1.0 / 2f32.sqrt() } else { 1.0 };
                    let cv = if v == 0 { 1.0 / 2f32.sqrt() } else { 1.0 };
                    sum += cu * cv * coeff[v * N + u] * cos[u][x] * cos[v][y];
                }
            }
            out[y * N + x] = 0.125 * sum;
        }
    }
    out
}

fn zigzag() -> &'static [usize; N * N] {
    use std::sync::OnceLock;
    static Z: OnceLock<[usize; N * N]> = OnceLock::new();
    Z.get_or_init(|| {
        let mut out = [0usize; N * N];
        let mut index = 0;
        for sum in 0..(2 * N - 1) {
            let start = sum.saturating_sub(N - 1);
            let end = sum.min(N - 1);
            if sum % 2 == 0 {
                for x in start..=end {
                    out[index] = (sum - x) * N + x;
                    index += 1;
                }
            } else {
                for x in (start..=end).rev() {
                    out[index] = (sum - x) * N + x;
                    index += 1;
                }
            }
        }
        out
    })
}

/// 16x16 luma quantization weight matrix, scaled so DC = 16.
/// Built from the usual contrast-sensitivity shape: near-flat at low
/// frequencies, rising steeply in both directions, with diagonal terms
/// weighted slightly higher.
const QUANT16_LUMA: [u16; N * N] = [
    16, 16, 17, 18, 20, 22, 25, 28, 32, 36, 41, 47, 53, 60, 68, 77, 16, 16, 17, 19, 21, 24, 27, 30, 34, 39,
    44, 50, 57, 64, 73, 82, 17, 17, 18, 20, 23, 26, 29, 33, 37, 42, 48, 54, 61, 69, 78, 88, 18, 19, 20, 22,
    25, 28, 32, 36, 41, 46, 52, 59, 67, 75, 85, 96, 20, 21, 23, 25, 28, 32, 36, 41, 46, 52, 59, 67, 76, 85,
    96, 108, 22, 24, 26, 28, 32, 36, 41, 46, 52, 59, 67, 76, 86, 97, 109, 122, 25, 27, 29, 32, 36, 41, 46,
    52, 59, 67, 76, 86, 97, 110, 124, 139, 28, 30, 33, 36, 41, 46, 52, 59, 67, 76, 86, 98, 110, 124, 140,
    157, 32, 34, 37, 41, 46, 52, 59, 67, 76, 86, 98, 111, 125, 141, 159, 179, 36, 39, 42, 46, 52, 59, 67, 76,
    86, 98, 111, 126, 142, 160, 180, 203, 41, 44, 48, 52, 59, 67, 76, 86, 98, 111, 126, 143, 161, 182, 204,
    230, 47, 50, 54, 59, 67, 76, 86, 98, 111, 126, 143, 162, 183, 206, 232, 261, 53, 57, 61, 67, 76, 86, 97,
    110, 125, 142, 161, 183, 207, 233, 262, 295, 60, 64, 69, 75, 85, 97, 110, 124, 141, 160, 182, 206, 233,
    263, 296, 333, 68, 73, 78, 85, 96, 109, 124, 140, 159, 180, 204, 232, 262, 296, 334, 376, 77, 82, 88, 96,
    108, 122, 139, 157, 179, 203, 230, 261, 295, 333, 376, 424,
];

/// 16x16 chroma weight matrix: coarser than luma, matching chroma's lower
/// contrast sensitivity.
const QUANT16_CHROMA: [u16; N * N] = [
    16, 17, 19, 22, 26, 31, 37, 44, 52, 62, 74, 88, 104, 124, 147, 175, 17, 18, 20, 23, 27, 33, 39, 46, 55,
    65, 77, 92, 109, 130, 154, 183, 19, 20, 22, 25, 30, 36, 43, 51, 60, 71, 85, 101, 120, 142, 169, 201, 22,
    23, 25, 29, 34, 40, 48, 57, 68, 80, 96, 114, 135, 160, 190, 226, 26, 27, 30, 34, 40, 47, 56, 66, 79, 93,
    111, 132, 157, 186, 221, 262, 31, 33, 36, 40, 47, 56, 66, 78, 93, 110, 131, 155, 184, 219, 260, 308, 37,
    39, 43, 48, 56, 66, 78, 92, 109, 130, 154, 183, 217, 257, 305, 362, 44, 46, 51, 57, 66, 78, 92, 109, 129,
    153, 182, 216, 256, 303, 359, 426, 52, 55, 60, 68, 79, 93, 109, 129, 153, 181, 215, 255, 302, 358, 424,
    503, 62, 65, 71, 80, 93, 110, 130, 153, 181, 215, 255, 302, 358, 424, 503, 596, 74, 77, 85, 96, 111, 131,
    154, 182, 215, 255, 302, 358, 424, 503, 596, 707, 88, 92, 101, 114, 132, 155, 183, 216, 255, 302, 358,
    424, 503, 596, 707, 838, 104, 109, 120, 135, 157, 184, 217, 256, 302, 358, 424, 503, 596, 707, 838, 994,
    124, 130, 142, 160, 186, 219, 257, 303, 358, 424, 503, 596, 707, 838, 994, 1179, 147, 154, 169, 190, 221,
    260, 305, 359, 424, 503, 596, 707, 838, 994, 1179, 1398, 175, 183, 201, 226, 262, 308, 362, 426, 503,
    596, 707, 838, 994, 1179, 1398, 1658,
];

/// How strongly the weight matrix deviates from flat (0 = flat, 1 = full table).
const MATRIX_STRENGTH: f32 = 0.6;

fn weighted_step(step: f32, u: usize, v: usize, chroma: bool) -> f32 {
    let index = v * N + u;
    let weight = if chroma {
        QUANT16_CHROMA[index]
    } else {
        QUANT16_LUMA[index]
    };
    let base = 16.0 + (f32::from(weight) - 16.0) * MATRIX_STRENGTH;
    step * base / 16.0
}

// WHY: explicit stream vectors keep the per-section byte accounting unambiguous.
#[allow(clippy::too_many_arguments)]
fn encode_block(
    block: &[f32; N * N],
    step: f32,
    dc_out: &mut Vec<u8>,
    count_out: &mut Vec<u8>,
    pos_out: &mut Vec<u8>,
    val_out: &mut Vec<u8>,
    chroma: bool,
    previous_dc: i16,
) -> i16 {
    let z = zigzag();
    let transformed = dct16(block);
    let mut ordered = [0i16; N * N];
    for i in 0..N * N {
        let index = z[i];
        let u = index % N;
        let v = index / N;
        ordered[i] = (transformed[index] / weighted_step(step, u, v, chroma))
            .round()
            .clamp(-32768.0, 32767.0) as i16;
    }
    put_uvarint(dc_out, zigzag_i32(i32::from(ordered[0]) - i32::from(previous_dc)));
    let mut nz = Vec::new();
    for (i, value) in ordered.iter().enumerate().skip(1) {
        if *value != 0 {
            nz.push((i, *value));
        }
    }
    count_out.push(nz.len() as u8);
    for (position, value) in nz {
        pos_out.push(position as u8);
        put_uvarint(val_out, zigzag_i16(value));
    }
    ordered[0]
}

// WHY: the stream cursors must remain explicit to validate section boundaries.
#[allow(clippy::too_many_arguments)]
fn decode_block(
    dc_data: &[u8],
    dc_cursor: &mut usize,
    count: u8,
    pos_data: &[u8],
    pos_cursor: &mut usize,
    val_data: &[u8],
    val_cursor: &mut usize,
    previous_dc: i16,
) -> Result<([i16; N * N], i16)> {
    let diff = unzigzag_i32(read_uvarint(dc_data, dc_cursor)?);
    let dc = (i32::from(previous_dc) + diff).clamp(-32768, 32767) as i16;
    let mut ordered = [0i16; N * N];
    ordered[0] = dc;
    for _ in 0..count {
        let position = *pos_data
            .get(*pos_cursor)
            .ok_or_else(|| Error("truncated DCT16 position".into()))? as usize;
        *pos_cursor += 1;
        let value = unzigzag_i16(read_uvarint(val_data, val_cursor)?);
        if position < N * N {
            ordered[position] = value;
        }
    }
    Ok((ordered, dc))
}

fn write_header(width: u32, height: u32, step: f32, file_size: u64, sections: u16) -> Vec<u8> {
    let mut header = vec![0u8; HEADER_SIZE];
    header[0..8].copy_from_slice(MAGIC);
    header[8..10].copy_from_slice(&VERSION_MAJOR.to_le_bytes());
    header[10..12].copy_from_slice(&VERSION_MINOR.to_le_bytes());
    header[12..14].copy_from_slice(&(HEADER_SIZE as u16).to_le_bytes());
    header[14] = 4;
    header[15] = 6;
    header[16..20].copy_from_slice(&width.to_le_bytes());
    header[20..24].copy_from_slice(&height.to_le_bytes());
    header[24..26].copy_from_slice(&sections.to_le_bytes());
    header[26..28].copy_from_slice(&BASIS_DCT16.to_le_bytes());
    header[28..32].copy_from_slice(&step.to_le_bytes());
    header[32..36].copy_from_slice(&(HEADER_SIZE as u32).to_le_bytes());
    header[36..44].copy_from_slice(&file_size.to_le_bytes());
    header[48] = 1;
    header[49] = 0;
    header[44..48].fill(0);
    let crc = crc32c(&header);
    header[44..48].copy_from_slice(&crc.to_le_bytes());
    header
}

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

pub fn encode16(image: &RgbaImage, step: f32) -> Result<Vec<u8>> {
    encode16_with(image, step, super::Entropy::Zstd)
}

pub fn encode16_with(image: &RgbaImage, step: f32, entropy: super::Entropy) -> Result<Vec<u8>> {
    let width = image.width();
    let height = image.height();
    if width == 0 || height == 0 {
        return Err(Error("empty image".into()));
    }
    let tiles_x = width.div_ceil(TILE_SIZE);
    let tiles_y = height.div_ceil(TILE_SIZE);
    let luma_bx = (tiles_x * 4) as usize;
    let luma_by = (tiles_y * 4) as usize;
    let chroma_bx = (tiles_x * 2) as usize;
    let chroma_by = (tiles_y * 2) as usize;
    let mut luma_grid = vec![0i16; luma_bx * luma_by];
    let mut cb_grid = vec![0i16; chroma_bx * chroma_by];
    let mut cr_grid = vec![0i16; chroma_bx * chroma_by];
    let mut dc_luma = Vec::new();
    let mut dc_chroma = Vec::new();
    let mut counts = Vec::new();
    let mut positions = Vec::new();
    let mut values = Vec::new();
    for ty in 0..tiles_y {
        for tx in 0..tiles_x {
            let px = tx * TILE_SIZE;
            let py = ty * TILE_SIZE;
            for by in 0..4 {
                for bx in 0..4 {
                    let mut block = [0.0f32; N * N];
                    for y in 0..N {
                        for x in 0..N {
                            let sx = (px + bx * BLOCK + x as u32).min(width - 1);
                            let sy = (py + by * BLOCK + y as u32).min(height - 1);
                            let p = image.get_pixel(sx, sy).0;
                            let ycc = rgb_to_ycbcr(f32::from(p[0]), f32::from(p[1]), f32::from(p[2]));
                            block[y * N + x] = ycc[0] - 128.0;
                        }
                    }
                    let gx = (tx * 4 + bx) as usize;
                    let gy = (ty * 4 + by) as usize;
                    let left = (gx > 0).then(|| luma_grid[gy * luma_bx + gx - 1]);
                    let top = (gy > 0).then(|| luma_grid[(gy - 1) * luma_bx + gx]);
                    let top_left = (gx > 0 && gy > 0).then(|| luma_grid[(gy - 1) * luma_bx + gx - 1]);
                    let predicted = med_predict(left, top, top_left);
                    let actual = encode_block(
                        &block,
                        step,
                        &mut dc_luma,
                        &mut counts,
                        &mut positions,
                        &mut values,
                        false,
                        predicted,
                    );
                    luma_grid[gy * luma_bx + gx] = actual;
                }
            }
            for channel in 1..3 {
                let grid = if channel == 1 { &mut cb_grid } else { &mut cr_grid };
                for by in 0..2 {
                    for bx in 0..2 {
                        let mut block = [0.0f32; N * N];
                        for y in 0..N {
                            for x in 0..N {
                                let mut sum = 0.0f32;
                                for oy in 0..2 {
                                    for ox in 0..2 {
                                        let sx = (px + bx * 32 + x as u32 * 2 + ox).min(width - 1);
                                        let sy = (py + by * 32 + y as u32 * 2 + oy).min(height - 1);
                                        let p = image.get_pixel(sx, sy).0;
                                        let ycc =
                                            rgb_to_ycbcr(f32::from(p[0]), f32::from(p[1]), f32::from(p[2]));
                                        sum += ycc[channel];
                                    }
                                }
                                block[y * N + x] = sum / 4.0 - 128.0;
                            }
                        }
                        let gx = (tx * 2 + bx) as usize;
                        let gy = (ty * 2 + by) as usize;
                        let left = (gx > 0).then(|| grid[gy * chroma_bx + gx - 1]);
                        let top = (gy > 0).then(|| grid[(gy - 1) * chroma_bx + gx]);
                        let top_left = (gx > 0 && gy > 0).then(|| grid[(gy - 1) * chroma_bx + gx - 1]);
                        let predicted = med_predict(left, top, top_left);
                        let actual = encode_block(
                            &block,
                            step,
                            &mut dc_chroma,
                            &mut counts,
                            &mut positions,
                            &mut values,
                            true,
                            predicted,
                        );
                        grid[gy * chroma_bx + gx] = actual;
                    }
                }
            }
        }
    }
    let basis = {
        let mut data = Vec::new();
        put_u16(&mut data, BASIS_DCT16);
        put_u16(&mut data, 16);
        put_u16(&mut data, 16);
        put_u16(&mut data, 0);
        data
    };
    let pack = |data: &[u8]| -> Result<(Vec<u8>, u8)> {
        match entropy {
            super::Entropy::Zstd => Ok((
                zstd::bulk::compress(data, 19).map_err(|e| Error(e.to_string()))?,
                super::CODEC_ZSTD,
            )),
            super::Entropy::Rans => Ok((super::rans::encode(data), super::CODEC_RANS)),
            super::Entropy::Best => super::pack_best(data),
        }
    };
    let (dc_luma_data, dc_luma_codec) = pack(&dc_luma)?;
    let (dc_chroma_data, dc_chroma_codec) = pack(&dc_chroma)?;
    let (count_data, count_codec) = pack(&counts)?;
    let (pos_data, pos_codec) = pack(&positions)?;
    let (val_data, val_codec) = pack(&values)?;
    let basis_data = zstd::bulk::compress(&basis, 19).map_err(|e| Error(e.to_string()))?;
    let sections = [
        (SECTION_BASIS, basis_data, basis.len() as u64, &basis[..], 1u8),
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
            count_data,
            counts.len() as u64,
            &counts[..],
            count_codec,
        ),
        (
            SECTION_AC_POS,
            pos_data,
            positions.len() as u64,
            &positions[..],
            pos_codec,
        ),
        (
            SECTION_AC_VAL,
            val_data,
            values.len() as u64,
            &values[..],
            val_codec,
        ),
    ];
    let table_len = HEADER_SIZE + sections.len() * SECTION_ENTRY;
    let mut file = vec![0u8; table_len];
    let mut offset = table_len;
    for (index, (kind, stored, raw, raw_bytes, codec)) in sections.iter().enumerate() {
        let entry = HEADER_SIZE + index * SECTION_ENTRY;
        file[entry..entry + 2].copy_from_slice(&kind.to_le_bytes());
        file[entry + 2] = *codec;
        file[entry + 4..entry + 8].copy_from_slice(&crc32c(raw_bytes).to_le_bytes());
        file[entry + 8..entry + 16].copy_from_slice(&(offset as u64).to_le_bytes());
        file[entry + 16..entry + 24].copy_from_slice(&(stored.len() as u64).to_le_bytes());
        file[entry + 24..entry + 32].copy_from_slice(&raw.to_le_bytes());
        file.extend_from_slice(stored);
        offset += stored.len();
    }
    let h = write_header(width, height, step, file.len() as u64, sections.len() as u16);
    file[..HEADER_SIZE].copy_from_slice(&h);
    Ok(file)
}

pub fn decode16(data: &[u8]) -> Result<RgbaImage> {
    if data.len() < HEADER_SIZE || &data[0..8] != MAGIC {
        return Err(Error("not a new RFI file".into()));
    }
    if read_u16(data, 26)? != BASIS_DCT16 {
        return Err(Error("not a DCT16 file".into()));
    }
    let width = read_u32(data, 16)?;
    let height = read_u32(data, 20)?;
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
    let section_count = read_u16(data, 24)? as usize;
    let mut dc_luma = None;
    let mut dc_chroma = None;
    let mut counts = None;
    let mut positions = None;
    let mut values = None;
    for index in 0..section_count {
        let entry = HEADER_SIZE + index * SECTION_ENTRY;
        let kind = read_u16(data, entry)?;
        let offset = read_u32(data, entry + 8)? as usize;
        let len = read_u32(data, entry + 16)? as usize;
        let raw = read_u64(data, entry + 24)? as usize;
        if offset + len > data.len() {
            return Err(Error("section outside file".into()));
        }
        let codec = data.get(entry + 2).copied().unwrap_or(1);
        let decoded = match codec {
            2 => super::rans::decode(&data[offset..offset + len]).map_err(Error)?,
            _ => {
                zstd::bulk::decompress(&data[offset..offset + len], raw).map_err(|e| Error(e.to_string()))?
            }
        };
        if crc32c(&decoded) != read_u32(data, entry + 4)? {
            return Err(Error("section CRC mismatch".into()));
        }
        match kind {
            SECTION_COEFF_DC => dc_luma = Some(decoded),
            SECTION_DC_CHROMA => dc_chroma = Some(decoded),
            SECTION_AC_COUNT => counts = Some(decoded),
            SECTION_AC_POS => positions = Some(decoded),
            SECTION_AC_VAL => values = Some(decoded),
            _ => {}
        }
    }
    let dc_luma = dc_luma.ok_or_else(|| Error("missing DCT16 luma DC".into()))?;
    let dc_chroma = dc_chroma.ok_or_else(|| Error("missing DCT16 chroma DC".into()))?;
    let counts = counts.ok_or_else(|| Error("missing DCT16 counts".into()))?;
    let positions = positions.ok_or_else(|| Error("missing DCT16 positions".into()))?;
    let values = values.ok_or_else(|| Error("missing DCT16 values".into()))?;
    let mut image = RgbaImage::new(width, height);
    let mut dc_luma_cursor = 0usize;
    let mut dc_chroma_cursor = 0usize;
    let mut count_cursor = 0usize;
    let mut pos_cursor = 0usize;
    let mut val_cursor = 0usize;
    let tiles_x = width.div_ceil(TILE_SIZE);
    let tiles_y = height.div_ceil(TILE_SIZE);
    let luma_bx = (tiles_x * 4) as usize;
    let luma_by = (tiles_y * 4) as usize;
    let chroma_bx = (tiles_x * 2) as usize;
    let chroma_by = (tiles_y * 2) as usize;
    let mut luma_grid = vec![0i16; luma_bx * luma_by];
    let mut cb_grid = vec![0i16; chroma_bx * chroma_by];
    let mut cr_grid = vec![0i16; chroma_bx * chroma_by];
    for ty in 0..tiles_y {
        for tx in 0..tiles_x {
            let px = tx * TILE_SIZE;
            let py = ty * TILE_SIZE;
            let tw = TILE_SIZE.min(width - px);
            let th = TILE_SIZE.min(height - py);
            let mut y_plane = [128.0f32; 64 * 64];
            for by in 0..4 {
                for bx in 0..4 {
                    let gx = (tx * 4 + bx) as usize;
                    let gy = (ty * 4 + by) as usize;
                    let left = (gx > 0).then(|| luma_grid[gy * luma_bx + gx - 1]);
                    let top = (gy > 0).then(|| luma_grid[(gy - 1) * luma_bx + gx]);
                    let top_left = (gx > 0 && gy > 0).then(|| luma_grid[(gy - 1) * luma_bx + gx - 1]);
                    let predicted = med_predict(left, top, top_left);
                    let count = *counts
                        .get(count_cursor)
                        .ok_or_else(|| Error("truncated DCT16 count".into()))?;
                    count_cursor += 1;
                    let (ordered, next) = decode_block(
                        &dc_luma,
                        &mut dc_luma_cursor,
                        count,
                        &positions,
                        &mut pos_cursor,
                        &values,
                        &mut val_cursor,
                        predicted,
                    )?;
                    luma_grid[gy * luma_bx + gx] = next;
                    let z = zigzag();
                    let mut transformed = [0.0f32; N * N];
                    for i in 0..N * N {
                        let index = z[i];
                        let u = index % N;
                        let v = index / N;
                        transformed[index] = f32::from(ordered[i]) * weighted_step(step, u, v, false);
                    }
                    let block = idct16(&transformed);
                    for y in 0..N {
                        for x in 0..N {
                            let idx = (by as usize * N + y) * 64 + bx as usize * N + x;
                            y_plane[idx] = block[y * N + x] + 128.0;
                        }
                    }
                }
            }
            let mut cb_plane = [128.0f32; 32 * 32];
            let mut cr_plane = [128.0f32; 32 * 32];
            for (channel, plane) in [&mut cb_plane, &mut cr_plane].into_iter().enumerate() {
                let grid = if channel == 1 { &mut cb_grid } else { &mut cr_grid };
                for by in 0..2 {
                    for bx in 0..2 {
                        let gx = (tx * 2 + bx) as usize;
                        let gy = (ty * 2 + by) as usize;
                        let left = (gx > 0).then(|| grid[gy * chroma_bx + gx - 1]);
                        let top = (gy > 0).then(|| grid[(gy - 1) * chroma_bx + gx]);
                        let top_left = (gx > 0 && gy > 0).then(|| grid[(gy - 1) * chroma_bx + gx - 1]);
                        let predicted = med_predict(left, top, top_left);
                        let count = *counts
                            .get(count_cursor)
                            .ok_or_else(|| Error("truncated DCT16 count".into()))?;
                        count_cursor += 1;
                        let (ordered, next) = decode_block(
                            &dc_chroma,
                            &mut dc_chroma_cursor,
                            count,
                            &positions,
                            &mut pos_cursor,
                            &values,
                            &mut val_cursor,
                            predicted,
                        )?;
                        grid[gy * chroma_bx + gx] = next;
                        let z = zigzag();
                        let mut transformed = [0.0f32; N * N];
                        for i in 0..N * N {
                            let index = z[i];
                            let u = index % N;
                            let v = index / N;
                            transformed[index] = f32::from(ordered[i]) * weighted_step(step, u, v, true);
                        }
                        let block = idct16(&transformed);
                        for y in 0..N {
                            for x in 0..N {
                                let idx = (by as usize * N + y) * 32 + bx as usize * N + x;
                                plane[idx] = block[y * N + x] + 128.0;
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
                    let x0 = cx.floor() as usize;
                    let y0 = cy.floor() as usize;
                    let x1 = (x0 + 1).min(31);
                    let y1 = (y0 + 1).min(31);
                    let fx = cx - x0 as f32;
                    let fy = cy - y0 as f32;
                    let sample = |plane: &[f32; 32 * 32]| {
                        let a = plane[y0 * 32 + x0];
                        let b = plane[y0 * 32 + x1];
                        let c = plane[y1 * 32 + x0];
                        let d = plane[y1 * 32 + x1];
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

pub fn encode16_to_budget(image: &RgbaImage, target: usize) -> Result<(f32, Vec<u8>)> {
    encode16_to_budget_with(image, target, super::Entropy::Zstd)
}

pub fn encode16_to_budget_with(
    image: &RgbaImage,
    target: usize,
    entropy: super::Entropy,
) -> Result<(f32, Vec<u8>)> {
    let mut low = 0.25f32;
    let mut high = 2000.0f32;
    let high_bytes = encode16_with(image, high, entropy)?;
    if high_bytes.len() > target {
        return Ok((high, high_bytes));
    }
    let mut best_step = high;
    let mut best = high_bytes;
    for _ in 0..18 {
        let mid = 0.5 * (low + high);
        let bytes = encode16_with(image, mid, entropy)?;
        if bytes.len() <= target {
            best_step = mid;
            best = bytes;
            high = mid;
        } else {
            low = mid;
        }
    }
    Ok((best_step, best))
}
#[cfg(test)]
mod tests {
    use image::{Rgba, RgbaImage};

    use super::N;

    #[test]
    fn transform_round_trip() {
        let mut block = [0.0f32; N * N];
        for y in 0..N {
            for x in 0..N {
                block[y * N + x] = ((x * 7 + y * 3) % 255) as f32 - 128.0;
            }
        }
        let original = block;
        let transformed = super::dct16(&block);
        let restored = super::idct16(&transformed);
        let max = original
            .iter()
            .zip(restored.iter())
            .map(|(a, b)| (a - b).abs())
            .fold(0.0f32, f32::max);
        println!("transform max {max}");
        assert!(max < 1e-3, "max {max}");
    }

    #[test]
    fn image_round_trip() {
        let mut image = RgbaImage::new(64, 64);
        for y in 0..64 {
            for x in 0..64 {
                image.put_pixel(x, y, Rgba([(x * 4) as u8, (y * 4) as u8, 128, 255]));
            }
        }
        let bytes = super::encode16(&image, 10.0).unwrap();
        let decoded = super::decode16(&bytes).unwrap();
        let mut sum = 0.0f64;
        for (a, b) in image.pixels().zip(decoded.pixels()) {
            for c in 0..4 {
                let d = f64::from(a.0[c]) - f64::from(b.0[c]);
                sum += d * d;
            }
        }
        let mse = sum / (64.0 * 64.0 * 4.0);
        let psnr = 10.0 * (255.0f64 * 255.0 / mse).log10();
        println!("dct16 psnr {psnr} bytes {}", bytes.len());
        assert!(psnr > 30.0, "psnr {psnr}");
    }
}
