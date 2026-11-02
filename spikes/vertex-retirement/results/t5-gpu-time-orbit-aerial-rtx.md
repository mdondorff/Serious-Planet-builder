# T5: indicative GPU time of the render pass, levels 0-2 candidates (adapter: Vulkan NVIDIA GeForce RTX 3080 Ti Laptop GPU (DiscreteGpu), key vulkan-nvidia-geforce-rtx-3080-ti-laptop-gpu)
2560x1440, Relief debug view, Reference field, cells 32, tau 6, 300 frames interleaved in rotating order, GPU timestamp of the render pass, median / p10 / p90 in ms, ratio to the first row. **Indicative, not a perf window.**

## orbit-20000km

| candidate | median ms | p10 | p90 | ratio |
|---|---|---|---|---|
| current: levels 0-2 vertex format, 3x3 compact (52 nodes, record 128 B) | 0.174 | 0.166 | 0.233 | 1.000 |
| A: levels 0-2 compact, 3x3 (not accurate) (52 nodes, record 128 B) | 0.183 | 0.174 | 0.246 | 1.053 |
| A: levels 0-2 compact, 5x5 base (all tiles) (52 nodes, record 320 B) | 0.205 | 0.198 | 0.273 | 1.176 |
| B: render floor 3, 3x3 compact (181 nodes, record 128 B) | 0.321 | 0.309 | 0.431 | 1.841 |
| B+5x5: floor 3, 5x5 base (all tiles) (181 nodes, record 320 B) | 0.379 | 0.367 | 0.510 | 2.176 |

## aerial-200km

| candidate | median ms | p10 | p90 | ratio |
|---|---|---|---|---|
| current: levels 0-2 vertex format, 3x3 compact (630 nodes, record 128 B) | 2.094 | 2.028 | 2.193 | 1.000 |
| A: levels 0-2 compact, 3x3 (not accurate) (630 nodes, record 128 B) | 2.093 | 2.025 | 2.189 | 1.000 |
| A: levels 0-2 compact, 5x5 base (all tiles) (630 nodes, record 320 B) | 2.299 | 2.228 | 2.406 | 1.098 |
| B: render floor 3, 3x3 compact (630 nodes, record 128 B) | 2.094 | 2.028 | 2.200 | 1.000 |
| B+5x5: floor 3, 5x5 base (all tiles) (630 nodes, record 320 B) | 2.303 | 2.228 | 2.419 | 1.100 |

Label: indicative, taken 2026-10-17 at about 11:31 local time, run time  s; sonnet-explorer confirmed the RTX free beforehand; Rasierklinge, adapter above; GPU clock state not controlled; no perf window; the 3x3 rows build the product's own shader through the flat-record generator only where a flag selects it.
