# P2dc: DC/AC bit allocation

Isolated matrix change on top of P2run: DC coefficients get a finer quantization
weight (luma DC weight 16 -> 8, chroma 17 -> 9). RGB/AC matrix weights are
unchanged.

This spends more bits on the low-frequency DC base and fewer on AC, while the
final serialized size is still held by the true rate controller.

## P2run -> P2dc

| budget | P2run bytes | P2run RGB | P2dc bytes | P2dc RGB | P2run SSIM | P2dc SSIM | P2run MS | P2dc MS | P2run block | P2dc block |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 25K | 25,000 | 20.70 | 25,000 | **21.65** | 0.6276 | **0.6376** | 0.7983 | **0.8456** | 3.584 | 3.922 |
| 50K | 49,871 | 23.52 | 49,969 | **23.91** | 0.7490 | **0.7545** | 0.8876 | **0.9202** | 2.369 | 2.439 |
| 100K | 99,859 | 26.29 | 99,984 | **26.49** | 0.8411 | **0.8481** | 0.9414 | **0.9607** | 1.740 | 1.736 |
| 200K | 199,828 | 30.14 | 199,931 | 30.13 | 0.9254 | 0.9246 | 0.9814 | **0.9843** | 1.345 | **1.335** |

PSNR improves strongly at low rates:

- 25K: +0.95 dB
- 50K: +0.39 dB
- 100K: +0.20 dB
- 200K: essentially flat

MS-SSIM improves at every point. SSIM improves at 25/50/100 KB. Blockiness is
worse at 25/50 KB, so this is a real tradeoff, not a free win.

## 50 KB section shift

| section | P2run | P2dc |
|---|---:|---:|
| DC | 9,934 B | 13,462 B |
| AC | 39,728 B | 36,298 B |
| Total | 49,871 B | 49,969 B |

The rate controller moves about 3.5 KB from AC to DC. The DC base improvement is
worth more than the lost AC bits at 25-100 KB.

## Current best

| budget | bytes | RGB | Y | Cb | Cr | SSIM | MS-SSIM | blockiness |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 25K | 25,000 | 21.65 | 21.00 | 31.93 | 31.91 | 0.6376 | 0.8456 | 3.922 |
| 50K | 49,969 | 23.91 | 23.14 | 34.97 | 35.16 | 0.7545 | 0.9202 | 2.439 |
| 100K | 99,984 | 26.49 | 25.73 | 37.59 | 37.45 | 0.8481 | 0.9607 | 1.736 |
| 200K | 199,931 | 30.13 | 29.50 | 40.45 | 40.27 | 0.9246 | 0.9843 | 1.335 |

## Verdict

Keep. The DC/AC allocation is now measurably unbalanced toward DC, and that is
better at 25-100 KB.

## Files

- p0.md, channels.csv, freq.csv, tiles.csv
- budget_*_diff.png
