## Why

M4 item 7a (SPIKE-05 follow-up). Over the eroded field the draw plan costs 17.8-20 ms warm at ground-1m (budget 8 ms) and seconds at altitude. The ablation (`docs/learnings.md`, 2026-10-15, `plan_ablation` example) shows where it goes: the per-node origin height `field.height(centre)` is evaluated every frame for every node (14.9 of 17.8 ms at ground-1m; at altitude each coarse-tile sample re-decodes store blocks, about 120 ms per node). The mesh key is not the cost (0.25 ms for 8,763 nodes; the eroded field does not read cells), so SPIKE-05's inference about coarse-tile key hashing was wrong.

## What Changes

- frame: `SelectionCache` also memoises each tile's origin height and mesh key (pure functions of the tile, the face mapping, the field identity and radius, and for the key the cell count). Same invalidation as the existing per-tile memo; the mesh-key memo also starts over when `cells` changes. `plan_terrain_cached` uses them. A warm plan of unchanged tiles evaluates the field zero times.
- terrain-lod spec: LOD-009.
- app: `plan_ablation` example (measurement tool, not product code), as `residency_counts` is.

Not in this change: key derivation (ADR 0018, 0021, 0023, 0026 unchanged, no ADR needed), the store's block cache (7b: a cold plan at altitude still pays the block decodes), the frustum cut (7c).
