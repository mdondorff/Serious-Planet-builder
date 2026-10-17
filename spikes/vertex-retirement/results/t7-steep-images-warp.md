# T7: steep field (adapter: Dx12 Microsoft Basic Render Driver (Cpu), key dx12-microsoft-basic-render-driver): vertex path (CPU mesh as vertex buffers) against compact 16-bit texels (relief guard off, so cliff tiles are quantised in 16 bits) and against the wide texel
Steep(1), cells 16, finest level 12 (tiles of about 1.5 km with kilometres of relief: the CON-13 fallback case), 960x540. Pixel counts as in T4.

| view | nodes | debug view | candidate | diff px | max | over8 | mean |
|---|---|---|---|---|---|---|---|
| aerial-200km | 767 | normals | compact 16-bit | 1818 | 7 | 0 | 1.01 |
| aerial-200km | 767 | normals | compact wide | 3491 | 7 | 0 | 1.00 |
| aerial-200km | 767 | relief | compact 16-bit | 1884 | 43 | 5 | 1.08 |
| aerial-200km | 767 | relief | compact wide | 1863 | 43 | 5 | 1.09 |
| aerial-200km | 767 | height | compact 16-bit | 779 | 53 | 4 | 1.18 |
| aerial-200km | 767 | height | compact wide | 310 | 53 | 4 | 1.46 |
| low-2km | 1016 | normals | compact 16-bit | 1673 | 8 | 0 | 1.01 |
| low-2km | 1016 | normals | compact wide | 2493 | 1 | 0 | 1.00 |
| low-2km | 1016 | relief | compact 16-bit | 2891 | 19 | 4 | 1.03 |
| low-2km | 1016 | relief | compact wide | 1239 | 1 | 0 | 1.00 |
| low-2km | 1016 | height | compact 16-bit | 1247 | 21 | 3 | 1.05 |
| low-2km | 1016 | height | compact wide | 70 | 1 | 0 | 1.00 |
| ground-200m | 969 | normals | compact 16-bit | 1147 | 39 | 4 | 1.10 |
| ground-200m | 969 | normals | compact wide | 1915 | 1 | 0 | 1.00 |
| ground-200m | 969 | relief | compact 16-bit | 10034 | 34 | 3 | 1.01 |
| ground-200m | 969 | relief | compact wide | 1555 | 1 | 0 | 1.00 |
| ground-200m | 969 | height | compact 16-bit | 5104 | 2 | 0 | 1.03 |
| ground-200m | 969 | height | compact wide | 433 | 1 | 0 | 1.00 |
