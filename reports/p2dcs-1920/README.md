# P2dcs: luma/chroma DC sections

Isolated context split on top of P2dcc:

- Luma DC differences: one zstd section.
- Chroma (Cb and Cr together) DC differences: one zstd section.
- DC prediction remains continuous across tiles.
- No extra per-block bytes; one extra section entry.

## P2dcc -> P2dcs

| budget | P2dcc bytes | P2dcc RGB | P2dcs bytes | P2dcs RGB | P2dcc SSIM | P2dcs SSIM | P2dcc MS | P2dcs MS | P2dcc block | P2dcs block |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 25K | 24,939 | 21.84 | 24,945 | 21.84 | **0.6276** | 0.6268 | **0.8434** | 0.8429 | 3.755 | 3.756 |
| 50K | 49,903 | 24.21 | 49,910 | **24.24** | 0.7688 | **0.7701** | 0.9276 | **0.9287** | 2.333 | **2.324** |
| 100K | 99,792 | 26.99 | 99,988 | **27.01** | 0.8669 | **0.8681** | 0.9678 | **0.9683** | 1.670 | **1.666** |
| 200K | 199,211 | 30.60 | 197,103 | **30.61** | 0.9351 | **0.9353** | 0.9871 | **0.9871** | 1.334 | 1.335 |

Small, consistent gains at 50-200 KB; 25K SSIM is 0.0008 lower. Kept because the
gain is real at the target 50-200 KB range and costs one section entry.

## Current best

| budget | bytes | RGB | Y | Cb | Cr | SSIM | MS-SSIM | blockiness |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 25K | 24,945 | 21.84 | 21.18 | 32.22 | 32.18 | 0.6268 | 0.8429 | 3.756 |
| 50K | 49,910 | 24.24 | 23.47 | 35.37 | 35.58 | 0.7701 | 0.9287 | 2.324 |
| 100K | 99,988 | 27.01 | 26.26 | 38.14 | 37.98 | 0.8681 | 0.9683 | 1.666 |
| 200K | 197,103 | 30.61 | 29.94 | 41.20 | 41.03 | 0.9353 | 0.9871 | 1.335 |

## Rejected: luma/chroma AC value split

Splitting AC values into luma and chroma sections was tested. It added one
section and did not improve the curve:

| budget | P2dcs RGB | AC value split RGB |
|---:|---:|---:|
| 25K | 21.84 | 21.84 |
| 50K | 24.24 | 24.24 |
| 100K | 27.01 | 26.99 |
| 200K | 30.61 | 30.61 |

Rejected; AC values stay in one section.

## Verdict

Keep. It is a low-risk context split with +0.02-0.03 dB at 50-100 KB.

## Files

- p0.md, channels.csv, freq.csv, tiles.csv
- budget_*_diff.png
