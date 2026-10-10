# 0014 Terrain LOD algorithm
Status: proposed
Date: 2026-10-10
Source: background report §2, §12; SPIKE-02; constitution CON-03, CON-08

## Context
Planet terrain needs continuous detail from orbit to 1 m, no cracks, no popping, and selection that is a pure CPU function (CON-03). The report leans to CDLOD on the cube-sphere quadtree.

## Options (with trade-offs)
1. **CDLOD**: quadtree nodes of fixed grids, split by screen-space sample spacing, per-vertex geomorph to the parent grid, skirts.
2. Geometry clipmaps: constant memory, but hard with per-tile procedural generation and edits.
3. Meshlet or Nanite-like clusters: lowest CPU cost, but wgpu support is experimental.

## Decision
Option 1 with these specifics (tested on the software adapter; real-GPU and realistic-terrain review is pending):
- Split a node while `surface_distance < split_distance(level)`, where the distance is to the node's bounding sphere on the **reference sphere** (not terrain) and `split_distance` is where the sample spacing projects to `tau_px` pixels. Leaves are balanced to differ by one level; horizon and frustum culling happen inside selection.
- Morph per vertex between `max(0.7 x end, split(level) + 2 r)` and `end = split_distance(parent)`, measured to a height-free reference position; `LodParams::validate` rejects parameter sets for which the interval vanishes (about `viewport_h_px / tau_px > 1.9 x cells`).
- Skirts (8 % of the tile edge) hide residual cracks; tile origins sit at the tile centre height.
- Meshlets are revisited after the M2 gate if draw counts cost too much.

## Consequences
- Selection ignores terrain height, so a camera low over high terrain gets coarser near tiles than ideal; per-node height bounds (M3) fix that.
- Draw count is about one per node: measured 772 nodes at the 1 m view with the small test parameters (cells 16, 96 px) and 3,003 with `LodParams::earth_1080p()` (cells 32, 1080 px); frustum culling cuts the count by 3x in aerial views but only by 1.4x to 1.9x near the ground.
- Tests: coverage, balance, morph continuity and crack-closing tests (A), scripted views with hole counts (C).

## Licences of adopted code or techniques
CDLOD: Strugar, "Continuous Distance-Dependent Level of Detail for Rendering Heightmaps", JGT 2010 (technique described in the paper; no code adopted).
