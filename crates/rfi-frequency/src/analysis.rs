//! P0 coefficient-stream instrumentation.

use image::RgbaImage;

use super::{
    BLOCKS_PER_TILE, Error, QUANT_CHROMA, QUANT_LUMA, Result, TILE_SIZE, dct8, decode, encode, encode_block,
    quantize, read_u16, read_u64, rgb_to_ycbcr,
};

#[derive(Debug, Clone)]
pub struct ChannelAnalysis {
    pub blocks: u64,
    pub coefficients: u64,
    pub dc_nonzero: u64,
    pub ac_nonzero: u64,
    pub dc_abs_sum: f64,
    pub ac_abs_sum: f64,
    pub positive: u64,
    pub negative: u64,
    pub zero_run_count: u64,
    pub zero_run_total: u64,
    pub raw_bytes: u64,
    pub zstd_bytes: u64,
    pub dc_zstd_bytes: u64,
    pub ac_zstd_bytes: u64,
    pub dc_bytes: u64,
    pub ac_bytes: u64,
    pub freq_nonzero: [u64; 64],
}

impl Default for ChannelAnalysis {
    fn default() -> Self {
        Self {
            blocks: 0,
            coefficients: 0,
            dc_nonzero: 0,
            ac_nonzero: 0,
            dc_abs_sum: 0.0,
            ac_abs_sum: 0.0,
            positive: 0,
            negative: 0,
            zero_run_count: 0,
            zero_run_total: 0,
            raw_bytes: 0,
            zstd_bytes: 0,
            dc_zstd_bytes: 0,
            ac_zstd_bytes: 0,
            dc_bytes: 0,
            ac_bytes: 0,
            freq_nonzero: [0; 64],
        }
    }
}

impl ChannelAnalysis {
    pub fn nonzero_ratio(&self) -> f64 {
        if self.coefficients == 0 {
            0.0
        } else {
            (self.dc_nonzero + self.ac_nonzero) as f64 / self.coefficients as f64
        }
    }

