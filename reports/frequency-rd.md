# RFI Frequency Codec Rate-Distortion

Date: 2026-11-08. New format prototype: rfi-frequency. Input: real 1920x1080
JPEG (386,694 B), decoded RGBA8. PSNR is sRGB RGBA8 against the decoded source.

## DCT/YCoCg 4:2:0 points

| quantization step | stored bytes | PSNR |
|---:|---:|---:|
| 4 | 773,777 | 47.69 dB |
| 8 | 679,404 | 44.60 dB |
| 12 | 605,934 | 40.89 dB |
| 16 | 527,111 | 38.45 dB |
| 20 | 482,968 | 36.93 dB |
| 30 | 391,609 | 34.19 dB |
| 40 | 334,310 | 32.07 dB |
| 60 | 239,945 | 28.98 dB |
| 80 | 178,692 | 27.13 dB |
| 120 | 109,994 | 24.71 dB |
| 160 | 79,792 | 22.95 dB |
| 200 | 60,200 | 21.64 dB |
| 250 | 45,671 | 20.56 dB |
| 300 | 34,821 | 19.99 dB |
| 400 | 23,397 | 19.00 dB |
| 500 | 17,558 | 18.35 dB |

Budget selection (largest point at or below the budget):

| budget | chosen bytes | PSNR | old hybrid PSNR | improvement |
|---:|---:|---:|---:|---:|
| 25,000 | 23,397 | 19.00 | - | n/a |
| 50,000 | 45,671 | 20.56 | 18.54 at 100 KB | not comparable |
| 100,000 | 79,792 | 22.95 | 18.54 | +4.41 dB |
| 200,000 | 178,692 | 27.13 | 18.76 | +8.37 dB |
| 500,000 | 482,968 | 36.93 | 19.40 | +17.53 dB |
| 765,000 | 605,934 | 40.89 | 20.44 | +20.45 dB |
| 1,000,000 | 773,777 | 47.69 | 20.44 at 765 KB | large |

## External codec controls

| codec | bytes | PSNR |
|---|---:|---:|
| WebP q5 | 90,734 | 28.61 |
| JPEG q30 | 102,749 | 27.75 |
| JPEG q20 | 140,060 | 29.83 |
| JPEG q5 | 359,228 | 41.57 |

The new frequency basis is a large improvement over the old RFI hybrid curve,
but it is still behind WebP/JPEG at 100-500 KB. At 100 KB: DCT 22.95 dB vs
WebP 28.61 dB at 91 KB. The first-stage requirement (improve the old RFI
100 KB-765 KB range) is met; the codec is not yet competitive with mature
codecs.

## Missing points

- There is still no 50 KB / 50 dB point. The 50 KB DCT point is 45,671 B at
  20.56 dB; the quality is far below target. The coefficient/DC overhead and
  scalar quantization are the next bottlenecks.
- Alpha is opaque; no alpha channel.

## Reproduction

~~~
cargo run -p rfi-frequency --release -- rd /tmp/photo1920.jpg reports/frequency-rd-ycbcr2
cargo run -p rfi-frequency --release -- encode <image> <out.rfi> <step>
cargo run -p rfi-frequency --release -- decode <out.rfi> <out.png>
~~~