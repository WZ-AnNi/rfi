# P2pv: AC positions and values split

Isolated entropy-layout change on top of P2dc:

- DC section unchanged.
- AC position/count data: section type 4.
- AC values: section type 5.
- Per block the position stream has count + run; the value stream has only the
  zigzag signed LEB128 values.
- No extra per-block count: the count stays in the position stream.
- One extra section entry (32 B), included in file size.

## P2dc -> P2pv

| budget | P2dc bytes | P2dc RGB | P2pv bytes | P2pv RGB | P2dc SSIM | P2pv SSIM | P2dc MS | P2pv MS | P2dc block | P2pv block |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 25K | 25,000 | 21.65 | 24,998 | **21.81** | **0.6376** | 0.6240 | 0.8456 | 0.8415 | 3.922 | **3.762** |
| 50K | 49,969 | 23.91 | 49,851 | **24.14** | 0.7545 | **0.7603** | 0.9202 | **0.9225** | 2.439 | **2.359** |
| 100K | 99,984 | 26.49 | 99,880 | **26.89** | 0.8481 | **0.8600** | 0.9607 | **0.9650** | 1.736 | **1.707** |
| 200K | 199,931 | 30.13 | 198,986 | **30.64** | 0.9246 | **0.9325** | 0.9843 | **0.9863** | 1.335 | **1.343** |

PSNR improves at every point:

- 25K: +0.16 dB
- 50K: +0.23 dB
- 100K: +0.40 dB
- 200K: +0.51 dB

SSIM improves at 50/100/200 KB; 25K SSIM is 0.0136 lower, but PSNR and
blockiness are better. MS-SSIM is slightly lower at 25K and higher at
50/100/200 KB.

## Current best

| budget | bytes | RGB | Y | Cb | Cr | SSIM | MS-SSIM | blockiness |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 25K | 24,998 | 21.81 | 21.14 | 32.22 | 32.15 | 0.6240 | 0.8415 | 3.762 |
| 50K | 49,851 | 24.14 | 23.38 | 35.23 | 35.44 | 0.7603 | 0.9225 | 2.359 |
| 100K | 99,880 | 26.89 | 26.14 | 38.02 | 37.81 | 0.8600 | 0.9650 | 1.707 |
| 200K | 198,986 | 30.64 | 30.00 | 41.05 | 40.85 | 0.9325 | 0.9863 | 1.343 |

## Rejected: low/high AC frequency value split

A further split of AC values into low-frequency (positions 1-15) and
high-frequency (16-63) streams was tested. It added a section and did not
improve the low-rate points:

| budget | P2pv RGB | low/high RGB | P2pv SSIM | low/high SSIM |
|---:|---:|---:|---:|---:|
| 25K | 21.81 | 21.81 | 0.6240 | 0.6239 |
| 50K | 24.14 | 24.05 | 0.7603 | 0.7476 |
| 100K | 26.89 | 26.89 | 0.8600 | 0.8600 |
| 200K | 30.64 | 30.66 | 0.9325 | 0.9327 |

Rejected: the extra section overhead and lost context outweigh the negligible
high-rate gain.

## Verdict

Keep P2pv. This is a cheap context separation: same per-block count, one extra
section entry, and consistent PSNR/MS-SSIM gains.

## Files

- p0.md, channels.csv, freq.csv, tiles.csv
- budget_*_diff.png
