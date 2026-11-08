# RFI Frequency Format 1.0

This document defines the new RFI format. It is not compatible with the former
RFI v1 format. A parser that sees the old RFI signature must reject the file;
the old parser must reject this format.

## Name and extension

- Name: RFI (RGBA Function Image)
- Extension: .rfi
- New signature: 52 46 49 0D 0A 1A 0A 00 ("RFI" CR LF EOF LF NUL)
- No v1 compatibility, no conversion path, no dual parser.

## Mathematical model

The image is represented by basis expansions, not a pixel matrix and not an
RGBA residual:

- luma: Y = IDCT(quantized DCT coefficients)
- chroma: Cb, Cr = IDCT(quantized DCT coefficients), at 4:2:0 resolution
- RGB = YCbCr to RGB after reconstruction

DCT coefficients are function-expansion parameters. Basis id 1 is DCT8 and basis
id 3 is DCT16. The per-file selector evaluates both bases with both entropy
backends. Basis id 2 is retired: the multilevel 5/3 wavelet with EZW zerotree was
measured 5.1-7.6 dB behind DCT at every budget and has been removed from the
codec. A decoder must reject basis id 2 as unknown.

## Header (96 bytes, little endian)

| offset | size | field |
|---|---|---|
| 0 | 8 | signature |
| 8 | 2 | version major = 1 |
| 10 | 2 | version minor = 0 |
| 12 | 2 | header size = 96 |
| 14 | 1 | channel count (3 color + opaque alpha) |
| 15 | 1 | tile size log2 = 6 |
| 16 | 4 | width |
| 20 | 4 | height |
| 24 | 2 | section count |
| 26 | 2 | primary basis id |
| 28 | 4 | quantization step f32 |
| 32 | 4 | section table offset = 96 |
| 36 | 8 | file size |
| 44 | 4 | header CRC32C, header bytes 44..48 zeroed |
| 48 | 1 | color mode: 1 = YCbCr 4:2:0, opaque alpha |
| 49 | 1 | quantization matrix id: 1 = fixed luma/chroma matrices |
| 50 | 46 | reserved |

## Section table entry (32 bytes)

| offset | size | field |
|---|---|---|
| 0 | 2 | section type |
| 2 | 1 | codec (0 raw, 1 zstd, 2 order-0 rANS, 4 bucketed frame) |
| 3 | 1 | flags |
| 4 | 4 | CRC32C of uncompressed bytes |
| 8 | 8 | stored offset |
| 16 | 8 | stored size |
| 24 | 8 | uncompressed size |

Sections: 1 basis, 3 luma DC, 4 AC counts, 5 AC positions, 6 AC values, 7 AC
count extensions, 8 chroma DC, 9 metadata (optional). Section 2 (tile directory)
is reserved for a future per-tile basis selection; it is absent in the current
basis-global format. Unknown required sections reject the file.

## Basis section

u16 basis id, u16 block width, u16 block height, u16 flags. DCT8 is id 1.

## Tile layout

There is no per-tile directory in the current format. The tile grid is derived
from width and height with tile size 64. Tiles are row-major; each tile encodes
64 luma DCT blocks and 16 Cb + 16 Cr DCT blocks in a fixed order, and the
coefficient stream is decoded sequentially. This removes the 16,320-byte raw
directory (and the 3,000-3,500-byte zstd directory) at 1920x1080.

Section 2 is reserved for a future per-tile basis directory. A file that uses
section 2 must describe its layout in a later format revision; the current
decoder does not accept it.

## Quantization matrices (id 1)

Natural 8x8 order, row-major.

LUMA_MATRIX (soft high-frequency weighting, 0.6 of the JPEG table above 16):

~~~
8 13 12 16 22 33 41 48
13 13 15 18 23 45 47 43
15 14 16 22 33 45 53 44
15 17 20 25 41 66 61 48
17 20 31 44 52 81 77 59
22 29 43 50 62 78 84 69
39 50 59 66 77 90 89 76
55 69 71 73 83 75 77 74
~~~

CHROMA_MATRIX:

