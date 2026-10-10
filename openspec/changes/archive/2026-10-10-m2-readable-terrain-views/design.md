## Decisions

- Bands and contours are computed in the fragment stage from values already passed (`height` in metres, clip depth, morph); no new vertex data, no planet-scale values (CON-11). Line widths use `fwidth` so they stay about one pixel on every adapter; goldens are per adapter anyway (CON-20).
- The morph oracle test reads the red channel, so the morph view keeps R = morph value.
- The normals golden uses its own camera instead of a scripted view: the scripted ground views cannot show normal variation because the height field is smooth below 100 km. Mesh and plan code are unchanged.
- Not done: tile-edge overlay lines (would need per-vertex tile coordinates).
