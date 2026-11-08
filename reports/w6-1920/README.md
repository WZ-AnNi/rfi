# W6: multi-level 5/3 wavelet + EZW zerotree

Basis id 2 was rebuilt into a real wavelet codec with a true zerotree coder.

## Implementation

- 5/3 lifting, 4 levels for 64x64 luma, 3 levels for 32x32 chroma (a 4-level
  32x32 chroma pyramid collapses to a 2x2 base, which destroyed colour; the
  level count is now chosen by plane size)
- Spatial orientation tree built from real subband geometry: LL roots, each
  root owning three children at the same relative position in the three detail
  bands of the next finer level, and each detail coefficient owning a 2x2 block
  in the corresponding position one level finer
- EZW dominant pass per bitplane with ZTR (zerotree root) pruning via a subtree
  maximum map, plus a subordinate refinement pass over already-significant
  coefficients
- Chroma has its own quantization step (x0.25 luma) so colour survives
- Per-plane length prefix, zstd level 19, same RFI container, no compatibility
  code, no legacy single-level path left in the tree

## Verification

- pyramid forward/inverse round-trip: exact to <1e-3
- EZW plane encode/decode: bit-exact on a synthetic plane with outliers
- full wavelet image round-trip and a dedicated chroma round-trip test
- 9 workspace tests pass; fmt, clippy (-D warnings) and release build clean

## Measured RD (1920x1080, true serialized bytes)

| budget | bytes | RGB PSNR | Y | Cb | Cr | SSIM | MS-SSIM | blockiness |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 25 KB | 25,000 | 14.86 | 13.94 | 27.59 | 27.30 | 0.4827 | 0.5154 | 1.380 |
| 50 KB | 49,997 | 17.42 | 16.63 | 30.07 | 29.96 | 0.5439 | 0.6061 | 1.280 |
| 100 KB | 99,956 | 21.26 | 20.25 | 33.70 | 33.78 | 0.6876 | 0.7262 | 1.180 |
| 200 KB | 199,930 | 25.56 | 24.66 | 37.94 | 38.41 | 0.7833 | 0.8263 | 1.118 |

## Comparison with DCT (best of DCT8/DCT16)

| budget | wavelet RGB | DCT RGB | gap | wavelet SSIM | DCT SSIM |
|---:|---:|---:|---:|---:|---:|
| 25 KB | 14.86 | 22.41 | -7.55 | 0.4827 | 0.7023 |
| 50 KB | 17.42 | 24.37 | -6.95 | 0.5439 | 0.7798 |
| 100 KB | 21.26 | 27.02 | -5.76 | 0.6876 | 0.8682 |
| 200 KB | 25.56 | 30.63 | -5.07 | 0.7833 | 0.9354 |

Wavelet blockiness is much lower (1.12-1.38 vs 1.34-3.76), but the PSNR and
SSIM deficit is large at every rate.

## What this does and does not establish

Established: a correct multilevel 5/3 wavelet plus a correct EZW zerotree is
implemented, round-trips exactly, and is still clearly worse than the DCT path
on this photo.

Not established: that wavelet cannot win in general. This implementation still
uses a single scalar step for the whole luma plane (no per-subband weighting),
one chroma step scale, and no arithmetic coder behind EZW. A tuned subband
quantization matrix plus an arithmetic-coded EZW is a different experiment and
was not run here.

## Decision

Basis id 2 stays decodable and experimental; the encoder does not select it.
The next round runs the rANS entropy-layer experiment on the DCT path.

## Files

- reports/w6-1920/budget_*.rfi and difference maps
- crates/rfi-frequency/src/wavelet.rs
