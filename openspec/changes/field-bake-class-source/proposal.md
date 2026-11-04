## Why
- The product's field bake (`generate --global-dir`) samples the legacy `global::sample`, not the class source the erosion layer was carved on: against `BakeInputs::base` it is off by 2,064 m rms on the mountains tile, so the viewer would show another world than the layer's.
- Owner decision 2026-11-04: the field bake takes the class source's surface; its climate, ocean and coast channels derive from it; the store records the source so a viewer that reads the layer can refuse a mismatched store.

## What Changes
- `global::class_sample(seed, d, inputs)` (GEN-115): the legacy sample's continent value, the class source's `BakeInputs::base` as elevation (clamped to the legacy range), the climate formula over that elevation. The shore is the legacy one by construction (both are 0 where the land depth is 0 and the band only lifts), so the ocean and coast channels keep their shape.
- `bake_blocks_sampled` and `bake_block_sampled` take any pointwise field; the legacy entry points call them with `sample(seed, d)` (bit for bit unchanged).
- `GlobalWorld::with_class_source` (STRM-073): the class source's identity (`BakeInputs::id()` and the seed) joins the bake's read set as a query entry (kind 6; kind 5 stays retired), so the stage key and every key sourced from it (blocks, ocean, wet floor, water summary, coupled run, horizon, relief, class weights) differ from the legacy world's; `build_global` and the verify step bake `world.field`; `check_source` refuses a store baked from another source, naming both keys.
- `generate --global-dir | --verify-global --class-source` selects it.
- Opt-in: a world that never calls `with_class_source` keeps every key, pin, golden and manifest row. Replacing the legacy generator is planned for M9's close.

## Impact
- Code: `generators::global`, `cache::global_store`, `cache::key` (query kind 5), `app` (`--class-source`). Requirements GEN-115 and STRM-073 added. No golden moves. Tests: `generators/tests/global_class.rs`, `cache/tests/class_source_store.rs`, one CLI test in `app`.
- The class band is read through the class tables (`BakeInputs::base`): a later change of the band's amplitude or shape is a parameter of the class source, not a code path here.
