## Tasks
1. [ ] item file: schema 2 (`[[points]]`, `group`, `part`), loader refusals, canonical writer, hash with the points, `Item::put_into` with the points (AUTH-005); tests first (round trip, hand-written spelling, refusals, a pinned hash, schema 1 unchanged).
2. [ ] world-def: `StampKind::Ridge` / `Channel`, params `[amount_m, width_m, 0]`, points in the stamp and its hash, put refusals (WDEF-011); tests first.
3. [ ] generators: polyline distance, profiles, combination in `apply_stamps`, `delta_range` for lines (GEN-028); tests first (monotone profiles, order independence, bounds enclose the sampled deltas, pole and antimeridian).
4. [ ] contract: output unchanged beyond reach and within `max_delta_m`; pass-through away from the line; key tests (`stamp_keys`).
5. [ ] app: stage selection unchanged; the preview list skips line kinds (and counts them).
6. [ ] tier C: a ridge and a channel over the test world, images in `target/review/lines/`; no golden.
7. [ ] numerics-reviewer, spec-guardian, verifier; archive.
