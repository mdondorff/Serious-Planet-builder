## Why

M2 starts with the pure, GPU-free half of the terrain: which cube-sphere quadtree nodes are drawn for a camera, with what morph factor. It is the part of CON-03 that must be testable at tier A before any terrain pixel exists, and it decides the scale of everything after it (node counts, mesh budgets).

## What Changes

- `frame::lod`: `LodParams`, `select_nodes` (screen-space-error splitting by sample spacing, 2:1 balancing across faces, horizon culling, CDLOD morph interval), `NodeSelection::summary`.
- Tests: determinism, coverage of the visible surface, balance, morph bounds, monotonicity and continuity at merges, ten scripted-view snapshots with node-count budget, cost bound.
- LOD-001..004 become active. The platform-maths scan now covers `frame` and `streaming` too.

## Capabilities

### Modified Capabilities
- `terrain-lod`: LOD-001, LOD-002, LOD-003 (reworded for horizon culling), LOD-004.

## Impact

`crates/frame`, `crates/core/tests/no_platform_math.rs`, dev profile (`frame`, `generators` optimised in dev builds). No renderer change yet.
