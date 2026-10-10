## Why

M2 gate (owner asked): no cracks, morph continuity and jitter were proven only on the smooth reference generator (5 octaves, about 600 km finest wavelength, 4 km amplitude; SPIKE-02 caveat).

## What Changes

- `generators::height::HeightField` (seed, implementation id, version, height bound) with `Reference(seed)`; `generators::stress::Steep` behind the `stress` feature, enabled only by dev-dependencies (ridged field on its own lattice layer: 16 octaves, gain 0.7, `|h| <= 8 km`, about -5.5 to +6 km, 20 m slopes up to 72 degrees; `STEEP_VERSION` with a known answer).
- Field-generic paths: `height::cache_key_field` (the field's implementation id and version form the height key), `mesh::tile_mesh_field`, `mesh_key_field` (chains the field's height key), `surface_normal_field`, `frame::plan_terrain_field` (plan errors for a seed mismatch or heights beyond `max_height_m`), `scripted_views_field`. The existing functions forward with `Reference` and stay bit-identical (known answers, plan snapshots unchanged).
- Tests on the steep field: steepness, reference equality and key salting, 1 mm vertex precision, cross-edge border positions and normals, morphed borders, morph continuity in ten views, 0.25 px jitter at 1 m on the leaf selection picks (tier A); ten views without holes on the GPU (tier C).
- Added: TEST-015, LOD-008 (planned: height-aware selection, the M3 target for the finding below).

## Result

All invariants hold on steep terrain (jitter 0.018 px at 6.4 km on the selected level-14 leaf). **Finding (known M3 item, now measured):** node selection ignores terrain height, so at steep ground-1m (terrain 687.4 m, camera 688.4 m) the tile under the camera is level 16 (9.5 m cells), drawn fully morphed to the level-15 grid (19 m) because morph is also measured height-free, and its nearest vertex is at 689.5 m: the camera is below the rendered surface and the frame shows one tile. The reference ground-1m gets level 19 (1.2 m cells). Height-aware selection (per-node height bounds) is the first M3 item; no parameter was tuned here.

## Impact

`crates/generators` (feature `stress`), `crates/frame` (`PlanError::HeightField`), new tests in `generators`, `frame`, `render`. No reference output, golden or snapshot changes; the stress field is a new, versioned, test-only artefact type.

Not covered: the no-holes oracle cannot see a camera below the drawn surface (steep ground-1m passes it while wrong); LOD-008 adds that check. Repro bundles do not record a height field yet (with the M3 `GenContext`).
