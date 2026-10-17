# T8: indicative GPU time of the render pass, texel and record layouts (adapter: Vulkan NVIDIA GeForce RTX 3080 Ti Laptop GPU (DiscreteGpu), key vulkan-nvidia-geforce-rtx-3080-ti-laptop-gpu)
Relief debug view, 2560x1440, 300 frames interleaved in rotating order (first row = baseline), GPU timestamp of the render pass, median / p10 / p90 ms, ratio to the first row. **Indicative, not a perf window.** Every row uses storage-buffer blocks (ADR 0054).

## Reference field, ground-1m

| layout | median ms | p10 | p90 | ratio |
|---|---|---|---|---|
| baseline: product shader, 16-bit texel, 3x3 base, buffer blocks (record 128 B) [128 B/slot record] | 1.858 | 1.816 | 1.917 | 1.000 |
| flat-record generator only (16-bit texel, 3x3) [128 B/slot record] | 1.878 | 1.836 | 1.936 | 1.011 |
| wide texel, one layout for all tiles (climate in the record) [452 B/slot record] | 1.867 | 1.828 | 1.926 | 1.005 |
| both decoders, per-slot flag (tiles stay 16-bit) [452 B/slot record] | 1.886 | 1.843 | 1.951 | 1.015 |
| 5x5 base surface, 16-bit texel [320 B/slot record] | 2.741 | 2.679 | 2.829 | 1.476 |
| 5x5 base + wide texel (the full v2 layout) [644 B/slot record] | 2.700 | 2.634 | 2.787 | 1.454 |

## Reference field, aerial-200km

| layout | median ms | p10 | p90 | ratio |
|---|---|---|---|---|
| baseline: product shader, 16-bit texel, 3x3 base, buffer blocks (record 128 B) [128 B/slot record] | 2.096 | 2.051 | 2.195 | 1.000 |
| flat-record generator only (16-bit texel, 3x3) [128 B/slot record] | 2.099 | 2.051 | 2.188 | 1.001 |
| wide texel, one layout for all tiles (climate in the record) [452 B/slot record] | 2.099 | 2.046 | 2.186 | 1.001 |
| both decoders, per-slot flag (tiles stay 16-bit) [452 B/slot record] | 2.103 | 2.051 | 2.199 | 1.003 |
| 5x5 base surface, 16-bit texel [320 B/slot record] | 2.301 | 2.246 | 2.405 | 1.098 |
| 5x5 base + wide texel (the full v2 layout) [644 B/slot record] | 2.297 | 2.242 | 2.396 | 1.096 |

## Steep field, ground-200m (4160 nodes of 16 cells, finest level 12; 16-bit rows quantise cliff tiles coarsely: timing only)

| layout | median ms | p10 | p90 | ratio |
|---|---|---|---|---|
| baseline: product shader, 16-bit texel, 3x3 base, buffer blocks (record 128 B) [128 B/slot record] | 1.158 | 1.143 | 1.292 | 1.000 |
| flat-record generator only (16-bit texel, 3x3) [128 B/slot record] | 1.164 | 1.150 | 1.302 | 1.005 |
| wide texel, one layout for all tiles (climate in the record) [452 B/slot record] | 1.159 | 1.145 | 1.293 | 1.001 |
| both decoders, per-slot flag (tiles stay 16-bit) [452 B/slot record] | 1.168 | 1.153 | 1.305 | 1.009 |
| 5x5 base surface, 16-bit texel [320 B/slot record] | 1.571 | 1.535 | 1.780 | 1.356 |
| 5x5 base + wide texel (the full v2 layout) [644 B/slot record] | 1.550 | 1.517 | 1.756 | 1.339 |

Label: indicative, taken 2026-10-17 at about 11:31 local time, run time  s; sonnet-explorer confirmed the RTX free beforehand; Rasierklinge, adapter above; GPU clock state not controlled; no perf window.
