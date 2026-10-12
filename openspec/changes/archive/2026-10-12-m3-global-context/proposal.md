## Why

The spec-guardian review of m3-global-fields (CON-06, GEN-001): `global::sample(seed, d)` took a raw seed, so nothing recorded what the global field reads and nothing could key it. ADR 0026 point 7 asked for a generator wrapper with a contract test before the bake is keyed.

## What Changes

- `generators::global`: `GlobalCoarseGenerator::field(ctx, body)` builds a `GlobalCoarseField` that reads the body's seed through the `GenContext` (world seed and the body's derived seed are recorded; a missing node is an error naming it). The field exposes `seed`, `body`, `implementation_id`, `version`, `inputs_hash`, `read_records`, `sample` and `bake`. `sample` and `bake` stay pure functions of the seed.
- GEN-013 text: built per body through a recording context; reads are exactly the two seeds. Source gains CON-06.
- Tests: `generators/tests/global_contract.rs` (declared reads equal the recorded ones, generation does not read more, equals the pointwise field, a node it does not read changes nothing, one it reads changes everything, another body is another world, a missing node errors) and a cache test (`the_key_of_the_global_coarse_bake_is_sensitive_to_resolution_seed_and_body`: the stage key is deterministic and follows resolution, seed and body (it does not isolate the read section: the seed also enters through the body parameter hash)).
- ADR 0026 is not edited here (accepted in part by the architect in #76).
- Not here: the ADR 0021 point 7 contract test for fields that read cells. The global field reads none, and no cell-reading generator other than test generators exists yet; that test is due before the first cell-reading field ships (M6 authored items, or the first regional field that reads items). This slice does not close it, and the briefing's wording ("before the first field that reads cells ships") is therefore still open, not done.
