# X2: banded (context-partitioned) rANS — rejected

This round completed the rANS work by attempting the obvious next step:
context modelling by partitioning the coefficient streams into bands, each with
its own rANS table.

## What was implemented

- `rans::encode_bands` / `rans::decode_bands`: split a byte stream into N
  interleaved bands and code each with the verified order-0 rANS coder
- codec id 3 in the section table, `Entropy::RansBanded` in the encoder API
- full decode dispatch in both the DCT8 and DCT16 readers
- round-trip test across 1, 4, 8 and 16 bands

## Measured result (1920x1080, DCT8, true serialized bytes)

| budget | zstd | rANS | banded rANS |
|---:|---:|---:|---:|
| 25 KB | 21.95 | 21.93 | 21.29 |
| 50 KB | 24.29 | 24.25 | 23.65 |
| 100 KB | 27.02 | 27.08 | 26.47 |
| 200 KB | 30.63 | 30.93 | 30.37 |

Banded rANS is worse than both at every budget, by 0.3 to 0.66 dB.

## Why it fails

1. Each band stream carries its own 256-entry frequency table. At 16 bands that
   is 16 tables of up to 3 bytes per used symbol, which is a large fixed cost at
   25-50 KB.
2. Splitting by position index does not actually condition on frequency content
   the way a real context model does; it only groups by stream position.
3. The rate controller then has to coarsen qstep to pay for the tables, which
   costs more quality than the modelling gains.

## Decision

Removed. `Entropy::RansBanded`, codec id 3, `encode_bands`/`decode_bands` and
the banded round-trip test are all deleted from the tree. The format keeps
codec id 1 (zstd) and 2 (order-0 rANS).

Recorded honestly: this is the second negative rANS-family result, after
Golomb-Rice in P9. Order-0 rANS remains the only entropy change that actually
won, and it wins only at 100-200 KB.

## Files

- reports/x2-1920/ (banded artifacts from the experiment)
