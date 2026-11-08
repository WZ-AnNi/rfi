# P6: DCT8 / DCT16 per-file rate-distortion selection

RFI basis id 3 is a 16x16 DCT basis. The encoder evaluates DCT8 and DCT16
against the same final serialized-byte budgets and selects the higher RGB PSNR
point. Selection is per file; the fixed 64x64 tile grid needs no per-tile basis
directory.

## Reproduction

~~~
cargo run -p rfi-cli --release -- best-rd /tmp/photo1920.jpg reports/best-final-1920
cargo run -p rfi-frequency --release --bin dct16-rd -- /tmp/photo1920.jpg reports/dct16-final-1920
~~~

## RD curve (1920x1080 source)

| budget | selected basis | bytes | RGB PSNR | SSIM | MS-SSIM | blockiness |
|---:|---|---:|---:|---:|---:|---:|
| 25 KB | DCT16 | 24,995 | 22.36 dB | 0.6957 | 0.8817 | 1.983 |
| 50 KB | DCT16 | 49,993 | 24.32 dB | 0.7789 | 0.9330 | 1.575 |
| 100 KB | DCT8 | 99,988 | 27.01 dB | 0.8681 | 0.9683 | 1.666 |
| 200 KB | DCT8 | 197,103 | 30.61 dB | 0.9353 | 0.9871 | 1.335 |

## DCT16 vs DCT8

| budget | DCT8 RGB / SSIM | DCT16 RGB / SSIM | winner |
|---:|---:|---:|---|
| 25 KB | 21.84 / 0.6268 | 22.36 / 0.6957 | DCT16 (+0.52 dB) |
| 50 KB | 24.24 / 0.7701 | 24.32 / 0.7789 | DCT16 (+0.08 dB) |
| 100 KB | 27.01 / 0.8681 | 26.84 / 0.8516 | DCT8 |
| 200 KB | 30.61 / 0.9353 | 30.39 / 0.9121 | DCT8 |

DCT16 reduces blockiness substantially across all four points; DCT8 retains the
medium/high-rate PSNR and SSIM lead. Selection here maximizes RGB PSNR, not a
composite perceptual score.

## Format and viewer

- basis id 3 = 16x16 DCT; decoder dispatches by basis id.
- dct16-rd measures the DCT16 curve.
- best-rd evaluates DCT8 and DCT16 at each byte budget, then writes the
  selected .rfi and its difference map.
- Viewer labels DCT16 correctly; headless --check reads DCT16 and DCT8 files.
- No compatibility code for the removed RFI format is present.

## Verdict

Keep DCT8 and DCT16 as separate bases. Per-file selection is useful: DCT16
improves the low-rate end while DCT8 retains the medium/high-rate lead. The
comparison uses actual serialized size, not qstep alone.
## C: DCT16 subband quantization matrices

DCT16 originally used a linear frequency weight, `step * (1 + 0.2*(u+v))`, with no
per-band tuning. It now uses real 16x16 luma and chroma weight matrices derived
from the usual contrast-sensitivity shape, scaled so DC = 16, with a strength
knob blending toward flat.

| matrix strength | 25 KB RGB / SSIM | 50 KB RGB / SSIM |
|---:|---:|---:|
| flat (previous) | 22.47 / 0.7042 | 24.40 / 0.7809 |
| 1.0 (full table) | 22.37 / 0.7064 | 24.40 / 0.7791 |
| **0.6 (kept)** | **22.41 / 0.7071** | **24.41 / 0.7840** |
| 0.8 | 22.39 / 0.6951 | 24.40 / 0.7791 |

Effect is small but positive and shows in the selected operating points:

| budget | before | after |
|---|---|---|
| 25 KB | DCT16+auto 22.47 / 0.7042 | **DCT16+auto 22.41 / 0.7071** |
| 50 KB | DCT8+auto 24.40 / 0.7791 | **DCT16+auto 24.41 / 0.7840** |
| 100 KB | DCT8+auto 27.23 / 0.8730 | unchanged |
| 200 KB | DCT8+auto 31.08 / 0.9409 | unchanged |

Honest reading: this is a perceptual trade, not a PSNR gain. 25 KB PSNR drops by
0.06 dB while SSIM rises by 0.0029 and blockiness falls; 50 KB gains +0.01 dB and
+0.0049 SSIM, and DCT16 becomes the selected basis again at 50 KB.

Strength 1.0 and 0.8 both lose SSIM, so the tuned deviation is 60% of the raw
table. DCT8 still wins at 100/200 KB.

Reproduction:

~~~
cargo run -p rfi-cli --release -- best-rd /tmp/photo1920.jpg reports/c4q-1920
~~~
