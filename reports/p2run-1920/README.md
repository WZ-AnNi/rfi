# P2run: AC run-length positions

Isolated AC coding change on top of P2split:

- DC section unchanged.
- AC section still has nonzero count then position/value pairs.
- Position changes from absolute zigzag index to run length from the previous
  nonzero position.
- Same one-byte position field; decoder reconstructs absolute position.
- No container change.

Run length is smaller and more repetitive in the sparse low/mid-rate regime,
which improves zstd context without adding bytes.

## P2split -> P2run

| budget | Split bytes | Split RGB | Run bytes | Run RGB | Split SSIM | Run SSIM | Split MS | Run MS | Split block | Run block |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 25K | 24,998 | 20.54 | 25,000 | **20.70** | **0.6355** | 0.6276 | 0.7970 | **0.7983** | 3.635 | **3.584** |
| 50K | 49,992 | 23.19 | 49,871 | **23.52** | **0.7517** | 0.7490 | 0.8771 | **0.8876** | 2.459 | **2.369** |
| 100K | 99,907 | 25.97 | 99,859 | **26.29** | 0.8307 | **0.8411** | 0.9366 | **0.9414** | 1.770 | **1.740** |
| 200K | 199,981 | 29.55 | 199,828 | **30.14** | 0.9138 | **0.9254** | 0.9773 | **0.9814** | 1.357 | **1.345** |

PSNR improves at every point:

- 25K: +0.16 dB
- 50K: +0.33 dB
- 100K: +0.32 dB
- 200K: +0.59 dB

SSIM is slightly lower at 25/50 KB but higher at 100/200 KB; blockiness is better
at every point.

## Current best DCT

| budget | bytes | RGB | Y | Cb | Cr | SSIM | MS-SSIM | blockiness |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 25K | 25,000 | 20.70 | 20.82 | 27.76 | 27.81 | 0.6276 | 0.7983 | 3.584 |
| 50K | 49,871 | 23.52 | 23.13 | 32.51 | 32.33 | 0.7490 | 0.8876 | 2.369 |
| 100K | 99,859 | 26.29 | 25.78 | 35.70 | 35.93 | 0.8411 | 0.9414 | 1.740 |
| 200K | 199,828 | 30.14 | 29.63 | 39.69 | 39.64 | 0.9254 | 0.9814 | 1.345 |

## Verdict

Keep. This is a direct AC entropy improvement with no new side data.

## Files

- p0.md, channels.csv, freq.csv, tiles.csv
- budget_*_diff.png
