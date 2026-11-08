# R4: rANS entropy layer on the DCT path

Second half of the requested comparison: replace the coefficient entropy layer
with rANS and measure it against the wavelet+zerotree result.

## Implementation

- Order-0 rANS: 32-bit state, 12-bit probability precision, byte-wise
  renormalisation, static per-section frequency table stored in the payload
- The whole coefficient entropy layer was replaced, not one stream:
  DC luma, DC chroma, AC counts, AC count extensions, AC positions and AC
  values all go through rANS
- Section table byte 2 now carries the codec id (1 = zstd, 2 = rANS); the basis
  section stays zstd because it is 8 bytes of metadata
- Decoder dispatches on that codec id for both the DCT8 and DCT16 readers
- `Entropy` enum plus `encode_with`, `encode_to_budget_with`,
  `encode16_with`, `encode16_to_budget_with` give a fair, selectable backend
- rANS round-trip unit test covers empty input, single byte, long runs,
  near-uniform and bimodal distributions

## DCT8: zstd vs rANS (1920x1080, true serialized bytes)

| budget | zstd bytes | zstd RGB | rANS bytes | rANS RGB | zstd SSIM | rANS SSIM |
|---:|---:|---:|---:|---:|---:|---:|
| 25 KB | 24,998 | 21.95 | 25,000 | 21.93 | 0.6542 | 0.6468 |
| 50 KB | 49,887 | 24.29 | 49,997 | 24.25 | 0.7645 | 0.7702 |
| 100 KB | 99,103 | 27.02 | 99,999 | 27.08 | 0.8682 | 0.8697 |
| 200 KB | 199,394 | 30.63 | 199,987 | 30.93 | 0.9354 | 0.9393 |

rANS loses at 25-50 KB, wins at 100-200 KB (+0.06 dB and +0.30 dB). Its 200 KB
SSIM also improves (0.9354 -> 0.9393).

Section comparison at 200 KB:

| section | zstd | rANS |
|---|---:|---:|
| luma DC | 18,696 | 20,003 |
| chroma DC | 5,296 | 5,455 |
| AC counts | 14,114 | 15,904 |
| AC count ext | 3,808 | 3,816 |
| AC positions | 58,909 | 52,922 |
| AC values | 98,234 | 101,550 |

rANS wins clearly on AC positions (-6.0 KB) but loses on DC, counts and AC
values. Order-0 rANS only helps streams whose byte distribution is close to
stationary; DC differences and counts are small-valued and the varint+zstd
pair already models them well.

## Head-to-head: wavelet+EZW vs DCT entropy variants

| budget | wavelet RGB | DCT8+zstd | DCT8+rANS | DCT16+zstd | DCT16+rANS | best |
|---:|---:|---:|---:|---:|---:|---|
| 25 KB | 14.86 | 21.95 | 21.93 | 22.41 | 22.11 | DCT16+zstd |
| 50 KB | 17.42 | 24.29 | 24.25 | 24.37 | 23.75 | DCT16+zstd |
| 100 KB | 21.26 | 27.02 | 27.08 | 26.89 | 25.73 | DCT8+rANS |
| 200 KB | 25.56 | 30.63 | 30.93 | 30.45 | 28.34 | DCT8+rANS |

## Which experiment came out better

rANS, decisively. On this photo:

- wavelet+EZW is 5.1 to 7.6 dB behind the DCT path at every budget
- rANS matches or slightly trails DCT+zstd at 25-50 KB and beats it at
  100-200 KB, and it is the winning backend at 100 KB (+0.06 dB) and 200 KB
  (+0.30 dB)

Note DCT16+rANS is worse than DCT16+zstd (28.34 vs 30.45 dB at 200 KB): the
DCT16 stream layout suits zstd better. The head-to-head selector therefore
evaluates both backends per basis and keeps the better point.

## Honest limits

- rANS is order-0 with a static table and no context model. The AC positions
  gain shows where context modelling would pay off further; DC and counts would
  need order-1 or per-frequency contexts to improve.
- The +0.30 dB at 200 KB is real but modest, and the 25-50 KB regression means
  rANS is not a universal win.
- Nothing here changes the earlier conclusion that no configuration reaches
  50 KB / 50 dB on this photo.

## Files

- crates/rfi-frequency/src/rans.rs
- reports/r4-1920/ (zstd vs rANS per budget)
- reports/c1-1920/ (head-to-head selected files)
## Addendum: the encoder selector now evaluates all four combinations

After the measurements above, the per-file selector was extended to evaluate
basis x backend at every budget instead of only the two bases. Result:

| budget | selected | bytes | RGB | SSIM | MS-SSIM | blockiness |
|---:|---|---:|---:|---:|---:|---:|
| 25 KB | DCT16+zstd | 24,995 | 22.41 | 0.7023 | 0.8850 | 1.966 |
| 50 KB | DCT16+zstd | 49,998 | 24.37 | 0.7798 | 0.9334 | 1.567 |
| 100 KB | DCT8+rANS | 99,999 | 27.08 | 0.8697 | 0.9687 | 1.659 |
| 200 KB | DCT8+rANS | 199,987 | 30.93 | 0.9393 | 0.9884 | 1.335 |

So rANS is not merely a side experiment: it becomes the chosen backend in the
final configuration for the 100 KB and 200 KB operating points.

Reproduction:

~~~
cargo run -p rfi-cli --release -- best-rd /tmp/photo1920.jpg reports/r5-best
~~~
## Final state after completing the rANS work

The rANS work is now closed. What was tried and what survives:

