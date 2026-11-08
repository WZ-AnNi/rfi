# P2m7: luma matrix strength 0.7

Isolated matrix tuning on top of P2pack:

- Luma DC weight stays at 8 (from P2dc).
- Luma AC high-frequency weights use 0.7 of the JPEG table instead of 0.6.
- Chroma matrix and all entropy coding unchanged.

This is a perceptual trade: slightly gentler high-frequency quantization.

## P2pack -> P2m7

| budget | P2pack bytes | P2pack RGB | P2m7 bytes | P2m7 RGB | P2pack SSIM | P2m7 SSIM | P2pack MS | P2m7 MS | P2pack block | P2m7 block |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 25K | 24,982 | 21.85 | 24,974 | 21.83 | 0.6188 | **0.6278** | 0.8390 | **0.8442** | 3.745 | 3.766 |
| 50K | 49,758 | 24.20 | 49,984 | 24.20 | 0.7663 | **0.7682** | 0.9259 | **0.9273** | 2.337 | **2.336** |
| 100K | 99,971 | 27.07 | 99,957 | 26.98 | 0.8624 | **0.8668** | 0.9660 | **0.9678** | 1.663 | 1.671 |
| 200K | 197,095 | 30.73 | 199,119 | 30.59 | 0.9340 | **0.9348** | 0.9866 | **0.9869** | 1.344 | **1.335** |

Reading:

- 50K keeps exactly 24.20 dB and improves SSIM/MS/blockiness.
- 25K loses 0.02 dB but gains 0.009 SSIM and 0.005 MS-SSIM.
- 100K/200K lose 0.09/0.14 dB but gain SSIM/MS-SSIM; 200K blockiness improves.
- This is a perceptual trade, not a PSNR sweep.

## Current best

| budget | bytes | RGB | Y | Cb | Cr | SSIM | MS-SSIM | blockiness |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 25K | 24,974 | 21.83 | 21.16 | 32.27 | 32.18 | 0.6278 | 0.8442 | 3.766 |
| 50K | 49,984 | 24.20 | 23.44 | 35.28 | 35.49 | 0.7682 | 0.9273 | 2.336 |
| 100K | 99,957 | 26.98 | 26.24 | 38.05 | 37.87 | 0.8668 | 0.9678 | 1.671 |
| 200K | 199,119 | 30.59 | 29.94 | 41.07 | 40.87 | 0.9348 | 0.9869 | 1.335 |

## Rejected: 6-bit run packing

Packing AC run lengths into 6 bits was tested. It reduced raw bytes but hurt the
zstd context and was worse at every point:

| budget | P2m7 RGB | 6-bit run RGB |
|---:|---:|---:|
| 25K | 21.83 | 21.67 |
| 50K | 24.20 | 23.96 |
| 100K | 26.98 | 26.73 |
| 200K | 30.59 | 30.39 |

Rejected; run bytes stay 8-bit.

## Verdict

Keep as the current perceptual point. If pure PSNR is the objective, P2pack's
0.6 matrix is slightly better at 100/200 KB; for SSIM/MS-SSIM and 50 KB parity,
0.7 is the better default.

## Files

- p0.md, channels.csv, freq.csv, tiles.csv
- budget_*_diff.png
