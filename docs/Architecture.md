# Architecture

- rfi-frequency: new container, DCT8 codec, YCbCr 4:2:0, coefficient coding.
- rfi-cli: new-format CLI.
- rfi-viewer: egui/eframe/wgpu viewer.

Encoding: sRGB RGBA8 -> YCbCr 4:2:0 -> DCT8 -> quantize -> DC prediction ->
zero-run coding -> zstd -> new RFI container. No RGBA residual section.

Decoding: validate signature/CRC -> decompress -> inverse DCT -> upsample chroma
-> YCbCr to RGB -> opaque RGBA.

Viewer uses rfi-frequency only; it supports fit/zoom, checkerboard, tile grid,
drag/drop, headless --check and --screenshot.
