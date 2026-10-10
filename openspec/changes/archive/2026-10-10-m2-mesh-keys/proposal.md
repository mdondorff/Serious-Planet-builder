## Why

ADR 0006 review (queued in docs/status.md, owner asked): GPU vertex buffers were keyed by tile id alone, so a changed mesh (other seed, radius or mesh construction) kept a stale buffer; meshes had no key, version or known answers, which is why the seam-normal fix (#15) changed mesh output without any version bump; the generator's known-answer table was not tied to its version.

## What Changes

- `generators::mesh`: `MESH_VERSION` (2: first bump, for #15), `mesh_key` (height key, cells, radius bits, skirt fraction bits, version), `TileMesh::key`, `TileMesh::content_hash`, known-answer hashes for three meshes.
- `render`: the terrain renderer keys resident buffers by mesh key and replaces a buffer whose key differs; `tile_key`.
- `tests/pipeline.rs`: `assert_eq!(GENERATOR_VERSION, 2)` beside `KNOWN_TILE_HASHES`.
- Active: GEN-010 (new); REND-013 (scenario added).
- Not in scope: GenContext and replay `definition_hash` (item 4, M3).

## Impact

`crates/generators`, `crates/render`. No pixel changes (goldens unaffected).
