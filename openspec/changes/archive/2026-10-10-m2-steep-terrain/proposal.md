## Why

M2 gate (owner asked): no cracks, morph continuity and jitter were proven only on the smooth reference generator (5 octaves, about 600 km finest wavelength, 4 km amplitude; SPIKE-02 caveat).

## What Changes

- `generators::height::HeightField` with `Reference(seed)`; `generators::stress::Steep` (test-only ridged field: 16 octaves, gain 0.7, about -5.5 to +6 km, 20 m slopes up to 72 degrees).
- Field-generic paths: `mesh::tile_mesh_field`, `mesh_key_field` (salted for non-reference fields), `surface_normal_field`, `frame::plan_terrain_field`, `scripted_views_field`. The existing functions forward with `Reference` and stay bit-identical (known answers, plan snapshots unchanged).
- Tests on the steep field: steepness, reference equality and key salting, 1 mm vertex precision, cross-edge border positions and normals, morphed borders, morph continuity in ten views, 0.25 px jitter at 1 m (tier A); ten views without holes on the GPU (tier C).
- Added: TEST-015.

## Result

All invariants hold on steep terrain (jitter 0.00018 px at 5.5 km). **Finding (known M3 item, now measured):** node selection ignores terrain height, so at steep ground-1m (terrain 687.4 m, camera 688.4 m) the tile under the camera is level 16 with 9.5 m cells and its nearest vertex is at 689.5 m: the camera is below the rendered surface and the frame shows one tile. The reference ground-1m gets level 19 (1.2 m cells). Height-aware selection (per-node height bounds) is the first M3 item; no parameter was tuned here.

## Impact

`crates/generators`, `crates/frame`, new tests in `generators`, `frame`, `render`. No output, golden or snapshot changes.
