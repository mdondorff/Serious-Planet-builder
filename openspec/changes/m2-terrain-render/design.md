## Context

ADR 0001 (frames), ADR 0002 (reversed-Z, accepted), ADR 0009; report §2, §8, §15 (M2).

## Decisions

- **Everything numeric is decided in `frame`.** The terrain plan holds the f32 view-projection (computed in f64, rounded once), each node's origin as an f64 difference rounded once, its morph interval, and flat colours. The shader takes tile-local offsets and camera-relative origins only; a scan test rejects planet-scale literals and radius-like names.
- **Morph per vertex.** Each vertex carries a morph target on the parent grid (even-even: itself; edge midpoint; cell-diagonal midpoint, matching the triangulation). The vertex shader blends by the vertex's distance between `0.7 x` and `1.0 x` the parent's split distance, so all vertices of a child are fully morphed exactly when the parent stops splitting.
- **Skirts** of 8 % of the tile edge hide residual cracks between levels; they are part of the mesh and the tile-id view shows no background pixel behind them.
- **Hole criterion (LOD-005):** a pixel whose view ray hits the sphere at `R - 4100 m - sagitta(coarsest mesh)` must show terrain: terrain lies within +-4000 m, and a coarse mesh cuts corners by its sagitta (7 km at level 0), which moves silhouettes inward legitimately.
- **Precision (REND-004):** the tier-A test evaluates the shader's f32 arithmetic on the CPU for a vertex on the far side of the planet at 1 m, in 0.1 mm steps: worst error 0.03 px (budget 0.25).
- **Camera height:** scripted cameras are placed above the generator's terrain height, but selection measures distance to the reference sphere, so near terrain is coarser than the ideal; per-node height bounds belong to M3 (recorded as a learning).
- **Pending goldens:** new goldens are listed in `tests/goldens/pending.txt` until blessed, so CI stays green for the blessed adapter without weakening the missing-golden rule.

## Risks

- Draw count (1,089 nodes at the 1 m view, one draw each) and 1.1 k vertices per tile are untuned; the M2 gate measures them on Rasierklinge.
- Normals and morph views are only golden-checked per adapter; GPU-vs-CPU twin (tier B) for the terrain shader is not built.
