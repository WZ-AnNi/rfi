# P2pack: packed AC counts

Isolated entropy-layout change on top of P2cnt:

- AC counts are packed two per byte as 4-bit nibbles.
- Count 0-14 uses the nibble directly.
- Count 15 uses nibble 15 plus an escape byte in a separate extension section.
- Counts, escapes, positions and values are separate zstd sections.
- No extra per-block bytes; one extra section entry.

## P2cnt -> P2pack

| budget | P2cnt bytes | P2cnt RGB | P2pack bytes | P2pack RGB | P2cnt SSIM | P2pack SSIM | P2cnt MS | P2pack MS | P2cnt block | P2pack block |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 25K | 24,979 | 21.81 | 24,982 | **21.85** | **0.6237** | 0.6188 | **0.8414** | 0.8390 | 3.760 | **3.745** |
| 50K | 49,832 | 24.19 | 49,758 | **24.20** | 0.7659 | **0.7663** | 0.9253 | **0.9259** | 2.340 | **2.337** |
| 100K | 99,489 | 27.04 | 99,971 | **27.07** | 0.8623 | **0.8624** | 0.9660 | 0.9660 | 1.672 | **1.663** |
| 200K | 198,642 | 30.73 | 197,095 | 30.73 | 0.9340 | 0.9340 | 0.9866 | 0.9866 | 1.344 | 1.344 |

The gain is very small: count packing saves about 70-100 bytes at 50 KB, and
rate control reinvests it. 25 KB SSIM is slightly lower; 50-200 KB are
effectively flat or a hair better.

## Why it is not a big win

The counts section was already zstd-compressed from 48,960 B raw to about 7,942 B
at 50 KB. Nibble packing halves the raw count bytes, but zstd already exploited
the repeated low-count values; the stored count section shrinks much less.

## Current best

| budget | bytes | RGB | Y | Cb | Cr | SSIM | MS-SSIM | blockiness |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 25K | 24,982 | 21.85 | 21.18 | 32.29 | 32.20 | 0.6188 | 0.8390 | 3.745 |
| 50K | 49,758 | 24.20 | 23.44 | 35.28 | 35.49 | 0.7663 | 0.9259 | 2.337 |
| 100K | 99,971 | 27.07 | 26.33 | 38.12 | 37.94 | 0.8624 | 0.9660 | 1.663 |
| 200K | 197,095 | 30.73 | 30.08 | 41.20 | 41.00 | 0.9340 | 0.9866 | 1.344 |

## Verdict

Keep, but mark as marginal. It is a valid entropy-layout optimization, not a
material rate-distortion gain. The next effort should target AC positions/values
or a real entropy coder, not further count packing.

## Files

- p0.md, channels.csv, freq.csv, tiles.csv
- budget_*_diff.png
