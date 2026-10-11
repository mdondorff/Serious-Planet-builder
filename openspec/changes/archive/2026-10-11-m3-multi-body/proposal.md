## Why

M3 queue item 4 (ADR 0001, 0003, 0006, 0007): `BodyId` existed only inside the read set (the derived seed path), so a key did not name its body, and the mesh key covered the radius but not the body. Moons and other planets need keys that cannot collide with the planet's, and ADR 0018 (proposed) puts the body id and a body parameter hash into every persistent key.

## What Changes

- `HeightField::body()` (default `BodyId::PLANET`): a field built directly from a seed stands for the single planet; generator-built fields (`Bound`) carry the body they were built for.
- Height keys add `KEY_FORMAT_VERSION` (2), the body id and the reference surface id. Mesh keys replace the bare radius bits with `body_params_hash(body, seed, radius, surface)`; `MeshKeyer` follows.
- `world-def`: `body_params_hash` and `SPHERE_SURFACE_ID`. Body radius and per-body seeds were already definition inputs (`body.N.radius_m`, `derive_body_seed`).
- Plan snapshots re-recorded (only plan hashes and mesh keys changed; geometry identical, checked); the determinism manifest carries `version key` and per-tile `heightkey`/`meshkey` lines so CI compares the new composition between Windows and Linux.
- Added: GEN-012. Modified: GEN-010 (wording).
- Not in this change: a body in `TerrainRequest` and plans for several bodies at once (a plan takes its field from the generator and so already names its body); the 128-bit persistent keys (ADR 0018, proposed).
