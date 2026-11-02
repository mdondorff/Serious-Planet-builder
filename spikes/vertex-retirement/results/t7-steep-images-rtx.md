# T7: steep field (adapter: Vulkan NVIDIA GeForce RTX 3080 Ti Laptop GPU (DiscreteGpu), key vulkan-nvidia-geforce-rtx-3080-ti-laptop-gpu): vertex path (CPU mesh as vertex buffers) against compact 16-bit texels (relief guard off, so cliff tiles are quantised in 16 bits) and against the wide texel
Steep(1), cells 16, finest level 12 (tiles of about 1.5 km with kilometres of relief: the CON-13 fallback case), 960x540. Pixel counts as in T4.

| view | nodes | debug view | candidate | diff px | max | over8 | mean |
|---|---|---|---|---|---|---|---|
| aerial-200km | 767 | normals | compact 16-bit | 1861 | 1 | 0 | 1.00 |
| aerial-200km | 767 | normals | compact wide | 3523 | 1 | 0 | 1.00 |
| aerial-200km | 767 | relief | compact 16-bit | 1874 | 29 | 2 | 1.03 |
| aerial-200km | 767 | relief | compact wide | 1974 | 29 | 2 | 1.02 |
| aerial-200km | 767 | height | compact 16-bit | 792 | 43 | 1 | 1.06 |
| aerial-200km | 767 | height | compact wide | 243 | 43 | 1 | 1.20 |
| low-2km | 1016 | normals | compact 16-bit | 1638 | 52 | 1 | 1.04 |
| low-2km | 1016 | normals | compact wide | 2543 | 1 | 0 | 1.00 |
| low-2km | 1016 | relief | compact 16-bit | 2842 | 20 | 5 | 1.04 |
| low-2km | 1016 | relief | compact wide | 1193 | 1 | 0 | 1.00 |
| low-2km | 1016 | height | compact 16-bit | 1179 | 21 | 3 | 1.06 |
| low-2km | 1016 | height | compact wide | 40 | 1 | 0 | 1.00 |
| ground-200m | 969 | normals | compact 16-bit | 1154 | 52 | 5 | 1.15 |
| ground-200m | 969 | normals | compact wide | 1887 | 1 | 0 | 1.00 |
| ground-200m | 969 | relief | compact 16-bit | 10238 | 34 | 4 | 1.01 |
| ground-200m | 969 | relief | compact wide | 1278 | 1 | 0 | 1.00 |
| ground-200m | 969 | height | compact 16-bit | 5194 | 2 | 0 | 1.03 |
| ground-200m | 969 | height | compact wide | 234 | 1 | 0 | 1.00 |
