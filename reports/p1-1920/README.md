# P1: fixed Y/chroma quantization matrices

Isolated change: only the quantization matrix changed. Rate control, DCT,
YCoCg 4:2:0, DC prediction, zero-run coding, zstd and the container are
unchanged. All four points are selected by final serialized bytes.

## P0 -> P1

P0 = scalar step, same step for all frequencies. P1 = fixed JPEG-style luma
matrix and independent chroma matrix; effective step is
step * M[u,v] / 16.

| budget | P0 bytes | P0 RGB | P1 bytes | P1 RGB | P0 SSIM | P1 SSIM | P0 MS | P1 MS | P0 block | P1 block |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 25K | 25,000 | 19.16 | 24,994 | 19.03 | 0.5520 | 0.5804 | 0.7149 | 0.7206 | 4.704 | 5.462 |
| 50K | 49,983 | 20.91 | 49,990 | 21.41 | 0.6773 | 0.6985 | 0.8268 | 0.8349 | 2.721 | 3.189 |
| 100K | 99,991 | 24.24 | 99,989 | 24.54 | 0.7711 | 0.7829 | 0.8985 | 0.9103 | 1.908 | 2.142 |
| 200K | 199,954 | 27.76 | 199,718 | 27.64 | 0.8668 | 0.8906 | 0.9510 | 0.9704 | 1.396 | 1.565 |

## Reading

- P1 improves SSIM at every point and MS-SSIM at every point.
- PSNR improves at 50K (+0.50 dB) and 100K (+0.30 dB), but is slightly lower
  at 25K (-0.13) and 200K (-0.12).
- Blockiness is worse at every point: the matrix removes more high-frequency
  energy without any deblocking stage.
- At 50K, Y AC zstd falls from 30,354 B to 29,474 B (-880 B), but chroma
  zstd rises from 1,303 B to 2,183 B (+880 B). The chroma matrix is too fine
  relative to the old 1.5x scalar rule, so it takes back the Y AC savings.

The result is not a clean win. It is a perceptual/PSNR trade, and the blockiness
regression means the matrix is too aggressive at high frequencies.

A coarser-chroma ablation (chroma step multiplied by 1.5) gave:

| budget | RGB | SSIM | blockiness |
|---:|---:|---:|---:|
| 25K | 19.04 | 0.5809 | 5.440 |
| 50K | 20.79 | 0.6989 | 3.115 |
| 100K | 24.14 | 0.7918 | 2.116 |
| 200K | 27.40 | 0.8909 | 1.550 |

It restores some SSIM but loses PSNR; it does not fix blockiness.

## P1 verdict

Fixed matrices are useful but not yet acceptable as a locked change:

1. SSIM/MS-SSIM improve consistently.
2. PSNR is mixed.
3. Blockiness worsens consistently.
4. The Y/chroma bit allocation needs tuning; chroma currently takes back the
   Y AC savings at 50-100 KB.

Before P2 entropy coding, tune the matrix strength:
- gentler high-frequency weights;
- chroma scale chosen by measured byte allocation;
- optionally a deblocking/loop filter, but that would be a separate experiment.

## Files

- p0.md: generated P1 quality and byte summary
- channels.csv, freq.csv, tiles.csv
- budget_*_diff.png
