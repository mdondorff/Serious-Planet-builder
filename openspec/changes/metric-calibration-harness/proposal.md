## Why
- O8 step 1 was tuned for a day against in-zone Xdiag before a synthetic check showed the metric reads a smooth 45-degree valley as facets (0.081, against a 0.010 target; spikes #184f, 2026-11-25).
- M4's along-channel form and D8 sinuosity have the same weakness: they read the lattice direction of a straight, smooth valley.
- After the owner's caution and an external process review (2026-11-25), the lead ruled that no geometric gate metric is registered without a calibration on synthetic valleys and on S0.

## What changes
- **TEST-155:** a tier-A harness, `planet_cache::metric_calibration`.
  - Synthetic valleys at 0, 22.5, 45, 67.5 and 135 degrees: V and round profiles; a smooth floor and three defects (a 4-connected staircase floor, an 8-connected staircase floor, a lattice-locked sawtooth).
  - Three backgrounds: none, a fixed-seed fBm, and a 256 x 256 crop of R1h's S0 (fixture, 256 KiB, hash in the spec).
  - Each metric is run on every case and judged against its declared band:
    - smooth cases inside the band at every angle and background;
    - every defect case outside it.
  - The harness writes an angle x floor table per metric to `target/review/metric-calibration/`.
- **BUILD-069:** a geometric gate column is registered only with a calibration entry that passes TEST-155. Each metric's band lives in its own requirement's Detail.
- **The first entries, as a report only** (BUILD-069 binds new registrations):
  - the gate's corner links (added to the reference), hump cuts and spectrum slope change;
  - in-zone Xdiag, M4 along the channel, D8 sinuosity and the D8 straight-run share.
  - A column that fails is reported and becomes an owner item (O14's evidence). Nothing existing is removed or changed (CON-31).

## Expected golden changes:
- none: a tier-A test and a library module; no render path

## Impact
- Spec deltas:
  - TEST-155 (testing);
  - BUILD-069 (build-tooling).
- Code:
  - `crates/cache/src/metric_calibration.rs` (new: surfaces, the `GeometricMetric` trait, the judge, the report, the entries);
  - `crates/cache/src/lib.rs` (the module);
  - `crates/cache/tests/metric_calibration.rs` and its entry in `tests/it.rs`;
  - the fixture `crates/cache/tests/support/s0-r1h-river-256.f32`.
- No change to the gate's columns, bars or outputs.
