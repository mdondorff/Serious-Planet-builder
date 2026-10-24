## ADDED Requirements

### Requirement: REND-038 The horizon shadow term
The lit terrain fragment stage SHALL multiply the direct sun term by a shadow factor from the horizon (GEN-046): the sun's elevation against the horizon of the fragment's point at the sun's azimuth, with a soft edge from -0.35 to +0.9 degrees around the horizon, taking the horizon textures of the fragment's chunk and the coarse horizon. The position used to sample the maps SHALL be tile-local or camera-relative f32 (CON-11). With no horizon resident, or with shadows off, the term SHALL be 1 and the output of every existing view SHALL be unchanged. The shadow term and the material map together SHALL take at most 1.5 ms of terrain-pass GPU time at 1440p (M8 criterion 5; measured in the architect's window, never in a test).

Verify: C
Status: planned
Source: opus-planner (2026-10-24, M8-5), SPIKE-32 step 2, CON-11, CON-18

#### Scenario: Shader against the reference
- **WHEN** the shader's shadow factor is read back at sampled texels and sun directions
- **THEN** it equals the CPU function of GEN-046 within the stated tolerance, and with shadows off the frame equals the frame of a build without the term

#### Scenario: The view
- **WHEN** the horizon view is drawn at the fixed poses at sunset and midday
- **THEN** the horizon elevation and the shadow mask read as the ridges that cast them, and the stills match their goldens
