## 1. Slot and texel (generators, cache)
- [ ] 1.1 Failing tier-A/B tests: GEN-025 offsets and strides (cells 16, 32, 64), GEN-019 cliff tiles (T6 rows, guard off), 15-bit normals within 0.01 degrees, determinism on 1, 2, 4 threads.
- [ ] 1.2 `generators::compact`: wide texel, header, climate block in the slot, builder switch for the relief guard, CPU mirror; `MESH_VERSION` 8; known answers re-recorded.
- [ ] 1.3 `cache::meshpack`: payload version 3, refusal of versions 1 and 2, pre-fill rebuilt; STRM-021 scenarios.

## 2. Render
- [ ] 2.1 WGSL decode (`extractBits`, `hr_min + f32(q) * hr_step`), header and climate reads from the block, REND-005 scan of the new shader; the record buffer removed; tile reference unchanged.
- [ ] 2.2 One `write_buffer` per run of slots (REND-018), readback for tests; warm-up (REND-017) unchanged in behaviour.
- [ ] 2.3 Image tests: the ten views in all debug views, wide against previous 16-bit (rounding noise numbers of GEN-019), compact against vertex within the REND-016 tolerances; known-answer hashes re-recorded on WARP and the discrete GPU.

## 3. Pool arithmetic (frame, streaming)
- [ ] 3.1 Failing arithmetic test: 14,339 slots per block, 215,085 and 329,797 at cells 32 (STRM-006 scenario), strides 2,960 and 34,448 B at cells 16 and 64.
- [ ] 3.2 `frame::pool` and `streaming::budget`: the record charged per slot, slot bytes from GEN-025's stride at every cell count, reserve without per-slot items; ADR 0033 point 3's table re-cut (ADR 0062 already amends it, no new ADR).
- [ ] 3.3 Re-run the headroom pins; fill-time scenario (79 frames of 112).

## 4. Close
- [ ] 4.1 `golden-review` over all terrain, lit, atmosphere and water goldens (declared change: texel); `cargo xtask test`, `test gpu`, spec-lint; flip the delta requirements to `Status: active` at archive.
- [ ] 4.2 Modify TEST-011 ("texels and record"), the 118-tile numbers of REND-018/STRM-006 scenarios and GEN-010's MESH_VERSION history (version 8) in this delta; log fragment in `docs/log.d/`.
- [ ] 4.3 Run `numerics-reviewer` (generators, shader) and `spec-guardian` before archive; tests cite GEN-019, GEN-021, GEN-025, REND-016, REND-018, REND-027, STRM-006, STRM-021.
- Open: ADR 0062 fixes 9,360 B, which includes 192 B reserved (the 5x5 header the ADR does not adopt); without it the slot is 9,168 B (14,639 slots per block). Planner decides before 1.2.
