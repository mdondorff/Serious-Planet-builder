# 0062 Retire the vertex tile format from rendering
Status: accepted (owner, 2026-10-18, opus-planner session; M5 gate decision 5); point 4 amended by 0063. Amends ADR 0033 point 3 (slot layout and share table) and ADR 0054.
Date: 2026-10-18
Source: SPIKE-12 (`spikes/vertex-retirement/results/results.md`, PR #233); M5 report section 5

## Context
- Two tile formats render today: the vertex format and the compact format (the default, ADR 0054). Every per-tile attribute and every change to tile refresh is built and tested twice.
- Defects hide between the formats (the M5 `lit-formats` comparisons, the cross-format minimum memo found in #293).
- SPIKE-12 measured the cost of one format: GPU time +0.1 to 0.5 % with a wide texel, about 750 lines net removed, the coarsest levels replaced by a render floor.
- M6's edit path (SPIKE-13) changes tile refresh; built after the retirement, it is built once.

## Decision
1. The compact format is the only rendering format. The vertex format stays as the CPU reference and oracle (`tile_mesh_field`, `TileMesh`) plus a test-only reference renderer, so no image test loses its independent reference.
2. One wide 8 B texel for every tile (32-bit height, two 15-bit normals); the climate block and record live in the slot; `MESH_VERSION` 8.
3. A render floor at level 3: levels 0 to 2 are not drawn. The orbit view changes visibly (more detail).
4. The slot grows 5.9 % (9,360 B); ADR 0033's share table is re-cut with the record charged per slot.
5. Not adopted: the 5x5 base on every tile.
6. First work of M6, before the edit path, in SPIKE-12's four changes (slot v2, pool arithmetic, render floor, removal) and one perf window.

## Consequences
- Every terrain golden moves through `golden-review`; the orbit change escalates to the owner's `/bless`.
- The software-adapter smoke, which uses the vertex format today because WARP loses the device with compact tiles and the atmosphere, moves to the test-only reference renderer or another remedy named in the removal change.
- Risks carried from SPIKE-12: the reserve arithmetic, cold start at orbit without a coarse fallback (the pre-fill covers it), a camera beyond the standard orbit drawing up to 384 tiles, the wide texel's 1 mm limit at about 30 km of relief in one tile.
- Numbers live in the specs that own the slot, the pools and the floor.

## Licences of adopted code or techniques
None.
