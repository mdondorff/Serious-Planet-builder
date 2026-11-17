## Why
- SPIKE-130 (G1e) found that nested basins collide under `basin = hash(body, level, floor anchor)`: a parent's floor is its deepest child's floor. Two siblings sharing a spill and their merged parent: the parent and the deeper child got one id.
- The lead accepted the default with one change (G0 contracts reading 4, 2026-11-17): the id holds the spill anchor; an endorheic basin hashes its spill cell too; 0 only for a basin with no spill cell at all.

## What changes
- GEN-189: `basin_id(body, level, floor, spill)` with the spill cell plus one (`None` -> 0); a stored basin has no spill cell when its spill height is infinite (`spill_anchor`); the id validator checks the new id.
- TEST-140: a parent on its child's floor has its own id; no spill cell is not cell 0; a spill moved under a kept id is an `IdMismatch`.

## Impact
generators (`bake/river_graph.rs`: `basin_id`, `spill_anchor`, the id validator); tests `river_graph.rs`, `g0_graph.rs`. GEN-145's H1 `hydro::basin_id` is unchanged.
