# Seams in the base (follow-up of SPIKE-47, opus-planner 2026-10-27, 1 h)

Question: the hillshade stills of `results.md` show rectangular seams in the base. What block size and alignment, which surface shows them, and does the real renderer show them?

## (a) Size and alignment
- Chunk of site 3 (lat 8.972, lon -16.238), region lattice i 397258..398613, j 751114..752469 (1356 x 1356 cells of 8.04 m), core starts at i 397632, j 751488.
- Height steps: none. No column or row boundary of the base has a mean absolute step over 2.5 times the median (median 0.91 m per cell).
- Gradient creases (signed mean of the second difference across each lattice line, `crease_report` in `proto/tribspike.rs.txt`; `raw/creases_site3.txt`): 23 columns and 25 rows over 6 times the median in the base. The strongest (0.3 to 0.6 m of change in the mean slope per cell, 12 to 25 times the median, about 2 to 4 degrees) sit at absolute lattice indices that are multiples of 76: columns 397327/8, 397479/80, 397631/2, 397783/4, 397935/6 (678), 398087/8, 398391/2, 398543/4, and rows 751183/4, 751411/2, 751943/4 (the core's own edge i 397632 and j 751488 are multiples of 76 too).
- 76 cells are 611 m: one cell of the 16384 bake (a face edge of 16384 cells, ADR 0027 point 3). The strongest creases fall on every second bake edge, 152 cells (1.22 km); the others on the odd edges (397707/8: 0.25 to 0.29 m). Not aligned with the chunk (608 cells = 8 bake cells; a crease at the chunk edge exists but so do creases inside it).
- Cause: the bake is read by bilinear interpolation (`global.rs` `read_tile_sample`, ADR 0027): a bilinear surface has a gradient jump at every cell edge. Why every second edge is stronger was not looked at.

## (b) Which surface
- The base (the input to the chunk) holds them: the hillshade panel "base" in `images/site3_8.972_-16.238.png` (top left) shows the block pattern; the crease table above is of `net.base_z`.
- The chunk's output keeps them where the solve does not overwrite the ground: production's final heights show creases at columns 222 and 1286 and rows 70/71, 297/298 of the same lattice (all multiples of 76; 8 to 11 times the median, which is higher there because the solve adds 10 m of roughness), and the production panel of the still shows the same blocks softened.
- Not the hillshade alone: the pattern is in the heights.

## (c) The real renderer
- Not settled. `docs/milestones/M8/look-review-2b/hills-b/default/{300m,30m}-{midday,sunset}.png` is this chunk. At midday the light is flat and no block is visible; at sunset the near ground is in shadow and the lit band beyond 1 km shows smooth ground without rectangles. A low-sun pose with the near ground lit (sun 15 to 25 degrees, looking along the sun) would show a crease of 2 to 4 degrees; none of the six stills is that pose, and a new capture needs the accept store (`target/accept/global16384`), not run here.
