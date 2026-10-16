## MODIFIED Requirements

### Requirement: REND-021 Exposure and tone-mapping maths
`frame::look` SHALL hold, in f64 with `libm`, the formulas the lit path's shaders implement: the exposure scale `1 / (1.2 x 2^EV100)`, the pixel EV `log2(8 L)`, the pre-exposure factor from scene units (one unit is the look's `units.solar_illuminance_lux`, 126,600 cd/m2 at the Earth default, LOOK-003), the 64-bin histogram over the look's axis (EV -10 to +20 at the default; pixels without light not counted, out-of-range pixels in the end bins), the mean EV of the band between the look's fractions (40 % and 90 % at the default) of the counted pixels, the target clamped to the EV100 range (held when nothing is counted), the adaptation step (linear in EV, at most `speed_up x dt` up and `speed_down x dt` down, snapping on a reset), the ACES filmic fit (Narkowicz 2015), the sRGB transfer function and the 8-bit tone-mapped pixel. The look's values come from the look file (LOOK-001) and the functions that take none are the Earth default. Class: presentation data (ADR 0035); the pre-exposure factor is decided in f64 and rounded once to f32 for the shader.

Verify: A
Status: active
Source: ADR 0038 points 1-4, ADR 0048 points 2-4, ADR 0042 point 6, ADR 0035, CON-03, CON-14

#### Scenario: Known answers
- **WHEN** the units, bins, band means, clamps, adaptation steps, ACES and sRGB are evaluated on hand-worked inputs
- **THEN** each equals its worked answer (EV 0 exposure 1/1.2, L = 1 is EV 3, ACES(1) = 2.54/3.16, sRGB(0.5) = 0.735357)

#### Scenario: Adaptation
- **WHEN** the EV is 10, the target 16 and dt 0.5 s with the default speeds, and the target 2 for the other direction
- **THEN** the EV becomes 11.5 and 9.5, a close target is reached without overshoot, no time means no change and a reset snaps

### Requirement: REND-022 Exposure histogram and adaptation on the GPU
The exposure histogram pass SHALL count the scene target's pixels into the 64 bins of REND-021 and the adaptation pass SHALL compute the band mean, the clamped target and the adapted EV from the histogram, with `dt`, the speeds, the clamps, the histogram axis and band, the mode and the pre-exposure taken from the plan (no wall clock, no readback; the axis and band are the look's, LOOK-006). Class: presentation data. Bound: the GPU histogram equals `look::histogram_of` bin for bin on pixels away from bin edges (on a terrain frame at most 0.5 % of the counts may sit in a neighbouring bin, the total is exact); the adapted and target EV equal the CPU step within 1e-4 EV; the same plan sequence gives the same exposure bit for bit; fixed exposure ignores the scene and the history. The adaptation sums bin counts in f32, exact below 2^24 counted pixels (4K is 8.3 million); larger internal resolutions need an integer running sum first. A plan with non-finite or negative time, a reversed range, negative speeds or a non-unit sun is refused with the value named (REND-024).

Verify: B
Status: active
Source: ADR 0038 points 2 and 4, ADR 0048 point 4, ADR 0042 point 6, CON-04

#### Scenario: Histogram against the reference
- **WHEN** an image of pixels at bin centres, black pixels and out-of-range pixels is written to the scene target and the adaptive frame runs
- **THEN** every bin equals the CPU reference and a second frame gives the same counts

#### Scenario: Adaptation sequence
- **WHEN** scenes of EV 8, 14, 14, 3, 3, 25, 25 and black follow each other with different dt
- **THEN** the GPU EV and target match the CPU chain frame by frame and never pass the range maximum

#### Scenario: Fixed exposure
- **WHEN** a fixed-exposure frame runs on a dark and on a bright scene
- **THEN** the EV and the target are the fixed EV and the factor is 1

### Requirement: REND-024 Lit plan
The `FramePlan` (`TerrainPlan`) SHALL carry the lit path's decisions in an optional `LitPlan`: the exposure mode (fixed with its EV100, or adaptive with its range, speeds and the CPU-chosen pre-exposure EV100), `dt` in seconds from the injected clock, the sun direction, a reset flag, the tone-map mode, and the look's scene-unit scale and histogram axis and band (LOOK-003). Its text form SHALL be exact (floats as bit patterns) and round-trip, a plan without it SHALL keep its text and hash, and a lit plan made with the Earth-default look SHALL keep the text it had before the look file existed (the look's tokens are written only when they differ). A plan with it SHALL be refused by a renderer that has not enabled the lit path, with an error naming `enable_lit`.

Verify: A
Status: active
Source: ADR 0042 point 3, ADR 0048 point 3, CON-03, CON-04, CON-19

#### Scenario: Round trip
- **WHEN** a fixed and an adaptive lit plan are written to text and read back
- **THEN** they are equal

#### Scenario: Debug path unchanged
- **WHEN** a plan has no lit part
- **THEN** its text has no `lit` line and equals the text it had before, and a lit plan has exactly one more line

#### Scenario: Not enabled
- **WHEN** a lit plan is rendered by a renderer without the lit path
- **THEN** the render fails with an error that names `enable_lit`
