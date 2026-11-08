# P2cnt: AC counts as a separate stream

Isolated entropy-layout change on top of P2pv:

- DC section unchanged.
- AC count stream: one byte per block (section type 4).
- AC position/run stream: run lengths only (section type 5).
- AC value stream: zigzag signed LEB128 values (section type 6).
- No extra per-block byte: the count was moved out of the position stream.
- One extra section entry (32 B), included in file size.

## P2pv -> P2cnt

| budget | P2pv bytes | P2pv RGB | P2cnt bytes | P2cnt RGB | P2pv SSIM | P2cnt SSIM | P2pv MS | P2cnt MS | P2pv block | P2cnt block |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 25K | 24,998 | 21.81 | 24,979 | 21.81 | **0.6240** | 0.6237 | **0.8415** | 0.8414 | 3.762 | **3.760** |
| 50K | 49,851 | 24.14 | 49,832 | **24.19** | 0.7603 | **0.7659** | 0.9225 | **0.9253** | 2.359 | **2.340** |
| 100K | 99,880 | 26.89 | 99,489 | **27.04** | 0.8600 | **0.8623** | 0.9650 | **0.9660** | 1.707 | **1.672** |
| 200K | 198,986 | 30.64 | 198,642 | **30.73** | 0.9325 | **0.9340** | 0.9863 | **0.9866** | 1.343 | 1.344 |

Gains are small but consistent at 50-200 KB:

- 50K: +0.05 dB, +0.0056 SSIM
- 100K: +0.15 dB, +0.0023 SSIM
- 200K: +0.09 dB, +0.0015 SSIM
- 25K: effectively flat

## Current best

| budget | bytes | RGB | Y | Cb | Cr | SSIM | MS-SSIM | blockiness |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 25K | 24,979 | 21.81 | 21.14 | 32.23 | 32.15 | 0.6237 | 0.8414 | 3.760 |
| 50K | 49,832 | 24.19 | 23.43 | 35.27 | 35.48 | 0.7659 | 0.9253 | 2.340 |
| 100K | 99,489 | 27.04 | 26.30 | 38.11 | 37.93 | 0.8623 | 0.9660 | 1.672 |
| 200K | 198,642 | 30.73 | 30.08 | 41.20 | 41.00 | 0.9340 | 0.9866 | 1.344 |

## Rejected

Splitting AC values into low-frequency/high-frequency streams was tested and
rejected: 50 KB fell from 24.14 to 24.05 dB with extra section overhead.

## Verdict

Keep. It is a cheap context separation; one extra section entry, no extra
per-block byte, and small but consistent gains at 50-200 KB.

## Files

- p0.md, channels.csv, freq.csv, tiles.csv
- budget_*_diff.png
