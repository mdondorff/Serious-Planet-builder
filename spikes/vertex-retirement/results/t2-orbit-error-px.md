# T2: screen-space error of the compact base surface at the standard views (2560x1440, tau 6, fov 60, Reference field, cells 32)
Per drawn node: error = largest vertex distance between the compact mirror and the vertex mesh (position and morph target); pixel size at the node = distance to the node's near side * 2 tan(fov/2) / 1440. Only nodes of level < 3 are listed per view (the others are compact today).

| view | base | level 0-2 nodes | worst error m | worst error px | mean error px |
|---|---|---|---|---|---|
| orbit-20000km | 3x3 | 52 | 6.928e3 | 0.4633 | 0.2546 |
| orbit-20000km | 5x5 | 52 | 2.615e1 | 0.0018 | 0.0008 |
