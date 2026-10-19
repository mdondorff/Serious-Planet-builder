## 1. stamp-stage
- [x] 1.1 Wait for the 128-bit item id switch (sonnet-crusader); read the merged `ItemSpec`
- [ ] 1.2 Failing tests: profile known answers, C1 at the edge, zero beyond reach, bound, plateau, order
- [x] 1.3 `StampKind`, parameters in `ItemSpec` (derived content hash, refusals), `FootprintItem`/`ItemRec` kind and parameter bits, `item_hash128` and summary leaves (E1 known answers unchanged), `WorldDefinition::footprint_items_cap`; update the `ItemSpec` literals of the E1 tests (`crates/cache/tests/keys_footprint.rs`, `crates/world-def/tests/footprint.rs`, `summary.rs`); fix the comment in `footprint.rs` that assigns the item file hash to E2 (it is E4's); WDEF-010
- [ ] 1.4 Profiles in `generators` (libm, fixed order); GEN-026
- [ ] 1.5 `ItemReader` and `StampStage` (batch height, bounds, reads, delegation to the base), `HeightField::upstream` and `stage_params` with forwarding in every wrapper (`Bound`, `app/skeleton.rs`, `app/examples/plan_ablation.rs`, `frame/tests/support/origin_batch.rs`, `generators/water.rs`, the streamer's cancellable wrapper); the chained key in `cache::key::field_inputs` and output parameter tags 8 and 9; STRM-027
- [ ] 1.6 Failing then passing: keys chain and edit locality (also a two-layer stack and wrapper forwarding), determinism at 1 and 3 threads and one point at a time, contract test with a recording reader, tile alone over the eroded base in memory and stored, no empty morph interval at levels 16 to 18, `reach_m = 0` refusal
- [ ] 1.7 Stamped coast scene (tier C): the debug relief view, a recorded frame hash per adapter family, images to `target/review/stamps/`
- [ ] 1.8 numerics-reviewer, spec-guardian, verifier; `cargo xtask prepush`; PR
