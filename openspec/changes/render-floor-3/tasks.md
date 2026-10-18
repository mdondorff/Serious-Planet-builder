## 1. Selection and plan (frame)
- [ ] 1.1 Failing tier-A tests: LOD-011 scenarios (ten views at floor 3 and 0, a 100,000 km camera draws at most 384 nodes, floor leaves have morph 0 and the interval is accepted, refused above the floor), LOD-004 floor leaf.
- [ ] 1.2 `frame::lod` `select_impl`: `level < floor` splits; floor leaves root-like; `frame::terrain` interval check exemption; `LodParams::min_level`, repro bundle field, `--floor-level`.

## 2. Residency and streaming
- [ ] 2.1 `streaming::managed` and the residency set: roots are the floor tiles, R and ancestors stop at the floor, eviction never touches a root, start-up load of 384 roots, cover from the floor tiles; the tests that assume level-0 roots take the floor as a parameter (floor 0 keeps the old tests).
- [ ] 2.2 Cold start and the pre-fill: levels 3..=4 (1,920 tiles), `generate --prefill-levels`, pack without vertex-form records; STRM-021 scenarios; blank-node count at a cold orbit start.

## 3. Pictures
- [ ] 3.1 Orbit views (level, relief, normals) on the software adapter: no hole or crack, no morph at the floor; the orbit goldens listed under `Expected golden changes:` and sent through `golden-review` (the orbit change escalates to the owner's `/bless`).

## 4. Close
- [ ] 4.1 `cargo xtask test`, `test gpu`, spec-lint; flip the delta requirements to `Status: active` at archive; log fragment in `docs/log.d/`.
- [ ] 4.2 Modify in this delta: TEST-018/019/021/024/008 (levels 0-L present, "126 compact tiles of levels 0-2", the dive gate's N, level-0 ancestors, `min_level` in the repro bundle), LOD-009 (`min_level` restarts plan reuse), STRM-006 ("six roots"), the atmosphere-ocean pre-filled-tiles scenario; decide `min_level` above `max_level` (refused).
- [ ] 4.3 Run `spec-guardian` before archive; tests cite LOD-011, LOD-004, STRM-001/007/015/016/021. Expected golden changes: the two orbit goldens (tile ids, levels, geometry).
