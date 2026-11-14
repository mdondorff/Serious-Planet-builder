## Why
- ADR 0119 (accepted 2026-11-12, owner: option A): K7's tile grid and halo are part of its definition; the result is defined over a fixed tile grid, versioned in the key; partitioning of execution stays result-free (CON-14, CON-30).
- D119 item 1: the spec text for the layout parameters, the key inputs and the behaviour at region edges and coasts; the cube-face edge rides with the face-seam work (REND-063) before any planet bake.

## What changes
- GEN-182 (generator-pipeline): the canonical layout of K7 (tile size, grid origin, alignment to the face lattice, halo) as a pure function of the face edge `n`, `k` and the profile; the parameters are inputs of K7's key; the behaviour at region edges and at coasts; the cube-face edge is stated as open and tied to REND-063.
- TEST-130 (tier B: K7 on one layout under 1, 3 and 8 threads and two schedules is bit-identical) is the next change; GEN-182 names it as its test.

## Not in this change
- Code. The layout constants are as built (`TileGrid`, `K7Params`); no value changes.
- The alternate-layout report and G-SEAM (D119 items 3 and 4).

## Impact
openspec/specs only (generator-pipeline), through this change's delta.
