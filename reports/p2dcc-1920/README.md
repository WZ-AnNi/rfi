# P2dcc: continuous DC prediction across tiles

Isolated predictor change on top of P2m7:

- DC predictor no longer resets to zero at each tile.
- Luma keeps the last DC of the previous tile.
- Cb and Cr each keep their own last DC.
- No container change; decoder applies the same predictor continuity.

This is a spatial continuity assumption: adjacent tiles usually have similar
DC. It reduces the first-block DC difference at each tile boundary.

## P2m7 -> P2dcc

| budget | P2m7 bytes | P2m7 RGB | P2dcc bytes | P2dcc RGB | P2m7 SSIM | P2dcc SSIM | P2m7 MS | P2dcc MS | P2m7 block | P2dcc block |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 25K | 24,974 | 21.83 | 24,939 | **21.84** | **0.6278** | 0.6276 | **0.8442** | 0.8434 | 3.766 | **3.755** |
| 50K | 49,984 | 24.20 | 49,903 | **24.21** | 0.7682 | **0.7688** | 0.9273 | **0.9276** | 2.336 | **2.333** |
| 100K | 99,957 | 26.98 | 99,792 | **26.99** | 0.8668 | **0.8669** | 0.9678 | 0.9678 | 1.671 | **1.670** |
| 200K | 199,119 | 30.59 | 199,211 | **30.60** | 0.9348 | **0.9351** | 0.9869 | **0.9871** | 1.335 | **1.334** |

Gain is consistent but tiny (+0.01 dB). 25K SSIM is 0.0002 lower; everything
else is flat or a hair better.

## Current best

| budget | bytes | RGB | Y | Cb | Cr | SSIM | MS-SSIM | blockiness |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 25K | 24,939 | 21.84 | 21.18 | 32.27 | 32.19 | 0.6276 | 0.8434 | 3.755 |
| 50K | 49,903 | 24.21 | 23.45 | 35.29 | 35.50 | 0.7688 | 0.9276 | 2.333 |
| 100K | 99,792 | 26.99 | 26.25 | 38.06 | 37.88 | 0.8669 | 0.9678 | 1.670 |
| 200K | 199,211 | 30.60 | 29.95 | 41.08 | 40.88 | 0.9351 | 0.9871 | 1.334 |

## Verdict

Keep. It is a free predictor improvement with no format overhead.

## Files

- p0.md, channels.csv, freq.csv, tiles.csv
- budget_*_diff.png