    pub fn average_zero_run(&self) -> f64 {
        if self.zero_run_count == 0 {
            0.0
        } else {
            self.zero_run_total as f64 / self.zero_run_count as f64
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct TileAnalysis {
    pub tile_x: u32,
    pub tile_y: u32,
    pub y_ac_energy: f64,
    pub cb_ac_energy: f64,
    pub cr_ac_energy: f64,
    pub nonzero_ratio: f64,
    pub zero_run_count: u64,
    pub zero_run_total: u64,
    pub average_zero_run: f64,
    pub raw_bytes: u64,
    pub stored_bytes_estimate: u64,
    pub mse: f64,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SectionSizes {
    pub file_bytes: u64,
    pub header_bytes: u64,
    pub section_table_bytes: u64,
    pub basis_stored: u64,
    pub tile_directory_stored: u64,
    pub tile_directory_raw: u64,
    pub coefficients_stored: u64,
    pub coefficients_raw: u64,
}

#[derive(Debug, Clone)]
pub struct Analysis {
    pub step: f32,
    pub width: u32,
    pub height: u32,
    pub tiles: Vec<TileAnalysis>,
    pub channels: [ChannelAnalysis; 3],
    pub sections: SectionSizes,
}

pub fn analyze(image: &RgbaImage, step: f32) -> Result<Analysis> {
    let bytes = encode(image, step)?;
    analyze_bytes(image, step, &bytes)
}

pub fn analyze_bytes(image: &RgbaImage, step: f32, bytes: &[u8]) -> Result<Analysis> {
    let width = image.width();
    let height = image.height();
    let decoded = decode(bytes)?;
    let section_count = read_u16(bytes, 24)? as usize;
    let mut sections = SectionSizes {
        file_bytes: bytes.len() as u64,
        header_bytes: super::HEADER_SIZE as u64,
        section_table_bytes: (section_count * super::SECTION_ENTRY) as u64,
        ..SectionSizes::default()
    };
    for index in 0..section_count {
        let entry = super::HEADER_SIZE + index * super::SECTION_ENTRY;
        let kind = read_u16(bytes, entry)?;
        let stored = read_u64(bytes, entry + 16)?;
        let raw = read_u64(bytes, entry + 24)?;
        match kind {
            super::SECTION_BASIS => sections.basis_stored = stored,
            super::SECTION_TILES => {
                sections.tile_directory_stored = stored;
                sections.tile_directory_raw = raw;
            }
            super::SECTION_COEFF_DC
            | super::SECTION_DC_CHROMA
            | super::SECTION_AC_COUNT
            | super::SECTION_AC_EXT
            | super::SECTION_AC_POS
            | super::SECTION_AC_VAL => {
                sections.coefficients_stored += stored;
                sections.coefficients_raw += raw;
            }
            _ => {}
        }
    }
    let tiles_x = width.div_ceil(TILE_SIZE);
    let tiles_y = height.div_ceil(TILE_SIZE);
    let mut channels = [
        ChannelAnalysis::default(),
        ChannelAnalysis::default(),
        ChannelAnalysis::default(),
    ];
    let mut streams: [Vec<u8>; 3] = std::array::from_fn(|_| Vec::new());
    let mut dc_streams: [Vec<u8>; 3] = std::array::from_fn(|_| Vec::new());
    let mut ac_streams: [Vec<u8>; 3] = std::array::from_fn(|_| Vec::new());
    let mut tiles = Vec::with_capacity((tiles_x * tiles_y) as usize);
    for ty in 0..tiles_y {
        for tx in 0..tiles_x {
            let px = tx * TILE_SIZE;
            let py = ty * TILE_SIZE;
            let tw = TILE_SIZE.min(width - px);
            let th = TILE_SIZE.min(height - py);
            let mut tile = TileAnalysis {
                tile_x: tx,
                tile_y: ty,
                ..TileAnalysis::default()
            };
            // Luma: 8x8 blocks.
            let mut prev_dc = 0i16;
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
                    let coeff = quantize(&dct8(&block), step, &QUANT_LUMA);
                    let mut tmp = Vec::new();
                    let previous_dc = prev_dc;
                    prev_dc = encode_block(&coeff, &mut tmp, prev_dc);
                    let raw = tmp.len() as u64;
                    let dc_len = super::dc_varint_len(coeff[0], previous_dc);
                    accumulate(&mut channels[0], &coeff, raw, dc_len);
                    streams[0].extend_from_slice(&tmp);
                    dc_streams[0].extend_from_slice(&tmp[..dc_len]);
                    ac_streams[0].extend_from_slice(&tmp[dc_len..]);
                    tile.y_ac_energy += ac_energy(&coeff);
                    tile.raw_bytes += raw;
                    tile.nonzero_ratio += nonzero_count(&coeff) as f64;
                    let (runs, values) = zero_runs(&coeff);
                    tile.zero_run_count += runs;
                    tile.zero_run_total += values;
                }
            }
            // Chroma: 4:2:0, 32x32 per tile.
            for channel in 1..3 {
                let mut prev_dc = 0i16;
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
                        let coeff = quantize(&dct8(&block), step, &QUANT_CHROMA);
                        let mut tmp = Vec::new();
                        let previous_dc = prev_dc;
                        prev_dc = encode_block(&coeff, &mut tmp, prev_dc);
                        let raw = tmp.len() as u64;
                        let dc_len = super::dc_varint_len(coeff[0], previous_dc);
                        accumulate(&mut channels[channel], &coeff, raw, dc_len);
                        streams[channel].extend_from_slice(&tmp);
                        dc_streams[channel].extend_from_slice(&tmp[..dc_len]);
                        ac_streams[channel].extend_from_slice(&tmp[dc_len..]);
                        let energy = ac_energy(&coeff);
                        if channel == 1 {
                            tile.cb_ac_energy += energy;
                        } else {
                            tile.cr_ac_energy += energy;
                        }
                        tile.raw_bytes += raw;
                        tile.nonzero_ratio += nonzero_count(&coeff) as f64;
                        let (runs, values) = zero_runs(&coeff);
                        tile.zero_run_count += runs;
                        tile.zero_run_total += values;
                    }
                }
            }
            // Normalize per-tile values.
            let blocks = (BLOCKS_PER_TILE * BLOCKS_PER_TILE + 32) as f64;
            tile.nonzero_ratio /= blocks * 64.0;
            tile.average_zero_run = if tile.zero_run_count == 0 {
                0.0
            } else {
                tile.zero_run_total as f64 / tile.zero_run_count as f64
            };
            // Reconstruction error for this tile.
            let mut sum = 0.0f64;
            for y in py..py + th {
                for x in px..px + tw {
                    let a = image.get_pixel(x, y).0;
                    let b = decoded.get_pixel(x, y).0;
                    for c in 0..4 {
                        let diff = f64::from(a[c]) - f64::from(b[c]);
                        sum += diff * diff;
                    }
                }
            }
            tile.mse = sum / f64::from(tw * th * 4);
            tiles.push(tile);
        }
    }
    for (index, channel) in channels.iter_mut().enumerate() {
        channel.zstd_bytes = zstd::bulk::compress(&streams[index], 9)
            .map_err(|e| Error(e.to_string()))?
            .len() as u64;
        channel.dc_zstd_bytes = zstd::bulk::compress(&dc_streams[index], 9)
            .map_err(|e| Error(e.to_string()))?
            .len() as u64;
        channel.ac_zstd_bytes = zstd::bulk::compress(&ac_streams[index], 9)
            .map_err(|e| Error(e.to_string()))?
            .len() as u64;
    }
    let total_raw: u64 = tiles.iter().map(|tile| tile.raw_bytes).sum();
    for tile in &mut tiles {
        tile.stored_bytes_estimate = (tile.raw_bytes * sections.coefficients_stored)
            .checked_div(total_raw)
            .unwrap_or(0);
    }
    Ok(Analysis {
        step,
        width,
        height,
        tiles,
        channels,
        sections,
    })
}

