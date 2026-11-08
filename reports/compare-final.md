# RFI basis x entropy comparison (1920x1080)

Source: /tmp/photo1920.jpg, 386,694 B. Every number is a real serialized file
size including header, section table, basis and coefficient sections; the rate
controller binary-searches qstep per configuration so each point sits at or
below the budget.

## Per-configuration RGB PSNR

| budget | wavelet+EZW | DCT8+zstd | DCT8+rANS | DCT16+zstd | DCT16+rANS |
|---:|---:|---:|---:|---:|---:|
| 25 KB | 14.86 | 21.95 | 21.93 | **22.41** | 22.11 |
| 50 KB | 17.42 | 24.29 | 24.25 | **24.37** | 23.75 |
| 100 KB | 21.26 | 27.02 | **27.08** | 26.89 | 25.73 |
| 200 KB | 25.56 | 30.63 | **30.93** | 30.45 | 28.34 |

## Selected configuration (evaluates all four DCT combinations)

| budget | selected | bytes | RGB | SSIM | MS-SSIM | blockiness |
|---:|---|---:|---:|---:|---:|---:|
| 25 KB | DCT16+zstd | 24,995 | 22.41 | 0.7023 | 0.8850 | 1.966 |
| 50 KB | DCT16+zstd | 49,998 | 24.37 | 0.7798 | 0.9334 | 1.567 |
| 100 KB | DCT8+rANS | 99,999 | 27.08 | 0.8697 | 0.9687 | 1.659 |
| 200 KB | DCT8+rANS | 199,987 | 30.93 | 0.9393 | 0.9884 | 1.335 |

## Verdict

1. Wavelet with a real EZW zerotree loses everywhere: 5.1-7.6 dB behind the
   DCT path. Its blockiness is lower, but not enough to compensate.
2. rANS wins at 100 KB (+0.06 dB) and 200 KB (+0.30 dB) and becomes the
   selected backend at both points. It trails zstd slightly at 25-50 KB.
3. Basis and backend interact: DCT16+rANS is much worse than DCT16+zstd
   (28.34 vs 30.45 dB at 200 KB), so selection must be joint, not staged.

So between the two requested experiments, rANS produced the usable result;
wavelet+zerotree produced a correct implementation and a negative result.

## Reproduction

~~~
cargo run -p rfi-cli --release -- compare-rd /tmp/photo1920.jpg reports/c1-1920
cargo run -p rfi-cli --release -- best-rd /tmp/photo1920.jpg reports/r5-best
cargo run -p rfi-cli --release -- wavelet-rd /tmp/photo1920.jpg reports/w6-1920
~~~

## Limits

- rANS is order-0 with a static table; no context model.
- Wavelet uses one scalar step for the whole luma plane and no arithmetic
  coding behind EZW, so it may not be the best possible wavelet result.
- No configuration reaches 50 KB / 50 dB on this photo.
## Update: wavelet removed, rANS completed

The wavelet column below is historical. The multilevel 5/3 + EZW codec was
deleted from the tree after this measurement; basis id 2 is now rejected.
The rANS work was then completed: order-0 rANS kept, banded-context rANS tried
and removed (see reports/x2-1920), and the per-file selector now evaluates
DCT8/DCT16 x zstd/rANS.

Current selected operating points:

| budget | selected | bytes | RGB |
|---:|---|---:|---:|
| 25 KB | DCT16+zstd | 24,995 | 22.41 |
| 50 KB | DCT16+zstd | 49,998 | 24.37 |
| 100 KB | DCT8+rANS | 99,999 | 27.08 |
| 200 KB | DCT8+rANS | 199,987 | 30.93 |
