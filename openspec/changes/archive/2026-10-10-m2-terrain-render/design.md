## Context

ADR 0001 (frames), ADR 0002 (reversed-Z, accepted), ADR 0009; report §2, §8, §15 (M2).

## Decisions

- **Everything numeric is decided in `frame`.** The terrain plan holds the f32 view-projection (computed in f64, rounded once), each node's origin as an f64 difference rounded once, its morph interval, and flat colours. The shader takes tile-local offsets and camera-relative origins only; a scan test rejects planet-scale literals and radius-like names.
- **Morph per vertex.** Each vertex carries a morph target on the parent grid (even-even: itself; edge midpoint; cell-diagonal midpoint, matching the triangulation). The vertex shader blends by the vertex's distance between `0.7 x` and `1.0 x` the parent's split distance, so all vertices of a child are fully morphed exactly when the parent stops splitting.
- **Skirts** of 8 % of the tile edge hide residual cracks between levels; they are part of the mesh and the tile-id view shows no background pixel behind them.
- **Hole criterion (LOD-005):** a pixel whose view ray hits the sphere at `R - 4100 m - sagitta(coarsest mesh)` must show terrain: terrain lies within +-4000 m, and a coarse mesh cuts corners by its sagitta (about 13 km at level 0 with the 1.3 safety factor of the test), which moves silhouettes inward legitimately.
- **Precision (REND-004):** the tier-A test evaluates the shader's f32 arithmetic on the CPU (including the morph mix) for vertices on the highest terrain found, at 1 m, in 0.1 mm steps: worst error 0.0002 px (budget 0.25). Tile origins sit at their centre terrain height, which keeps tile-local offsets and camera-relative origins small; with origins on the reference sphere the same test gave 0.22 px at 2.5 km terrain.
- **Camera height:** scripted cameras are placed above the generator's terrain height, but selection measures distance to the reference sphere, so near terrain is coarser than the ideal; per-node height bounds belong to M3 (recorded as a learning).
- **Pending goldens:** new goldens are listed in `tests/goldens/pending.txt` until blessed, so CI stays green for the blessed adapter without weakening the missing-golden rule.

## Risks

- Draw count (1,089 nodes at the 1 m view, one draw each) and 1.1 k vertices per tile are untuned; the M2 gate measures them on Rasierklinge.
- Normals and morph views are only golden-checked per adapter; GPU-vs-CPU twin (tier B) for the terrain shader is not built.

## Review follow-ups (numerics-reviewer, verifier)

- **Crack-free morph:** the shader measures the morph distance to a height-free per-vertex reference position (`ref_pos`), the same quantity node selection uses, so a fine vertex at a coarser neighbour reaches morph 1 and a coarse vertex at finer neighbours stays at 0. `LodParams::morph_interval` starts a node's morph no earlier than `split(level) + 2 r` and ends it at the parent's split distance; `LodParams::validate` rejects parameter sets for which the interval vanishes (about `viewport_h_px / tau_px > 1.9 * cells`). Tested over scripted views and two configurations by evaluating the shader formula in f64 on border vertices (`morph.rs`) and by comparing fully morphed borders with the coarser neighbour's border polyline across faces (`mesh.rs`).
- A vertex-layout bug (the new `ref_pos` attribute was not serialised) made every mesh garbage on the GPU while the earlier hole test still passed; vertex serialisation is now single-sourced (`MeshVertex::to_floats`, `STRIDE`, `OFFSETS`) and a new tier-C oracle test checks that each node's centre vertex projects to a pixel showing that node.
- A depth-order test (horizon culling off vs on gives the same image) falsifies a broken depth compare; degenerate cameras (over a pole, straight down) no longer panic; the node uniform stride uses the device alignment.
- `PLANET_GOLDEN_STRICT=1` now overrides the pending list; `cargo xtask check` fails when `tests/goldens/pending.txt` still lists a name that has a golden, so `/bless` must remove the line.
- Deferred and recorded: normals at cube-face borders differ by up to 0.02 degrees (halo samples extend the plane instead of using the neighbour's lattice); the hole criterion excludes a silhouette band for coarse meshes; the six new goldens await `/bless`.
- Second verifier round: a tier-C oracle now compares the GPU morph output with the f64 formula at projected vertices (catches a misread `ref_pos`), and `nodes_appear_where_their_vertices_project` requires each node's own colour. A morph distance that includes terrain height differs from the reference by well under the tolerance at test scale and is caught only by the (pending) morph golden; recorded as a known gap.
