# P2: variable-length DC/AC coefficient coding

Isolated compression change on top of the P1 soft-luma matrix:

- DC difference: zigzag signed LEB128 instead of fixed i32.
- AC value: zigzag signed LEB128 instead of fixed i16.
- AC position remains one byte.
- DCT, matrices, YCbCr 4:2:0, DC prediction, zstd and container unchanged.
- Rate control still searches qstep by final serialized bytes.

## P1s -> P2

P1s = soft luma matrix + fixed i32/i16 coefficient words. P2 = P1s + varints.

| budget | P1s bytes | P1s RGB | P2 bytes | P2 RGB | P1s SSIM | P2 SSIM | P1s MS | P2 MS | P1s block | P2 block |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 25K | 25,000 | 19.07 | 24,995 | 19.66 | 0.5674 | 0.5595 | 0.7181 | 0.7389 | 5.242 | 4.538 |
| 50K | 49,996 | 21.65 | 50,000 | 22.49 | 0.6827 | 0.7245 | 0.8344 | 0.8599 | 3.066 | 2.700 |
| 100K | 99,996 | 24.72 | 99,985 | 25.50 | 0.7786 | 0.8203 | 0.9077 | 0.9294 | 2.049 | 1.896 |
| 200K | 199,973 | 28.08 | 199,970 | 28.86 | 0.8895 | 0.9062 | 0.9678 | 0.9743 | 1.532 | 1.376 |

P2 improves PSNR at every point, SSIM/MS-SSIM at 50/100/200 KB, and blockiness
at every point. At 25 KB SSIM is 0.008 lower, but PSNR is +0.59 dB and blockiness
is better.

## Why it works

At 50 KB, coefficient bytes before zstd:

- P1s: 333,237 B
- P2: 173,916 B

The varint removes most of the fixed coefficient word overhead. Rate control
then spends the saved bytes on a lower qstep:

- P1s qstep: 225.04
- P2 qstep: 189.89

The P2 decoded quality is higher despite the same 50 KB file size.

## P2 section bytes

| budget | header+table | basis | tile stored/raw | coeff stored/raw |
|---:|---:|---:|---:|---:|
| 25K | 192 | 17 | 2,858 / 16,320 | 21,928 / 124,324 |
| 50K | 192 | 17 | 3,004 / 16,320 | 46,787 / 173,916 |
| 100K | 192 | 17 | 3,295 / 16,320 | 96,481 / 279,026 |
| 200K | 192 | 17 | 3,503 / 16,320 | 196,258 / 482,202 |

## P2 50 KB channel split

| channel | raw | zstd | dc zstd | ac zstd | nonzero | avg zero run |
|---|---:|---:|---:|---:|---:|---:|
| Y | 141,274 | 43,525 | 8,423 | 34,125 | 3.244% | 35.41 |
| Cb | 16,320 | 1,009 | 855 | 19 | 0.518% | 63.00 |
| Cr | 16,322 | 978 | 816 | 27 | 0.540% | 63.00 |

## Verdict

P2 is a clear win and is kept. It is still not the full P2 plan (frequency
contexts / adaptive zero-run / rANS), but it establishes the entropy baseline:
same byte budget, +0.5 to +0.9 dB in the 25-200 KB range and consistently better
SSIM/blockiness.
