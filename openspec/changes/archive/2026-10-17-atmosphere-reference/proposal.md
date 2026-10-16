## Why
M5 item 5 (ADR 0040, accepted after SPIKE-09) is large, so it is split in two changes. This first one lays the ground the GPU passes are checked against: the atmosphere's look parameters, the static tables computed on the CPU in f64, and the f64 reference of every per-frame product (sky view, aerial perspective, the orbit march), with the altitude-relative primitives at every altitude and the tier-A bounds. The second change (`atmosphere-passes`) builds the render-graph passes, the composite, the terrain lighting, the tier-B oracle against this reference, the temporal-stability scenario and the goldens.

## What Changes
- **`planet_frame::look::atmosphere`** (GPU-free, CON-02/CON-03): `AtmosphereParams` (Hillaire 2020 Earth coefficients as look data), `Atmosphere` (the parameters on the body's radius from the world definition), the transmittance table (256 x 64, optical depth) and the multiple-scattering table (32 x 32), both computed on the CPU in f64 with `libm` on any number of threads (identical bit for bit, a known-answer hash), the altitude-relative ray primitives, the ray march, the sky-view texel mapping and reference, the aerial-perspective texel and lookup, the orbit march from the entry point, the sun disc, `FrameGeometry` (the per-frame inputs decided in f64 and rounded once to f32, with the GPU word layout), and `AtmospherePlan`/`AtmosphereView` for the plan.
- **Look schema 3 (LOOK-001, LOOK-002):** the `atmosphere` table (coefficients, scale heights, ozone tent, top of the atmosphere, ground albedo, the sun's angular radius), the identity step 2 to 3, the version 3 fixture, the older fixtures kept. The body radius is world data and is not a look key.
- **The plan (LOOK-003, ATMO-009):** `LitPlan::atmosphere`, `with_atmosphere`; its tokens follow `view` in the text and only when present, so default plans keep their text and hash. `Camera::ray_frame` gives the pixel rays' frame.
- **Spec (ATMO-005 to ATMO-009):** the static tables, the geometry, the discretisation bounds of the tables and the marches, the physical reference cases, the plan.

No pass, shader, render-graph or golden change: the debug path and the lit path are untouched, every existing plan text, hash and golden is as before.

## Expected golden changes:
None.

## ADR 0037 conditions:
None (no golden in this change).