| attempt | outcome |
|---|---|
| order-0 rANS on all coefficient sections | kept; wins 100 KB (+0.06 dB) and 200 KB (+0.30 dB) |
| Golomb-Rice on AC values (P9) | rejected, reverted |
| adaptive binary rANS with per-band models | abandoned: never passed its round-trip test |
| banded order-0 rANS, 16 bands (X2) | rejected, removed; per-stream table cost |

Verified current encoder selection (evaluates DCT8/DCT16 x zstd/rANS):

| budget | selected | bytes | RGB |
|---:|---|---:|---:|
| 25 KB | DCT16+zstd | 24,995 | 22.41 |
| 50 KB | DCT16+zstd | 49,998 | 24.37 |
| 100 KB | DCT8+rANS | 99,999 | 27.08 |
| 200 KB | DCT8+rANS | 199,987 | 30.93 |

## Cleanup performed in the same round

- deleted `wavelet.rs` (702 lines) and the whole wavelet/EZW basis id 2 path,
  because it lost by 5.1-7.6 dB in W6
- removed `encode_wavelet`, `encode_wavelet_to_budget`, the wavelet CLI commands
  and the viewer's wavelet label
- basis id 2 is now rejected as an unknown basis (verified: decoding an old
  wavelet file fails with "unknown RFI basis")
- removed `compare-rd`, which only existed to compare against wavelet
- archived superseded experiment artifacts under `reports/archive/`
- deleted the never-working adaptive binary rANS and the banded coder
## A1: per-section backend selection (the step that actually wins)

`Entropy::Best` compresses every coefficient section with both backends and keeps
the smaller one. Because the section table already stores a codec id per
section, one file can freely mix zstd and rANS.

| budget | previous best | new best | bytes | RGB gain |
|---:|---|---:|---:|---:|
| 25 KB | DCT16+zstd 22.41 | DCT16+auto 22.47 | 24,998 | +0.06 |
| 50 KB | DCT16+zstd 24.37 | DCT16+auto 24.40 | 49,998 | +0.03 |
| 100 KB | DCT8+rANS 27.08 | DCT8+auto 27.22 | 99,994 | +0.14 |
| 200 KB | DCT8+rANS 30.93 | DCT8+auto 31.03 | 199,930 | +0.10 |

Selected operating points:

| budget | selected | bytes | RGB | SSIM | MS-SSIM | blockiness |
|---:|---|---:|---:|---:|---:|---:|
| 25 KB | DCT16+auto | 24,998 | 22.47 | 0.7042 | 0.8873 | 1.950 |
| 50 KB | DCT16+auto | 49,998 | 24.40 | 0.7809 | 0.9339 | 1.563 |
| 100 KB | DCT8+auto | 99,994 | 27.22 | 0.8725 | 0.9702 | 1.655 |
| 200 KB | DCT8+auto | 199,930 | 31.03 | 0.9405 | 0.9889 | 1.342 |

The mixed codecs are real. At 50 KB the section table reads:

| section | codec | stored bytes |
|---|---:|---:|
| basis | 1 (zstd) | 17 |
| luma DC | 1 | 3,712 |
| chroma DC | 2 (rANS) | 983 |
| AC counts | 1 | 4,656 |
| AC positions | 1 | 25,762 |
| AC values | 2 | 14,580 |

At 200 KB the same file type chooses rANS for positions, values and count
extensions, and zstd for luma DC, chroma DC and counts.

Interpretation: no single backend is right for every stream. rANS suits the
high-entropy position/value streams, zstd suits the small-valued DC and count
streams. Choosing per section recovers the best of both without any new format
machinery - the codec id field already existed.

Reproduction:

~~~
cargo run -p rfi-cli --release -- best-rd /tmp/photo1920.jpg reports/a1-1920
~~~
## B1: bucketing the AC run stream by previous zigzag position (marginal)

AC positions is the largest single section, so it was the obvious next target.
The run length distribution depends strongly on where the previous nonzero sits:
early positions have short runs, late positions have long ones. zstd sees one
mixed distribution.

Implementation: the run byte stream is split into `POS_BUCKETS` sub-streams keyed
by `previous_position / POS_BUCKETS`. Encoder and decoder both derive the bucket
from already-known state, so this needs no side data. Each bucket picks its own
codec (zstd or rANS), framed by codec id 4 (`CODEC_BUCKETED`).

| bucket count | 25 KB | 50 KB | 100 KB | 200 KB |
|---:|---:|---:|---:|---:|
| 1 (previous) | 22.47 | 24.40 | 27.22 | 31.03 |
| 8 | 22.47 | 24.40 | 27.23 | 31.08 |
| 16 | 22.47 | 24.40 | 27.21 | 31.05 |

Section size at 100 KB:

| config | AC positions stored |
|---|---:|
| 1 bucket | 31,683 |
| 8 buckets | 31,440 |
| 16 buckets | larger tables outweighed the split |

## Honest verdict

This is a **marginal** change: about 243 B saved on the 31.7 KB positions section
(0.77%), worth +0.01 dB at 100 KB and +0.05 dB at 200 KB. 16 buckets regress at
high rate because each bucket pays its own frequency table.

8 buckets is kept because it never regresses and helps slightly at 200 KB, but it
should not be presented as a real rate-distortion breakthrough.

The structural finding is more useful than the number: bucketing by a
coder-derivable state works and costs no side data, so a stronger model can reuse
the same mechanism without format changes.

Reproduction:

~~~
cargo run -p rfi-cli --release -- best-rd /tmp/photo1920.jpg reports/b3-1920
~~~
