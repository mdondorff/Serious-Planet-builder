## Why
Part (3) of `m8-relief` (ADR 0080 points 1, 3 and 5, crusader's lane, the bake side): the base is smooth at hill scale (2 km slope p99 at most 2.4 degrees in every ground band). Hills of 1.5 to 8 km belong in the bake, so the coupled run, the coarse horizon and the hydrology read them.

## What Changes
- **A hill band in `global::sample`** (GEN-064): a ridged multifractal of three octaves on the warped direction, wavelengths 8, 4 and 2 km on a body of Earth's radius, added to the elevation of land only (0 over the sea and at the coast, a ramp over the first 0.1 of land depth). It only lifts, so no inland cell goes below the sea.
- **The landform driver** `global::landform(seed, d)` in `[0, 1]`: the range-zone strength times the distance from the coast, a pure function of the direction that does not read the band. The band's amplitude is 30 m on the plains, up to 175 m in hills, up to 1000 m in mountains by the mountain strength `landform x (0.25 + 0.75 ridge)`.
- **Bounds:** the largest amplitude is `HILL_BAND_MAX_M` (1000 m); the base and ranges reach 6200 m and the bake clamp is 9000 m, so the clamp and `max_abs_height_m` stay as they are (a test pins that).
- `GLOBAL_VERSION` 4 to 5: every bake, climate texture and mesh key moves; the pinned known answers of the crate are re-recorded.

## Impact
- Specs: generator-pipeline (GEN-064).
- Code: `generators/global.rs`; the re-pinned known answers in `generators/tests`.
- ADR 0080 point 3 is amended: the driver is the range-zone strength, not a neighbourhood range (the bake is a pointwise function of the direction, so a neighbourhood range needs a stencil per sample or a coarse lattice).
- The shared rebuild makes the new bake (about 8 minutes at 16384), the coupled run (about 5 minutes) and the coarse horizon.
- Out of scope: the erosion filter's crease amplitude following the landform (explorer's lane, reads `landform`), the targets' measurements (after the rebuild).

Expected golden changes:
- lit_orbit_ev15: the hill band lifts land by up to 1000 m, so a few limb/coast pixels of the orbit view change (4 pixels at WARP).
- lit_orbit_ev_bands: the same cause (2 pixels).
- lit_orbit_temperature_ev15: the temperature texels of land fall with the lifted heights (352 pixels, max delta 45).
- sky_orbit_ev15: the same cause as the lit orbit view (5 pixels).
- water_class_orbit: the coast of the water class moves with the lifted land (10 pixels).

Bound tests:
- lit_orbit_ev15: planet-generators::it::global::the_hill_band_only_lifts_land_stays_within_its_largest_amplitude_and_is_zero_at_the_coast
- lit_orbit_ev_bands: planet-generators::it::global::the_hill_band_only_lifts_land_stays_within_its_largest_amplitude_and_is_zero_at_the_coast
- lit_orbit_temperature_ev15: planet-generators::it::global::the_hill_band_only_lifts_land_stays_within_its_largest_amplitude_and_is_zero_at_the_coast
- sky_orbit_ev15: planet-generators::it::global::the_hill_band_only_lifts_land_stays_within_its_largest_amplitude_and_is_zero_at_the_coast
- water_class_orbit: planet-generators::it::global::the_hill_band_only_lifts_land_stays_within_its_largest_amplitude_and_is_zero_at_the_coast
