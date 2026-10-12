## Why

Task 4 of M3: the first regional field turns the baked global elevation into terrain heights. ADR 0028 (proposed) decides that it is a pure function of the direction (one path `height(dir)`), which removes the second-path risk of a `height_at` method.

## What Changes

- `generators::terrain`: `TerrainField` / `TerrainGenerator` (`cpu-terrain-v1`, `TERRAIN_VERSION` 1): bake elevation at the direction (`global::read_direction`, new, sharing `blend_cell` with the integer reader) plus fBm detail with an elevation-dependent amplitude (at most 300 m). Sound, tight `height_bounds` with the face-coordinate Lipschitz argument (L = 2 over the extended region, several faces scanned near edges and corners, f32 margin).
- `HeightField::source_key` (default `None`); `cache::key::field_inputs` sets the key's `source` from it, so a bake change re-keys the tiles made from it (mesh keys still overwrite it with the height key).
- GEN-016 added. Tests: `generators/tests/terrain.rs` (bounds on random, edge-midpoint and corner caps, f32 coverage, small caps hug, one point one height over two tile levels on all faces, threads, pinned known answers, input hash), `cache/tests/keys.rs` (bake chaining), `frame/tests/terrain_field.rs` (standard flight at cells 32 / ratio 240, res 256 bake: no steepness rejection, deepest level 26).
- Not here: a stored terrain field (waits for ADRs 0027 and 0028 to be accepted), the erosion filter (task 5), land-by-continentalness consumers (GEN-015 rule; the terrain field uses elevation only).
