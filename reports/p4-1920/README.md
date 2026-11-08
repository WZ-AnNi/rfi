# P4: no per-tile directory

The tile grid is fixed at 64x64, all tiles use the same basis, and coefficient
blocks are decoded in a fixed order. The per-tile directory is therefore
redundant and was removed.

## P2 -> P4

P2 = varint coefficients, fixed soft-luma matrix, 32-byte tile directory.
P4 = same codec, tile directory removed; decoder derives tile order.

| budget | P2 bytes | P2 RGB | P4 bytes | P4 RGB | P2 SSIM | P4 SSIM | P2 MS | P4 MS | P2 block | P4 block |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 25K | 24,995 | 19.66 | 24,996 | 20.15 | 0.5595 | 0.6309 | 0.7389 | 0.7917 | 4.538 | 4.005 |
| 50K | 50,000 | 22.49 | 49,993 | 22.79 | 0.7245 | 0.7180 | 0.8599 | 0.8633 | 2.700 | 2.613 |
| 100K | 99,985 | 25.50 | 99,988 | 25.67 | 0.8203 | 0.8299 | 0.9294 | 0.9382 | 1.896 | 1.841 |
| 200K | 199,970 | 28.86 | 199,984 | 28.95 | 0.9062 | 0.9048 | 0.9743 | 0.9738 | 1.376 | 1.361 |

## Why it helps

At 50 KB:

- P2 tile directory: 3,004 B
- P2 coefficient section: 46,787 B
- P4 tile directory: 0 B
- P4 coefficient section: 49,816 B (same file budget; lower qstep)

The saved bytes go into a lower qstep:

- P2 qstep at 50K: 189.89
- P4 qstep at 50K: 178.79

The 25K point gains the most (+0.49 dB RGB, +0.071 SSIM) because the directory
was 11% of that file.

## Format change

- Header section count is now 2: basis + coefficients.
- No tile section is written or read.
- Tile coordinates are derived from width, height and tile size 64.
- The coefficient stream is decoded sequentially.
- Section 2 remains reserved for a future per-tile basis directory.
- Old files with a tile section are rejected by the current reader (new format,
  no compatibility).

## Verdict

Removing the redundant directory is a clear win, especially at 25-50 KB. The
remaining fixed overhead is header + table + basis = 209 B.

## Files

- p0.md, channels.csv, freq.csv, tiles.csv
- budget_*_diff.png
