## Why
- The viewer rebuilds the keys of the erosion layer's records from the bake's shape (seed, face-res, planet-res, tile, halo) and the versions it was made with, and needs the global range of the change for the pyramid's bound; today the shape is passed by options and the range is the representable one. A reader that knows the layer from its store alone cannot drift from the command that made it.
- Owner decision 2026-11-04 (planner): the layer carries a manifest with the shape, the versions, the field source (STRM-073), the global change range and the pyramid block size.

## What Changes
- `layer_manifest::LayerManifest` (STRM-074): `new(world, shape)` (this build's versions, the world's field source, the shape with tile and halo resolved by `bake_specs`), `widen`, `to_text` / `parse`, `read(dir)`, atomic `write(dir)`, `merge_into(dir)` (shape, versions and source must be equal, else an error naming both texts; the range only widens), `check_current` (names each version that differs), `range`.
- `bake_layer` adopts `bake_specs`, writes the manifest to the layer's `tiles/` directory after each run (the range over the tiles of the run merged into what is there), and its default `--planet-res` is the production 98,304 (previews pass `--planet-res` or a `--region`).
- `erosion_layer::BLOCK_CELLS` becomes public (the manifest records it).

## Impact
- Code: `cache::layer_manifest` (new), `cache/examples/bake_layer`, `generators::erosion_layer` (one `pub`). Requirement STRM-074 added. No key, record or kernel version changes; no golden moves.
- Hand-off to crusader: `StoreLayer::open` and the viewer's key check read `LayerManifest::read(dir)` (shape for `bake_specs`, `range()`, `check_current`, `source` against the store's recorded source).
