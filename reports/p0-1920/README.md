# P0: coefficient-stream analysis (1920x1080)

Input: real 1920x1080 JPEG (386,694 B). Rate control searches qstep by the final
serialized file size, including header, section table, basis, tile directory and
zstd coefficient section. All points are at or below the target.

| budget | qstep | actual bytes | RGB PSNR | Y | Cb | Cr | SSIM | MS-SSIM | blockiness |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 25,000 | 384.28 | 25,000 | 19.16 | 19.28 | 26.24 | 25.66 | 0.5520 | 0.7149 | 4.704 |
| 50,000 | 232.19 | 49,983 | 20.91 | 21.62 | 26.85 | 26.63 | 0.6773 | 0.8268 | 2.721 |
| 100,000 | 130.85 | 99,991 | 24.24 | 24.48 | 31.00 | 31.13 | 0.7711 | 0.8985 | 1.908 |
| 200,000 | 71.78 | 199,954 | 27.76 | 28.05 | 34.62 | 34.61 | 0.8668 | 0.9510 | 1.396 |

SSIM is 8x8-window luma SSIM; MS-SSIM is the 5-scale weighted version. These are
implemented approximations, not a reference codec implementation.

## Section bytes

| budget | header+table | basis | tile stored/raw | coeff stored/raw |
|---:|---:|---:|---:|---:|
| 25,000 | 192 | 17 | 2,899 / 16,320 | 21,892 / 270,858 |
| 50,000 | 192 | 17 | 3,103 / 16,320 | 46,671 / 329,094 |
| 100,000 | 192 | 17 | 3,315 / 16,320 | 96,467 / 462,168 |
| 200,000 | 192 | 17 | 3,465 / 16,320 | 196,280 / 744,594 |

At 50 KB the stored coefficient section is 93.4% of the file, not 42 KB of a
hypothetical 50 KB; it is 46,671 B. The tile directory is 3,103 B (6.2%), not
16,320 B (32.6%); zstd compresses the 510 regular 32-byte records heavily.

## 50 KB channel split

Per-channel zstd is independently compressed for analysis; it is an estimate,
while the actual format compresses the whole coefficient stream as one section.

| channel | raw bytes | independent zstd | DC zstd | AC zstd | nonzero ratio | avg zero run |
|---|---:|---:|---:|---:|---:|---:|
| Y | 247,494 | 44,250 | 10,997 | 30,354 | 2.738% | 38.18 |
| Cb | 40,800 | 575 | 547 | 19 | 0.107% | 63.00 |
| Cr | 40,800 | 728 | 708 | 19 | 0.170% | 63.00 |

At 50 KB:

- Y is 94.8% of the coefficient section and 88.5% of the whole file.
- Y AC is 30,354 B (60.7% of the file): this is the actual detail stream.
- Y DC is 10,997 B (22.0% of the file): low-frequency base, still significant.
- Chroma is almost entirely DC: 1,303 B total. Do not optimize chroma AC first.
- Tile directory 3,103 B: maximum practical saving is about 2 KB here.
- Header + table 192 B: not relevant.

## What is "valuable" vs "redundancy"?

The statistics cannot label a byte as valuable without a distortion model, but
the split is clear enough:

1. Y AC is the only large high-frequency stream. At 50 KB it is 30,354 B and
   sparse: 2.7% non-zero, average zero run 38.2. Entropy coding and frequency
   contexts can target this.
2. Y DC is 10,997 B for 32,640 blocks. Better DC prediction/entropy is a
   real target, not just AC.
3. Chroma AC is effectively dead at 50 KB (19 B each); chroma is not the
   bottleneck at this rate.
4. Tile directory is 3.1 KB; compacting it can save maybe 2 KB, but it cannot
   move PSNR much.
5. At 50 KB, blockiness is 2.72 and SSIM 0.677. The low-rate ceiling is
   dominated by quantization, not by directory overhead.

## Priority implication

- P1: frequency-weighted quantization matrix, especially luma; independent
  luma/chroma matrices.
- P2: DC/AC separated entropy coding with frequency contexts.
- P3: already implemented true serialized-byte rate control.
- P4: compact tile directory.
- P5: wavelet after the DCT pipeline is squeezed.

## Files

- p0.md: generated summary
- channels.csv: per-budget channel statistics
- freq.csv: per-zigzag-position non-zero counts
- tiles.csv: per-tile AC energy, non-zero ratio, zero run, raw bytes, MSE
- budget_*_diff.png: difference maps
