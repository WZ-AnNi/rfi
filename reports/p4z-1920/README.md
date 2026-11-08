# P4z: zstd level 19

Isolated change on top of P4: coefficient and basis sections use zstd level 19
instead of level 9. Rate control still searches qstep by final serialized bytes.

## P4 -> P4z

| budget | P4 bytes | P4 RGB | P4z bytes | P4z RGB | P4 SSIM | P4z SSIM | P4 block | P4z block |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 25K | 24,996 | 20.15 | 24,983 | **20.47** | 0.6309 | **0.6330** | 4.005 | **3.691** |
| 50K | 49,993 | 22.79 | 49,970 | **23.11** | 0.7180 | **0.7424** | 2.613 | **2.499** |
| 100K | 99,988 | 25.67 | 98,653 | **25.79** | 0.8299 | **0.8387** | 1.841 | **1.794** |
| 200K | 199,984 | 28.95 | 199,969 | **29.35** | 0.9048 | **0.9143** | 1.361 | **1.351** |

Level 19 is kept. Level 22 was also measured: it changed 50 KB by less than
0.01 dB and cost more CPU, so level 19 is the useful point.

The 100 KB point lands at 98,653 B because the qstep search's next lower step
exceeds 100 KB; it is still a true serialized-size point.

## Current DCT path

- 25 KB: 20.47 dB / SSIM 0.6330 / blockiness 3.691
- 50 KB: 23.11 dB / SSIM 0.7424 / blockiness 2.499
- 100 KB: 25.79 dB / SSIM 0.8387 / blockiness 1.794
- 200 KB: 29.35 dB / SSIM 0.9143 / blockiness 1.351

Compared with P0 scalar coding:

- 50 KB: 20.91 -> 23.11 dB (+2.20 dB)
- 100 KB: 24.24 -> 25.79 dB (+1.55 dB)
- 200 KB: 27.76 -> 29.35 dB (+1.59 dB)

## Files

- p0.md, channels.csv, freq.csv, tiles.csv
- budget_*_diff.png
