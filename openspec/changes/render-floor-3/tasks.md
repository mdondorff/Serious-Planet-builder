## 1. Selection and plan (frame)
- [ ] 1.1 Failing tier-A tests: LOD-011 scenarios (ten views at floor 3 and 0, a 100,000 km camera draws at most 384 nodes, floor leaves have morph 0 and the interval is accepted, refused above the floor), LOD-004 floor leaf.
- [ ] 1.2 `frame::lod` `select_impl`: `level < floor` splits; floor leaves root-like; `frame::terrain` interval check exemption; `LodParams::min_level`, repro bundle field, `--floor-level`.

## 2. Residency and streaming
- [ ] 2.1 `streaming::managed` and the residency set: roots are the floor tiles, R and ancestors stop at the floor, eviction never touches a root, start-up load of 384 roots, cover from the floor tiles; the tests that assume level-0 roots take the floor as a parameter (floor 0 keeps the old tests).
- [ ] 2.2 Cold start and the pre-fill: levels 3..=4 (1,920 tiles), `generate --prefill-levels` and `--prefill-dir --floor-level`, an explicit `--floor-level` above `max_level` refused (the effective floor is min(floor, max_level)), pack without vertex-form records; STRM-021 scenarios; blank-node count at a cold orbit start.

## 3. Pictures
- [ ] 3.1 Orbit views (level, relief, normals) on the software adapter: no hole or crack, no morph at the floor; the orbit goldens that move listed under `Expected golden changes:` and sent through `golden-review` (the orbit change escalates to the owner's `/bless`).

## 4. Close
- [ ] 4.1 At archive, after `compact-slot-v2`: add to STRM-006 the clause that the start-up load of the roots is exempt from the per-frame cap (STRM-001); `cargo xtask test`, `test gpu`, spec-lint; flip the delta requirements to `Status: active` at archive; log fragment in `docs/log.d/`.
- [ ] 4.2 Implement the testing, LOD-009, STRM-022 and ATMO-020 deltas of this change (pre-fill levels F..L, `min_level` in the repro bundle, plan reuse, 384-tile store check).
- [ ] 4.3 Measure the cold start at orbit without the pre-fill once (time until the 384 floor tiles are resident, from the readiness report) and report it in docs/status.md; no budget is set.
- [ ] 4.3b Re-pin TEST-011's per-view counts for orbit-20000km (52 to 181 nodes) with the reason; the effective floor is min(floor, max_level), so scenes with a finest level 0-2 need no `--floor-level`; the viewer takes no flag, `test-render` and `generate` do.
- [ ] 4.4 Run `spec-guardian` before archive; tests cite LOD-011, LOD-004, STRM-001/007/015/016/021. Expected golden changes: the orbit goldens that move (level, relief and normals views; tile ids, levels, geometry).
