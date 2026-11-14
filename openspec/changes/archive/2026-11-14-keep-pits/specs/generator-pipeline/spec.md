## ADDED Requirements
### Requirement: GEN-120 The bake keeps its pits and records how deep each lies below its spill level
The bake's carves SHALL keep the pits of the carved surface in the layer's change and SHALL record each pit's depth, so that the runtime shows the filled look until lake water and a pit-tolerance rule exist:
- pass 1 (GEN-100): the route runs over the filled copy (the flood's receivers and area), but a cell that stands below its receiver is not raised to it; it gains its uplift only, so the donors above it carve toward its floor and the basin does not eat the carve's relief (SPIKE-102 run (b): the high mountains' 3 km relief p50 0.85 of Everest's against 0.62, the mountains' 0.94 of the Alps' against 0.86, and 0.85 and 0.94 on the filled surface too). The option is `keep_pits` (on); an incise-only carve (pass 2) never raises, so it ignores it;
- pass 2 keeps its pits (its local carve only incises) and leaves the tile's final heights as bake scratch (STRM-077); it computes no depth of its own: a window's flood cannot know a basin wider than its halo (the keep-pits preview: 152 m of disagreement across a tile seam);
- the depth of a core cell is how far the surface the runtime shows (the exact base plus the stored change) lies below the planet-wide spill level of the pass-2 surface (GEN-121), clamped at 0, `round(depth * FILL_STEPS_PER_M)` in a 16-bit code; only the cells that are pits of the pass-2 surface (the level above their final height) are given a depth, and a cell whose exact surface stands at or above the level (an outcrop inside the basin) keeps its height with depth 0; so the filled floor of a basin is flat against the bake's exact base (`BakeInputs::base`) to the code's step, the interpolation ripple of the pass-2 base being taken out; the runtime shows the stored bake's lattice read plus the crease detail and fades the change near the coast, so a floor at runtime differs from the bake's by the lattice read's error (docs/learnings.md 2026-11-04: 3.4 m rms on mountains) and the detail, which the depth does not touch (4 steps a metre, 0.25 m, owner 2026-11-05; the step is a parameter of the channel's key); a cell at or below the sea level, and a cell on a flat at its spill level, hold 0;
- the planes SHALL be the same bytes on any thread count (CON-14);
- a reader that shows the filled look SHALL add the depth back to the stored change (`TileResult::with_fill_added`); a tile without a fill plane is read as it is.

Verify: A
Status: active
Source: Owner 2026-11-05 (SPIKE-102 D1: keep every pit that survives the breach, store the fill depth, the runtime adds it back); SPIKE-102 section 4.1 row 2 and section 6; planner inbox #67 (the post-pass); GEN-100, GEN-101, GEN-121

#### Scenario: A kept pit
- **WHEN** a plateau with a 20 m bowl and a channel to its edge at 60 m is carved with `keep_pits` and without, and the fill depth of the kept surface is taken
- **THEN** the kept pit stays at or under its 20 m while the filling carve raises it to over 59 m, the depth of the pit is its spill level minus its floor to 1 m, no outlet holds a depth, no depth is under 0.5 m, the same input gives the same bits, and an incise-only carve is the same bits with either setting

#### Scenario: Pass 2 keeps heights, not a depth
- **WHEN** one synthetic tile is run on 1 and on 8 threads
- **THEN** the final heights are the same bits on both, finite, within the base's range of the base plus the change, and the tile holds no depth plane

#### Scenario: The depth below the planet-wide level
- **WHEN** the final heights of the 16 tiles of a face with basins across tile edges and across a four-tile corner are post-passed, and compared with a global flood of the assembled face
- **THEN** every cell's code is the oracle's, and the runtime layer reads each cell as the stored change plus the depth

#### Scenario: A flat floor against the exact base
- **WHEN** a layer is baked from a smooth bowl terrain with the exact base given to the post-pass, and the basins of 20 cells or more are read as the exact base plus the stored change plus the depth
- **THEN** the cells of one connected basin hold one surface to within the code's step (0.25 m) of their median
