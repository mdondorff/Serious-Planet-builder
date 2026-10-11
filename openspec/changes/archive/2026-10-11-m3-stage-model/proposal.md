## Why

M3 plan, CON-08, GEN-007 and ADR 0021 point 6: the field pipeline needs its staging model before any field is written, so that a stage cannot read a later stage and a downstream key follows its inputs.

## What Changes

- `planet_generators::stage`: `Stage` (global-coarse, regional, local), `FieldSpec` (id, stage, reads, version, implementation id, halo fraction), `FieldGraph::new` (refuses same-or-later-stage reads, unknown or duplicate fields, duplicate reads and bad halos, naming fields and stages; `-0.0` halo is `0.0`), `order`, `consumers_of`, `output_key` and `all_keys`. The key of an output chains the keys of its inputs and covers the stage key version (`STAGE_KEY_VERSION`, its own constant), a typed `Scope` (global or tile), the body, surface, face mapping and resolution (`OutputContext`); a missing input key or a scope of the wrong kind panics. Known answers are pinned.
- Modified: GEN-007 (now active). Tests: nine in `generators/tests/stages.rs` (every part of the key changes it on its own, propagation through the chain for version, implementation id and halo, middle and leaf bumps, scope rules, pinned known answers).
- Review findings fixed (verifier, spec-guardian, numerics-reviewer): body, surface, mapping and resolution in the key, typed scope, own key version and known answers, canonical halo, duplicate reads rejected, per-part tests. Proposed ADR 0022 records what the code decides now and the question the reviews raised about same-stage reads (hydrology chains, local composition), which needs the owner's reading of CON-08 before SPIKE-03.
- Not here: any field. The global-coarse bake, the regional and local fields and the generator that composes them follow as separate changes under a new implementation id, so existing goldens and snapshots stay as they are.
