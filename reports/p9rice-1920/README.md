# P9: Golomb-Rice AC coding (rejected)

User request: try Golomb-Rice (and rANS-family) entropy coding for AC values.
This round implemented and measured Golomb-Rice.

## What was implemented

- Adaptive signed Rice code for AC magnitudes.
- zigzag mapping applied to the signed value, so sign survives the unary+binary
  split.
- Rice parameter k chosen from the mean zigzag magnitude of the AC value vector
  and stored in header byte 50.
- Bit writer/reader with 64-bit accumulator, fed into the existing zstd
  section pipeline.
- Encoder and decoder both switched to the Rice path; round-trip tests passed
  (10 tests including DCT8/DCT16 image and transform round-trips).

## Measured result (1920x1080, true serialized budgets)

| budget | varint+zstd (P7) | Rice+zstd | delta |
|---:|---:|---:|---:|
| 25 KB | 21.95 | **21.96** | +0.01 |
| 50 KB | 24.29 | 24.26 | -0.03 |
| 100 KB | 27.02 | 27.02 | 0.00 |
| 200 KB | 30.63 | 30.57 | -0.06 |

Section-level comparison at 50 KB:

| section | varint+zstd | Rice+zstd |
|---|---:|---:|
| AC values raw | 54,342 B | 19,360 B |
| AC values stored | 14,957 B | 12,869 B |

Rice compresses the raw AC value stream well (about 2.1 KB smaller stored), but
the rate controller responds by lowering qstep, which inflates the DC and count
sections. The net effect on final PSNR is negative at 50 KB and 200 KB, and
flat at 100 KB.

## Why it does not win here

1. The existing varint+zstd path already exploits the magnitude distribution.
2. Rice is optimal for a geometric distribution; quantized DCT AC values are
   closer to Laplacian with a heavy zero mass, and the counts/positions streams
   already remove most zeros.
3. Bit-level packing destroys the byte-aligned repetition that zstd exploits,
   so the second stage has less to compress.
4. A single global k per image is too coarse; real gains need per-frequency or
   per-block context modelling, which is a much larger change.

## Decision

Rejected and reverted. The AC value section is back to varint + zstd level 19.

This is an honest negative result: Golomb-Rice alone does not beat the existing
pipeline on this content. A future attempt should combine Rice/rANS with
per-frequency context (or replace the whole entropy layer, not one stream).

## Files

- reports/p9rice-1920/ (Rice-coded artifacts from the experiment)
- reports/p9-final/ (post-revert curve, identical to P7)
