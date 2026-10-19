## Tasks
1. [ ] world-def: `StampKind::Ridge` / `Channel`, three slots `[amount_m, half_length_m, heading_deg]`, file mapping (`amount_m`, `half_length_m`; heading from the anchor), refusals (WDEF-011), derived hash; tests first.
2. [ ] generators: segment distance, profiles, combination in `apply_stamps`, `delta_range` for lines (GEN-028); tests first (monotone profiles, combination order-independence, bounds enclose the sampled deltas, pole and antimeridian).
3. [ ] contract: output unchanged beyond reach and within `max_delta_m`; pass-through away from the chain; key tests (`stamp_keys`).
4. [ ] app: nothing for stage selection; the preview list skips line kinds (and counts them).
5. [ ] tier C: a ridge and a channel over the test world, images in `target/review/lines/`; no golden.
6. [ ] numerics-reviewer, spec-guardian, verifier; archive.
