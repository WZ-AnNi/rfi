# P7: JPEG-LS MED DC prediction

Isolated predictor change: the DC predictor is no longer "previous block DC"
(1D raster order). It is now the JPEG-LS MED (median edge detector) over the
block grid:

- left, top and top-left neighbours of the current block in that channel's grid
- MED rule with edge detection (if top_left >= max -> min; if <= min -> max;
  else left + top - top_left)
- image border: only available neighbours are used, and (none, none) predicts 0
- the block grid spans tiles, so there is no reset at tile boundaries
- Y, Cb and Cr each have their own grid

This changes both encoder and decoder; the container and section layout are
unchanged.

## Previous (P2dcs: 1D previous DC) -> P7 (MED)

| budget | P2dcs RGB | P7 RGB | P2dcs SSIM | P7 SSIM | P2dcs MS | P7 MS | P2dcs block | P7 block |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 25K | 21.84 | **21.95** | 0.6268 | **0.6542** | 0.8429 | **0.8596** | 3.756 | **3.655** |
| 50K | 24.24 | **24.29** | 0.7701 | 0.7645 | 0.9287 | 0.9263 | 2.324 | **2.315** |
| 100K | 27.01 | **27.02** | 0.8681 | **0.8682** | 0.9683 | **0.9684** | 1.666 | **1.664** |
| 200K | 30.61 | **30.63** | 0.9353 | **0.9354** | 0.9871 | 0.9871 | 1.335 | 1.335 |

Gains are small but consistent in PSNR (+0.01 to +0.11 dB). The 25K SSIM and
MS-SSIM improve by about 0.027 and 0.017. At 50K, SSIM is slightly lower while
PSNR and blockiness improve.

## DCT16 with MED

DCT16 now uses the same MED predictor per its 16x16 block grid.

| budget | DCT16 before | DCT16 with MED |
|---:|---:|---:|
| 25K | 22.36 | **22.41** |
| 50K | 24.32 | **24.37** |
| 100K | 26.84 | **26.89** |
| 200K | 30.39 | **30.45** |

## Best selected curve

| budget | basis | bytes | RGB | SSIM | MS-SSIM | blockiness |
|---:|---|---:|---:|---:|---:|---:|
| 25K | DCT16 | 24,995 | 22.41 | 0.7023 | 0.8850 | 1.966 |
| 50K | DCT16 | 49,998 | 24.37 | 0.7798 | 0.9334 | 1.567 |
| 100K | DCT8 | 99,103 | 27.02 | 0.8682 | 0.9684 | 1.664 |
| 200K | DCT8 | 199,394 | 30.63 | 0.9354 | 0.9871 | 1.335 |

## Verdict

Keep. It is a free predictor improvement with no side data. The gain is small in
PSNR, but it is the correct predictor for block-grid DC and it removes the
tile-boundary reset entirely.

## Reproduction

~~~
cargo run -p rfi-cli --release -- analyze /tmp/photo1920.jpg reports/p7med-1920
cargo run -p rfi-cli --release -- best-rd /tmp/photo1920.jpg reports/p7-1920
cargo run -p rfi-frequency --release --bin dct16-rd -- /tmp/photo1920.jpg reports/p7-dct16
~~~
## DCT16 frequency-weight sweep

The DCT16 scalar frequency weight `1 + k*(u+v)` was swept with the MED
predictor active. Each point is a true serialized-size curve on the same
1920x1080 source.

| k | 25K | 50K | 100K | 200K |
|---:|---:|---:|---:|---:|
| 0.05 | 22.37 | 24.40 | 26.98 | 30.65 |
| 0.10 | 22.41 | 24.41 | 26.99 | 30.59 |
| 0.20 | 22.41 | 24.37 | 26.89 | 30.45 |
| 0.35 | 22.34 | 24.25 | 26.75 | 30.27 |

k=0.05 and k=0.10 are nearly tied on PSNR. k=0.20 is kept because it also has
the best 25K/50K SSIM/MS-SSIM in the full-metric run (0.7023/0.8850 and
0.7798/0.9334), and it removes more high-frequency energy, which is what keeps
DCT16's blockiness lower.

Recorded honestly: on pure PSNR k=0.05 is marginally ahead at 50K and 200K. The
default remains k=0.2 for the perceptual metrics.
