## Why

M2 gate follow-up 3 (docs/status.md, owner asked): `plan_terrain` took 17.1 ms at ground-1m (5,108 nodes, release, 1440p) and 12.9 ms at ground-200m, more than a 60 Hz frame, and the perf harness did not budget it. LOD-008 (height-aware selection) will add per-node height bounds next, so the speed-up must keep selection geometry in one per-tile structure.

## What Changes

- `frame::lod`: per-tile geometry (`TileGeom`: bounding sphere and angular radius, from one evaluation of the eight boundary directions) is computed once per visited tile and shared by the horizon test, the frustum test, the split distance and the morph (previously three to five evaluations per tile); the memo is a hash map keyed by tile. This is also the natural home for per-node height bounds.
- `balance_leaves`: hash sets (no SipHash) for leaves and for "internal" tiles (tiles with leaf descendants), so the walk up from a neighbour stops at the first internal ancestor; after the first round only the children of the splits and the finer leaves that triggered them are rechecked. Same fixed point (the unique minimal refinement).
- `generators::mesh::MeshKeyer`: the per-plan constant parts of the mesh key (string hashes of implementation id and mapping id) are computed once; `key(tile)` equals `mesh_key_field` exactly.
- Perf harness: `PLAN_CPU_BUDGET_MS = 8.0`; the terrain workload prints a verdict per view for the median CPU plan time (a full recompute, the worst case).
- Results are bit-identical: the committed selection and plan snapshots, the golden images and the determinism manifest are unchanged.
- Measured with an informal release-mode timing (20 repetitions, this machine, not a `cargo xtask perf` session): ground-1m 5,108 nodes about 6 ms (was 17 ms), ground-200m 5 ms (was 13 ms), aerial-20km 1.5 ms. The official measurement needs the owner's `cargo xtask perf --machine-ready`.
- Verified by differential tests in a scratch clone (old lod.rs against new): 160 scripted selections and 7,800 selections at 1,500 random cameras (135 million nodes, including poles, cube corners, face edges and cameras below the sphere), 300 random partitions for `balance_leaves`, and 210 terrain plans (text hashes incl. mesh keys, also with the steep field): zero differences. A randomized equality test against the reference algorithm is added to the repo; `balance_leaves` now debug-asserts its partition precondition.
- Modified: TEST-012.
