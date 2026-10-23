# 0077 Regional stages run per block of a fixed grid; a coupled process is one field
Status: proposed (opus-planner, 2026-10-23; for the owner's ruling on the CON-08 reading; SPIKE-33 sets option A or B and the numbers)
Date: 2026-10-23
Source: owner, 2026-10-23 (the stage model for M8; erosion and hydrology as one simulation over time); SPIKE-31, SPIKE-32, SPIKE-33; ADR 0022, 0030, 0076; constitution CON-06, CON-08, CON-14, CON-22

## Context
- SPIKE-31 and SPIKE-32: hydrology, landforms, horizon shadows and material weights need inputs over kilometres; per-tile computation at the tile's own spacing changes with the LOD.
- ADR 0022 sets the halo as a fraction of the tile edge in [0, 1] and the scope as `Global` or `Tile`; a 100 m tile cannot carry a 5 km halo.
- SPIKE-31: partition invariance is exact only when the computation grid is part of the field's definition.
- ADR 0022 option 1 already lets one field compute a chain internally; ADR 0030 point 2 plans a global stream-power stage.
- CON-08's intent is bounded input per output: it keeps the tile cost, edit invalidation, parallel work and caching bounded.

## Decision
1. **Blocks.** A regional field is computed per block of a fixed grid (whole coarse cells of the global bake); the block grid is part of the field's definition and its key; tiles of any level sample the blocks.
2. **Halo in metres.** A block field states its halo in metres in its spec, with a stated maximum; ADR 0022's halo fraction stays for tile-scoped fields.
3. **Option A (preferred): a coupled process is one field.** Erosion and drainage run as one simulation over time steps inside one field (ADR 0022 option 1), with several output channels (heights, drainage area, sediment); its halo is at least the step count times the reach per step, stated in its spec.
4. **Option B (fallback): an ordered chain of block fields**, each reading only earlier ones, with the sum of their halos under a stated cap.
5. **Two scales.** The same process runs planet-wide at the global-coarse stage (ADR 0030 point 2); the block field starts from its state and takes its border inflow.
6. **CON-08 reading.** "Regional (per tile with a halo)" reads as "per block of a fixed grid with a bounded halo"; the owner rules whether this is a reading or a one-sentence amendment.
7. **Streaming and edits.** A visible tile whose block is not ready draws from the coarse field (CON-22); an edit invalidates the blocks within its reach plus the halo.

## Consequences
- `Scope` gains `Block`; the stage key covers the block grid; changing the grid is a new field version.
- Partition invariance holds exactly for tile requests; seams at block borders are bounded by the halo (SPIKE-31: a river's size steps at some borders).
- Determinism: a block's run has a fixed cell order and `libm` only (CON-14); droplet erosion needs per-block seeding to meet it.
- Open until SPIKE-33: A against B, the step cap and halo, the block size, the offline and runtime cost, the landform method.

## Licences of adopted code or techniques
None yet. The implicit stream-power solver (Braun and Willett 2013) is a published method; any adopted implementation is recorded here.
