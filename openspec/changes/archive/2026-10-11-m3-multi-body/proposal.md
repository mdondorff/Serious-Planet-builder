## Why

M3 queue item 4 (ADR 0001, 0003, 0006, 0007): `BodyId` existed only inside the read set (the derived seed path), so a key did not name its body, and the mesh key covered the radius but not the body. Moons and other planets need keys that cannot collide with the planet's, and ADR 0018 (proposed) puts the body id and a body parameter hash into every persistent key.

## What Changes

- `HeightField::body()` (default `BodyId::PLANET`): a field built directly from a seed stands for the single planet; generator-built fields (`Bound`) carry the body they were built for.
- Height keys add `KEY_FORMAT_VERSION` (2), the body id and the reference surface id. Mesh keys replace the bare radius bits with `body_params_hash(body, seed, radius, surface)`; `MeshKeyer` follows.
- `world-def`: `body_params_hash` and `SPHERE_SURFACE_ID`. Body radius and per-body seeds were already definition inputs (`body.N.radius_m`, `derive_body_seed`).
- Plan snapshots re-recorded (only plan hashes and mesh keys changed; geometry identical, checked); the determinism manifest carries `version key` and per-tile `heightkey`/`meshkey` lines so CI compares the new composition between Windows and Linux.
- `HeightField::surface_id()` (default the sphere) feeds the keys, so a field on another reference surface keys differently; the `SelectionCache` key now also covers the field's inputs hash and body; the mesh key names `KEY_FORMAT_VERSION` itself. The manifest also lists a moon built through the generator.
- Added: GEN-012. Modified: GEN-003 (manifest content), GEN-010 (wording).
- Not in this change: a body in `TerrainRequest` and plans for several bodies at once (a plan takes its field from the generator and so already names its body); the 128-bit persistent keys (ADR 0018, proposed).
- Also not here, found in review: the radius used by meshes and plans still comes from the request (`lod.radius_m`), not from `GenContext::body_radius_m`, so a moon's plan must be given the moon's radius by its caller; rotation, axial tilt and gravity (ADR 0001) are not definition inputs yet; `BodyId` is not in the frame types (`PlanetFixed` etc.); the surface id comes from the field, with `planet_world_def::SPHERE_SURFACE_ID` (`sphere`) distinct from `Sphere::id()` (`sphere:<radius>`): pick one source before authored items record it (learning 2026-10-11).