~~~
9 18 24 47 99 99 99 99
18 21 26 66 99 99 99 99
24 26 56 99 99 99 99 99
47 66 99 99 99 99 99 99
99 99 99 99 99 99 99 99
99 99 99 99 99 99 99 99
99 99 99 99 99 99 99 99
99 99 99 99 99 99 99 99
~~~

The header stores the matrix id; a decoder that does not know the id must reject
the file. Matrix id 1 is the only matrix in this version. The DC weights are
finer than the AC weights (luma DC 8, chroma DC 9) to spend more of the budget
on the low-frequency base.

## Entropy codecs

Each coefficient section carries its own codec id, so a single file may mix
backends. Codec 1 is zstd level 19. Codec 2 is order-0 rANS (32-bit state,
12-bit probability precision, byte renormalisation) with its static frequency
table stored at the start of the section payload. The basis section is always
zstd because it is 8 bytes of metadata.

The encoder exposes both backends, and it can also choose per section: each
coefficient section is compressed with both, and the smaller result is stored
with its codec id. Mixed-codec files are therefore normal, not an edge case.

Codec 4 is a bucketed frame: a bucket count byte, then per bucket a codec byte,
a raw length u32, a stored length u32 and the stored bytes. Each bucket
independently uses codec 1 or 2. The AC position section uses this frame, with
buckets keyed by the previous zigzag position divided by 8; both encoder and
decoder derive the bucket from state they already track, so no side data is
stored. A bucketed section is checksummed over its stored bytes rather than a
single uncompressed form.
## DCT coefficient coding

- 8x8 DCT-II, orthonormal scaling.
- YCbCr conversion before DCT, Cb/Cr 4:2:0 (2x2 average).
- Quantization matrix id 1: luma and chroma use separate fixed JPEG-style
  8x8 frequency matrices. The effective step for coefficient (u,v) is
  step * M[u,v] / 16, with M = LUMA_MATRIX for Y and CHROMA_MATRIX for Cb/Cr.
  The encoder and decoder must use the same matrices.
- Coefficients in zigzag order.
- DC prediction is the JPEG-LS MED predictor over the block grid, per channel
  (Y, Cb, Cr each have their own grid). The grid spans tiles, so there is no
  reset at tile boundaries. Border blocks use only available neighbours; a block
  with no neighbours predicts 0.
- DC differences are zigzag signed LEB128, split into a luma DC section and a
  chroma (Cb+Cr) DC section.
- AC count stream: two 4-bit counts per byte. Counts 0-14 fit directly; nibble
  15 means the real count is in the AC count extension section (one byte).
- AC position stream: one run-length byte per nonzero. Run length is the number
  of zero coefficients between the previous nonzero and this one; the decoder
  reconstructs the absolute zigzag position.
- AC value stream: zigzag signed LEB128 value per nonzero.
- Golomb-Rice for AC values was implemented and measured; it did not beat
  varint+zstd, so the format keeps varint. See reports/p9rice-1920/README.md.
- DC, AC counts, count extensions, AC positions and AC values are compressed as
  separate zstd sections.
- Alpha is opaque in this version; a future alpha section must be a new flag,
  not a reinterpretation of the color stream.

The old RGBA residual section does not exist. Pixel residual is not a v1 fallback
of this format.

## Rate control

The encoder exposes a scalar quantization step. A rate-distortion curve is
produced by encoding at a sequence of steps, measuring the final stored size
including header, tile table and zstd, and decoding for PSNR. No estimated byte
count is accepted as the final size.

## Limits

- width and height must be non-zero and fit u32.
- section offsets/sizes must be checked with checked arithmetic.
- coefficient offsets must be inside the section.
- unknown basis/color modes reject the file.
- old RFI signature is rejected before any other parse.

## Current limitations

- The wavelet basis was removed after measuring it 5.1-7.6 dB behind DCT. Basis
  id 2 is rejected; there is no wavelet decoder in the codec.
- Alpha is opaque.
- The prototype uses scalar step quantization, not a visually weighted matrix.
- The DCT prototype is not yet competitive with WebP/JPEG at very low rates;
  it is the first new-format implementation and the RD baseline for v-next.