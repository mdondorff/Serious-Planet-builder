## Why

M3 task 3: persistent keys for staged outputs before any stage output is stored (ADR 0022 and ADR 0023 point 10: the 64-bit stage keys of `generators::stage` are in-memory only). ADR 0026 point 6.

## What Changes

- `planet_cache::stage_key`: `encode_stage_key` and `stage_key`, a 128-bit key over a canonical encoding with its own format tag (`STAGE_KEY_FORMAT_TAG` 1) and BLAKE3 context `planet-builder stage key v1`. The 128-bit keys of the inputs chain; reads are entries as in the height key. Missing, extra and unknown inputs and a wrong scope panic.
- `generators::stage`: `Stage::tag` and `scope_fits` are public (the persistent key reuses the stage tag and the scope rule; no behaviour change).
- STRM-011 added (tier A). Tests (`cache/tests/stage_keys.rs`): purity and order independence, exact re-keying (version of the global or regional field, halo, resolution, face mapping, body, parameters, reads), scope and tile, panics, a hand-written byte layout and known answers of a three-stage example.
- Not here: block scopes and several keys per input field (a stored bake is blocks, ADR 0026 points 2, 5, 6: a new format tag, made with the tiled bake before anything is stored), storing stage outputs (the tiled bake and its record kind, next), the `GenContext` wrapper for the global field and the ADR 0021 point 7 contract test, a persistent form of the in-memory `FieldGraph::output_key` (the in-memory 64-bit keys stay for maps).
