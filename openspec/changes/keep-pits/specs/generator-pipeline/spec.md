## ADDED Requirements
### Requirement: GEN-120 The bake keeps its pits and records how deep each lies below its flood
The bake's carves SHALL keep the pits of the carved surface in the layer's change and SHALL record each pit's depth, so that the runtime shows the filled look until lake water and a pit-tolerance rule exist:
- pass 1 (GEN-100): the route runs over the filled copy (the flood's receivers and area), but a cell that stands below its receiver is not raised to it; it gains its uplift only, so the donors above it carve toward its floor and the basin does not eat the carve's relief (SPIKE-102 run (b): the high mountains' 3 km relief p50 0.85 of Everest's against 0.62, the mountains' 0.94 of the Alps' against 0.86, and 0.85 and 0.94 on the filled surface too). The option is `keep_pits` (on); an incise-only carve (pass 2) never raises, so it ignores it;
- the fill depth of a face is, for each of the face's own cells, how far its carved height lies below the priority flood of the carved heights (epsilon 1 mm; the face's border cells and the cells at or below the sea level are outlets), 0 where that is under `FILL_MIN_M` (0.5 m: the flood's tilt over a plain and a dent are not a pit); it is coded `round(depth * FILL_STEPS_PER_M)` in a byte, 255 for the deeper (the codes are 1 m steps now: the step is a parameter of the channel's key, the code may change before the planet bake);
- the fill depth of a tile's core cell (pass 2) is the larger of the depth below the flood of the window's final heights (the window's border cells and the sea are outlets) and, where pass 1's fill depth at the cell is above 0, the depth below pass 1's spill level (the window's height before the local carve plus pass 1's fill depth), so a basin the window cuts (a basin crossing a tile's edge) is as deep as pass 1's flood of the face found it; a cell at or below the sea level holds 0;
- pass 2 reads pass 1's fill depth through the cell of the face that holds the direction (no cross-fade: it is a level difference of a flood, not a field to differentiate);
- the planes SHALL be the same bytes on any thread count (CON-14);
- a reader that shows the filled look SHALL add the depth back to the stored change (`TileResult::with_fill_added`); a tile without a fill plane is read as it is.
`BAKE_KERNEL_VERSION` 5 and `PASS2_KERNEL_VERSION` 5.

Verify: A
Status: planned
Source: Owner 2026-11-05 (SPIKE-102 D1: keep every pit that survives the breach, store the fill depth, the runtime adds it back); SPIKE-102 section 4.1 row 2 and section 6; GEN-100, GEN-101

#### Scenario: A kept pit
- **WHEN** a plateau with a 20 m bowl and a channel to its edge at 60 m is carved with `keep_pits` and without, and the fill depth of the kept surface is taken
- **THEN** the kept pit stays at or under its 20 m while the filling carve raises it to over 59 m, the depth of the pit is its spill level minus its floor to 1 m, no outlet holds a depth, no depth is under 0.5 m, the same input gives the same bits, and an incise-only carve is the same bits with either setting

#### Scenario: A tile's pit depth
- **WHEN** one synthetic tile is run on 1 and on 8 threads, over the sea, and on flat ground with a pass-1 depth of 30 m and without
- **THEN** the planes are equal on both thread counts, the sea holds none, the flat tile holds none without pass 1's depth and 29 m or more in every cell with it, and the change is the same bytes with and without the depth

#### Scenario: Added back
- **WHEN** a layer is baked with pits and read through the runtime layer
- **THEN** each cell reads the stored change plus the depth in metres, and the layer's pyramid holds that sum
