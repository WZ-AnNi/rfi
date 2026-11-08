# P5: wavelet 5/3 experiment

Experimental basis id 2: one-level 2D 5/3 lifting wavelet per tile, scalar
quantization, zero-run varint coefficients, zstd level 19. Same container as
DCT but no tile directory and no quantization matrix.

This is a fair first comparison, not a full wavelet codec: it is one-level, with
no multilevel zero-tree, no frequency context and no wavelet-specific entropy
model.

## best DCT (P6) vs wavelet (P5) at true serialized budgets

| budget | DCT bytes | DCT RGB | W bytes | W RGB | DCT Y | W Y | DCT SSIM | W SSIM | DCT block | W block |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 25K | 24,995 | 22.36 | 24,989 | 12.44 | 21.45 | 11.27 | 0.6957 | 0.4540 | 1.983 | 1.187 |
| 50K | 49,993 | 24.32 | 49,711 | 16.01 | 23.42 | 15.28 | 0.7789 | 0.5530 | 1.575 | 1.208 |
| 100K | 99,988 | 27.01 | 99,871 | 20.21 | 26.26 | 20.76 | 0.8681 | 0.7033 | 1.666 | 1.122 |
| 200K | 197,103 | 30.61 | 199,998 | 23.40 | 29.94 | 25.73 | 0.9353 | 0.7901 | 1.335 | 1.089 |

Wavelet is **worse at every PSNR/SSIM point** in this implementation:

- 50 KB: 16.01 dB vs 23.11 dB (-7.1 dB)
- 100 KB: 20.21 dB vs 25.79 dB (-5.6 dB)
- 200 KB: 23.40 dB vs 29.35 dB (-6.0 dB)

Wavelet blockiness is much lower, but that is not enough: SSIM and RGB are much
lower.

## Why

This is a deliberately minimal wavelet path:

1. One-level 2D lifting only.
2. Scalar quantization of all subbands.
3. Sparse coefficient stream without zerotree/context modeling.
4. No wavelet-specific rate allocation.

The result does not justify replacing DCT. It also does not prove wavelet is
inherently worse; a multilevel 5/3 + zerotree/context coder is required before
that claim. But as an isolated P5 test, the current wavelet path loses.

## Decision

- Keep DCT8 as the default basis.
- Keep basis id 2 in the reader for experiments.
- Do not add per-tile DCT/Wavelet selection yet (P6); the wavelet candidate is
  not competitive.
- Revisit wavelet only with multilevel decomposition and a proper coefficient
  coder.

## Files

- p5-1920/budget_*.rfi
- p5-1920/budget_*_diff.png