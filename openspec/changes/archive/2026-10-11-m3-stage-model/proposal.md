## Why

M3 plan, CON-08, GEN-007 and ADR 0021 point 6: the field pipeline needs its staging model before any field is written, so that a stage cannot read a later stage and a downstream key follows its inputs.

## What Changes

- `planet_generators::stage`: `Stage` (global-coarse, regional, local), `FieldSpec` (id, stage, reads, version, implementation id, halo fraction), `FieldGraph::new` (refuses same-or-later-stage reads, unknown or duplicate fields and bad halos, naming fields and stages), `order`, `consumers_of`, `output_key` and `all_keys` (the key of an output chains the keys of its inputs; a missing input key panics).
- Modified: GEN-007 (now active). Tests: six in `generators/tests/stages.rs`.
- Not here: any field. The global-coarse bake, the regional and local fields and the generator that composes them follow as separate changes under a new implementation id, so existing goldens and snapshots stay as they are.
