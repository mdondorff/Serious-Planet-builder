# T1: position error of the compact base surface against the vertex-format mesh, by level (CPU, Reference field, cells 32)
Tiles: 24 random per level (seed 12). Error = largest distance over all grid and skirt vertices of position and morph target between the CPU mirror (shade_all) and tile_mesh_field. px = metres / (tile edge / 190), SPIKE-06's convention (a 190-pixel tile).

| level | tile edge m | 3x3 max m | 3x3 max px | 5x5 max m | 5x5 max px | 5x5 p50 px |
|---|---|---|---|---|---|---|
| 0 | 1.001e7 | 1.911e5 | 3.629 | 1.847e4 | 0.35059 | 0.34979 |
| 1 | 5.004e6 | 3.665e4 | 1.392 | 1.035e3 | 0.03930 | 0.03916 |
| 2 | 2.502e6 | 6.941e3 | 0.527 | 2.603e1 | 0.00198 | 0.00101 |
| 3 | 1.251e6 | 1.070e3 | 0.163 | 1.276e0 | 0.00019 | 0.00006 |
| 4 | 6.255e5 | 1.242e2 | 0.038 | 1.289e-1 | 0.00004 | 0.00003 |
| 5 | 3.127e5 | 1.213e1 | 0.007 | 6.693e-2 | 0.00004 | 0.00003 |
| 6 | 1.564e5 | 1.331e0 | 0.002 | 3.494e-2 | 0.00004 | 0.00003 |
| 7 | 7.818e4 | 2.686e-1 | 0.001 | 1.963e-2 | 0.00005 | 0.00003 |
| 8 | 3.909e4 | 2.899e-2 | 0.000 | 8.415e-3 | 0.00004 | 0.00003 |
