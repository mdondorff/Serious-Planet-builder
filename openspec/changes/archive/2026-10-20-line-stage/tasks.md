## Tasks
(The item file schema 2 and the line kinds of the world definition are the change `line-items`.)
1. [ ] generators: polyline distance, profiles, combination in `apply_stamps`, `delta_range` for lines (GEN-028); tests first (monotone profiles, order independence, bounds enclose the sampled deltas, pole and antimeridian).
2. [ ] contract: output unchanged beyond reach and within `max_delta_m`; pass-through away from the line; key tests (`stamp_keys`).
3. [ ] app: stage selection unchanged; the editor's preview skips line kinds (done in `line-items`).
4. [ ] tier C: a ridge and a channel over the test world, images in `target/review/lines/`; no golden.
5. [ ] numerics-reviewer, spec-guardian, verifier; archive.
