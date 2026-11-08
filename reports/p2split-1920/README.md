# P2split: separate DC and AC coefficient sections

Isolated entropy-layout change on top of P4z:

- Basis section unchanged.
- DC differences go into one zstd section (section type 3).
- AC count/position/value goes into a second zstd section (section type 4).
- Decoder reads the two sections independently and walks tiles/blocks in the
  same fixed order.
- No tile directory.

This gives DC and AC separate zstd contexts without adding a per-tile table.

## P4z -> P2split

| budget | P4z bytes | P4z RGB | Split bytes | Split RGB | P4z SSIM | Split SSIM | P4z block | Split block |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 25K | 24,983 | 20.47 | 24,998 | **20.54** | 0.6330 | **0.6355** | 3.691 | **3.635** |
| 50K | 49,970 | 23.11 | 49,992 | **23.19** | 0.7424 | **0.7517** | 2.499 | **2.459** |
| 100K | 98,653 | 25.79 | 99,907 | **25.97** | **0.8387** | 0.8307 | 1.794 | **1.770** |
| 200K | 199,969 | 29.35 | 199,981 | **29.55** | **0.9143** | 0.9138 | 1.351 | 1.357 |

PSNR improves at every point. SSIM improves at 25/50 KB and is essentially flat
at 100/200 KB. The gain is modest but consistent; not enough to change the
low-rate trajectory by itself.

## Why

DC and AC have very different statistics: DC is a small predicted value per
block, AC is sparse position/value data. Separating them lets zstd build two
contexts instead of one mixed stream. The cost is one extra section entry
(32 B), included in the measured file size.

## P2split 50 KB sections

- Header + 3 section entries: 192 B
- Basis: 17 B
- DC + AC sections: 49,783 B stored / 192,078 B raw
- Total: 49,992 B

## Rejected: adaptive per-tile qscale

An attempt with one byte of per-tile qscale (detail-adaptive, 0.7-1.3x) was
tested and rejected:

| budget | split RGB | adaptive-q RGB | split SSIM | adaptive SSIM |
|---:|---:|---:|---:|---:|
| 25K | 20.54 | 19.99 | 0.6355 | 0.6153 |
| 50K | 23.19 | 22.84 | 0.7517 | 0.7348 |
| 100K | 25.97 | 25.86 | 0.8307 | 0.8333 |
| 200K | 29.55 | 29.50 | 0.9138 | 0.9100 |

The heuristic and extra side data lose more than they gain; it is not kept.

## Verdict

Keep split DC/AC. Current best DCT chain:

- 25 KB: 20.54 dB / SSIM 0.6355
- 50 KB: 23.19 dB / SSIM 0.7517
- 100 KB: 25.97 dB / SSIM 0.8307
- 200 KB: 29.55 dB / SSIM 0.9138

## Files

- p0.md, channels.csv, freq.csv, tiles.csv
- budget_*_diff.png