fn accumulate(channel: &mut ChannelAnalysis, coeff: &[i16; 64], raw_bytes: u64, dc_bytes: usize) {
    channel.blocks += 1;
    channel.coefficients += 64;
    channel.raw_bytes += raw_bytes;
    channel.dc_bytes += dc_bytes as u64;
    channel.ac_bytes += raw_bytes.saturating_sub(dc_bytes as u64);
    let dc = coeff[0];
    if dc != 0 {
        channel.dc_nonzero += 1;
    }
    channel.dc_abs_sum += f64::from(dc.unsigned_abs());
    let mut run = 0u64;
    for (position, value) in coeff.iter().enumerate().skip(1) {
        if *value != 0 {
            channel.ac_nonzero += 1;
            channel.ac_abs_sum += f64::from(value.unsigned_abs());
            channel.freq_nonzero[position] += 1;
            if *value > 0 {
                channel.positive += 1;
            } else {
                channel.negative += 1;
            }
            if run > 0 {
                channel.zero_run_count += 1;
                channel.zero_run_total += run;
                run = 0;
            }
        } else {
            run += 1;
        }
    }
    if run > 0 {
        channel.zero_run_count += 1;
        channel.zero_run_total += run;
    }
}

fn zero_runs(coeff: &[i16; 64]) -> (u64, u64) {
    let mut runs = 0u64;
    let mut values = 0u64;
    let mut run = 0u64;
    for value in coeff.iter().skip(1) {
        if *value == 0 {
            run += 1;
        } else if run > 0 {
            runs += 1;
            values += run;
            run = 0;
        }
    }
    if run > 0 {
        runs += 1;
        values += run;
    }
    (runs, values)
}

fn ac_energy(coeff: &[i16; 64]) -> f64 {
    coeff[1..]
        .iter()
        .map(|value| f64::from(*value) * f64::from(*value))
        .sum()
}

fn nonzero_count(coeff: &[i16; 64]) -> u32 {
    coeff.iter().filter(|value| **value != 0).count() as u32
}
