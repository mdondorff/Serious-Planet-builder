## Why
- The precipitation of the global field is latitude bands times noise: no range has a wet and a dry flank, no rain shadow, no aridity index (SPIKE-102 finding; `global.rs climate_from_elevation`). ADR 0104 (owner, 2026-11-05, "Full d3") decides a relief-aware climate: a wind field and a linear orographic precipitation model run for two seasons, with aridity and seasonality as outputs. Arid class rules (GEN-128) and runoff in the carve read it later.
- Taken from explorer's draft (#1177, GEN-117; its proposal dropped from that PR per planner inbox #76).

## What Changes
- GEN-117: an analytic wind field (prevailing direction by latitude band, noise warp, a steadiness value) and an orographic precipitation field for two seasons, CPU-canonical (`libm`, fixed order, f64, same bits for any thread count), built in memory on a coarse lattice per cube face from the class source's base elevation, never stored, keyed by `OROGRAPHY_VERSION`: annual precipitation (the two seasons summed), aridity `AI = P / PET` (`PET = 300 + 45 T`, SPIKE-81) and a seasonality value.
- The global field bake reads the annual precipitation from it in place of today's bands times noise for class-source worlds (`class_sample`): `GLOBAL_VERSION` 7. Stores baked before it are refused (owner: older stores need not stay compatible).
- GEN-130: `generate --climate-preview DIR [--seed N | --world F] [--km N] [--fine-lattice N]`: planet maps of annual precipitation with the single-wind model against the two-season one, aridity, seasonality, wind arrows, the four preview regions' material colours, the land shares by aridity class and by latitude band and the windward over lee ratios across belts (also on a finer lattice); every image labelled as an input.
- Not here: arid class rules (GEN-128, next change), runoff in the carve (explorer's lane, reads `Orography::precipitation_mm`), a rebaked preview store (explorer), cold-current coastal deserts, dunes.

## Impact
- Code: `generators` (`orography.rs`, `orography_preview.rs`, `global.rs`, `bake/inputs.rs`), `app` (the option). Requirements: GEN-117, GEN-130 added; `BakeInputs` holds the climate (built on first use); GEN-013 and GEN-021 follow the version.
- Material colours that read precipitation change planet-wide (the owner's look, decided on the preview).
- All numbers are start values for the owner's preview round.
