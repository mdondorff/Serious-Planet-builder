# 0077 Regional stages run per block of a fixed grid; the global coupled run fixes the river network
Status: proposed (opus-planner, 2026-10-23, revised after SPIKE-33 and SPIKE-34; for the owner's acceptance and the ruling on the CON-08 reading)
Date: 2026-10-23
Source: owner, 2026-10-23 (the stage model for M8; erosion and hydrology as one simulation over time); SPIKE-31 to 34; ADR 0022, 0030, 0076; constitution CON-06, CON-08, CON-14, CON-22

## Context
- SPIKE-31 and SPIKE-32: hydrology, landforms, horizon shadows and material weights need inputs over kilometres; per-tile computation at the tile's own spacing changes with the LOD.
- ADR 0022 sets the halo as a fraction of the tile edge in [0, 1] and the scope as `Global` or `Tile`; a 100 m tile cannot carry a 5 km halo.
- SPIKE-33: a coupled run that re-routes on every step has no bounded light cone per block (p99 28 m against a whole-region run, river courses differ at 35.5 % of channel cells); with the river network fixed by the global run, neighbouring blocks agree exactly and cost 7 times less.
- SPIKE-34: a block field fits streaming and the plan under four conditions (point 5); without them the field pops by the process's amplitude.
- CON-08's intent is bounded input per output: it keeps the tile cost, edit invalidation, parallel work and caching bounded.

## Decision
1. **Global coupled run.** Erosion and drainage run together over time steps planet-wide at the global-coarse stage (ADR 0030 point 2), once per world version; outputs heights, drainage area and receivers. Its river network is the network of every block.
2. **Blocks.** A regional field is computed per block of a fixed grid (whole coarse cells); the grid is part of the field's definition and key; changing it is a new field version.
3. **Fine steps along the frozen network.** A block runs a capped number of steps of incision along the global network plus local hillslope processes, with no re-routing, on one fixed-resolution raster.
4. **Halo in metres.** A block field states its halo and step cap in its spec; ADR 0022's halo fraction stays for tile-scoped fields.
5. **Engine conditions (SPIKE-34).** Block detail applies from the level its cell size supports; the plan requests blocks, not height samples; a tile reads the block's box-filtered level of its own spacing; a block computes in seconds.
6. **CON-08 reading.** "Regional (per tile with a halo)" reads as "per block of a fixed grid with a bounded halo"; the owner rules whether this is a reading or a one-sentence amendment.
7. **Streaming and edits.** A tile whose block is not ready draws the coarse field (CON-22) and the block's change fades in; an edit invalidates the blocks within its reach plus the halo.

## Consequences
- `Scope` gains `Block`; `HeightField` gains a level-aware height; the stage key covers the block grid.
- Seams: none inside the cores (identical fields); determinism holds per block (fixed order, `libm`; CON-14).
- The fine result depends on the raster's cell size, so LOD parents come from the one raster, never from a coarser run.
- Option B of the first draft (an ordered chain of block fields) is not needed; it returns only through a new ADR.
- Open (M8 first tasks): the halo below 5 km, the breach rule (19.7 % of channel cells still run uphill), drainage not aligned to the grid axes, the pop fade, real threads.

## Licences of adopted code or techniques
None yet. The implicit stream-power solver (Braun and Willett 2013) is a published method; any adopted implementation is recorded here.
