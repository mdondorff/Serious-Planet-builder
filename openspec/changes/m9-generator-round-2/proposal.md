## Why
Round 2 of the M9 generator (inbox #41, 2026-11-02): the continents. SPIKE-92 closed with H2r3 as the continent base (S1-S3, S9-S11 on seeds 1-5). Today's continent value (a warped fBm) gives 1 to 4 masses and 30-100 % in the largest one. The belts and classes of round 1b were placed on random plates, independent of the new coasts.

## What Changes
- **Continent stage (planet-generators `continents`, new):** H2r3 as CPU-canonical code (libm, integer hashing, deterministic layout retry): ranked cratons as chains of elongated lobes, a warped fBm damped in the deep ocean, an island term on a smaller domain warp (0.3, from 0.6: the striations of SPIKE-92's island fields), a per-seed offset to 29 % land. `GLOBAL_VERSION` 6 (GEN-104 to GEN-106).
- **Class source (`classes`):** the plate draw that puts the most land next to a convergent margin is kept (up to 16 draws); the coast distances and the coastal arcs follow the smooth continent value (the full value's gradient is not defined at the lattice's scale); the coast gate ramps from 50 to 300 km and over a land depth of 0.25; the mountains' membership is `smoothstep(0, 0.7, max(core, 0.8 flank))` and the high mountains' elevation range 1000-5500 m. `CLASS_SOURCE_VERSION` 4.
- **Preview:** S9 (compactness), S10 (elongation), S11 (islands) and the polar rule beside S1-S8; the default seed is 14, the owner's reference world (picked from the engine gallery) (the seed stays a parameter).
- **Spec text:** GEN-094, GEN-095, GEN-096 (planned, in `m9-bake`) follow; GEN-104 to GEN-106 and TEST-087, TEST-088 added.
- Not in this change: wiring into the bake, the plateau share and the step limit of `k` (reported again).

## Capabilities
### New Capabilities
- `generator-pipeline`: GEN-104 to GEN-106
- `testing`: TEST-087, TEST-088

### Modified Capabilities
- `generator-pipeline`: GEN-094 to GEN-096 (planned, in `m9-bake`)

Expected golden changes:
- lit_orbit_ev15, lit_orbit_ev_bands, lit_orbit_precipitation_ev15, lit_orbit_temperature_ev15, sky_orbit_ev15, water_class_orbit: once, the test field's climate now follows its own heights (GEN-021, owner 2026-11-02) instead of the generator's world: biome colours change where the reference heights and the old generator's elevation differed; the geometry is the reference field's and does not move; a few hundred pixels
- lit_aerial20km_ev15, lit_aerial20km_albedo_ev15, sky_aerial20km_ev15, water_class_coast_aerial2km: the same climate change under the aerial camera (a whole-ground tint of at most 18 of 255)
- lit_ground1m_ev15, lit_ground_eye0p5m_natural_ev14p5, lit_ground_eye1p8m_natural_ev14p5, sky_ground1m_ev15, sky_ground1m_sunset_ev11: the same, at the ground (at most 14 of 255)
After this change a change of the generator moves none of these goldens (they read the reference field's heights and climate only).
