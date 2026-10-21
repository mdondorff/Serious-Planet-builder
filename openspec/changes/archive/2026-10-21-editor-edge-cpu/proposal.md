# Change: editor-edge-cpu

## Why
- Owner (2026-10-21): with the editor on, near a cube edge or corner at about 18 km the CPU frame costs over 90 ms (editor off: under 2 ms); it follows the cube's edges, not the edits.
- Cause: a height query of the stamped field (the picking field the editor samples many times a frame) reads the cell square of its point (`WorldDefinition::footprint_items_cap` → `cells::with_margin`). Inside a face the square is built from grid coordinates (0.06 to 0.3 µs); where it reaches a face edge it fell back to `with_margin_walk`, which projects every neighbour and every corner neighbour through the tangent warp: 3, 26 and 78 µs at 1, 2 and 3 rings on an edge or a corner. A stamped height query measured 2.3 µs at a face centre and 135 to 158 µs on an edge or at a corner (20 stamps nearby).

## What changes
- `TileId::neighbors8` (core): the eight neighbours of a cell across cube faces by integer unfolding of the cube (the cell's centre in half-cell units folded over the shared edge), no projection; seven at a cube corner.
- `cells::with_margin` uses it on the seam path (ring by ring over sorted vectors). `with_margin_walk` stays as the geometric reference.
- No requirement changes (WDEF-005's set is the same, bit for bit).

## Tests
- `cell_index.rs` (tier A): the margin equals the walk for every cell of levels 0 to 3 on all faces (margins 0 to 3), plus the existing level-8 edge, corner and bundle cases (margins 0 to 6); the margin at an edge and at a cube corner costs under 30 times one inside a face (it was 50 to 250 times).

Expected golden changes: none.
