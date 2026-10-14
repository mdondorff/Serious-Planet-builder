## Why

M3 gate condition (CON-14, GEN-003, ADR 0023 point 9): the determinism manifest that CI diffs between Windows and Linux covers tile heights, meshes and keys only. The global bake (GEN-014), the terrain field (`cpu-terrain-v1`, GEN-016), the eroded field (`cpu-erosion-v1`, GEN-017) and the persistent known answers of `cache` (ADR 0018) are not cross-OS checked.

## What Changes

- `crates/generators/src/manifest.rs`: the manifest gains `globalgrid` (a 64-cell bake hashed whole), `globalblock` (every block of the same bake as 16 cells at level 2, 96 lines), and `terrain` / `erosion` (raw `f64` bits of `height(dir)` at 12 face-coordinate points on each face: the four cube corners, the four edge midpoints, the centre, an interior point, an off-middle edge point and a point a hair inside a corner), plus `version global|terrain|erosion` lines. The bake and fields are single-threaded; the fields' bake key is a fixed stand-in (`MANIFEST_BAKE_KEY`).
- `GlobalFieldValues` computes the values once and renders the lines with an optional one-ulp perturbation per line, which is the mutation check's hook; `determinism_manifest_perturbed` exposes it for the whole manifest.
- `crates/app/src/manifest.rs`: `generate --manifest` writes `generators`' manifest followed by the persistent known answers of `cache` (`persistent <name> <hex>`: base, height, mesh, node, body, empty query, stage key, block key, node set id; plus `version cachekey|cachehash`), the same inputs and literals as `cache`'s tests. `generators` cannot depend on `cache` (CON-02), hence in `app`. This completes ADR 0023 point 9 (the known answers move into the manifest, which was written from `app`).
- GEN-003's text is extended; tier A tests: the new kinds and counts, directions at edges and corners, and a one-ulp mutation of every new entry that changes exactly that line (all 241 lines); the persistent lines equal the literals pinned in `cache`'s tests.
