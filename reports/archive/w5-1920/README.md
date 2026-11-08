# W5: rejected chroma scale 0.1

Tested a finer chroma step (x0.1). Chroma PSNR rose (27.6 -> 33.4 dB at 25 KB)
but luma PSNR fell (13.94 -> 12.75 dB) because the rate controller had to
coarsen luma to stay in budget; total RGB PSNR dropped at every point.

Rejected; the codec keeps CHROMA_STEP_SCALE = 0.25. Details in
[reports/w6-1920/README.md](../w6-1920/README.md).
