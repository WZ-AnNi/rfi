# T1: is a stronger 8x8 transform basis worth implementing?

Question: would replacing DCT8 with a theoretically stronger basis actually buy
rate-distortion? This measures the ceiling before touching the format.

Tool: `crates/rfi-frequency/src/bin/basis-rd.rs`

~~~
cargo run -p rfi-frequency --release --bin basis-rd -- /tmp/photo1920.jpg
~~~

It gathers every 8x8 luma block of the photo, mean-removes them, and compares:

- DCT8 - the current basis
- DST8 - discrete sine basis, a plausible alternative
- KLT8 - Karhunen-Loeve basis trained on the actual blocks, which is the
  optimal decorrelating transform for this data

Basis transmission cost is excluded, so this overstates any alternative's
advantage.

## Energy compaction (share of AC energy in the top-k coefficients)

| basis | top1 | top4 | top8 | top16 | top32 |
|---|---:|---:|---:|---:|---:|
| DCT8 | 0.0% | 14.9% | 17.2% | 48.3% | 82.6% |
| DST8 | 1.3% | 15.8% | 18.5% | 49.5% | 83.1% |
| KLT8 | 19.0% | 44.8% | 62.1% | 79.5% | 95.2% |

KLT is dramatically better at compaction, which is exactly why a naive
compaction-based argument would recommend it.

## Transform coding gain (the metric that actually predicts RD)

G = 10*log10( arithmetic mean of per-coefficient variance / geometric mean ).
This is how much rate a basis saves at equal distortion when each coefficient
gets a step matched to its own variance.

| basis | coding gain |
|---|---:|
| DCT8 | 7.10 dB |
| DST8 | 5.35 dB |
| KLT8 | **7.31 dB** |

KLT beats DCT by only **0.21 dB**. DCT is already within a rounding error of the
optimal decorrelator on this content.

## Uniform-step PSNR (unmatched steps, shown for completeness)

| step | DCT8 | DST8 | KLT8 |
|---:|---:|---:|---:|
| 4 | 51.55 | 48.51 | 49.07 |
| 8 | 46.21 | 43.29 | 43.84 |
| 20 | 38.36 | 36.47 | 36.90 |
| 48 | 31.27 | 30.38 | 30.56 |

With one shared step, DCT wins outright. This is not a fair test of basis
strength (KLT concentrates energy into large coefficients that then suffer large
absolute error), so it is reported only to show the trap.

## Variance-matched step PSNR

| step | DCT8 | DST8 | KLT8 |
|---:|---:|---:|---:|
| 1 | 53.29 | 54.03 | 52.21 |
| 4 | 41.60 | 42.46 | 40.75 |
| 16 | 31.09 | 32.19 | 30.51 |

Even with per-coefficient steps, KLT still loses to both DCT and DST at equal
step scale. The coding-gain advantage does not translate into realised PSNR
because the gain is measured against an ideal entropy coder and uniform
distortion weighting that this codec does not have.

## Held-out generalization

KLT trained on the same blocks it is scored on is optimistic. Retraining on even
blocks and scoring on odd blocks:

| basis | best held-out PSNR |
|---|---:|
| DCT8 | **55.36 dB** |
| KLT8 (held out) | 52.15 dB |

A KLT that must be transmitted and generalized loses by 3.2 dB.

## Verdict

**Not worth implementing.** Three independent measurements agree:

1. Coding gain over DCT is +0.21 dB, and only for an eagerly-fitted KLT.
2. With held-out training the advantage reverses to -3.2 dB.
3. Any KLT would additionally need its 64 basis vectors stored in the file or
   derived from the image, which costs far more than 0.21 dB.

DST is strictly worse on this content (5.35 dB gain, and it loses at matched
steps too).

So the transform basis is not the bottleneck. The realistic remaining levers are
quantization and entropy modelling, not a different 8x8 basis.

## What this does not rule out

- Non-8x8 or non-separable bases (already partly explored via DCT16).
- Shape-adaptive or directional transforms for edge-dominant content.
- Perceptual weighting and masking, which change the distortion metric rather
  than the decorrelation.
