## ADDED Requirements

### Requirement: TEST-069 The lit path draws the same pixels frame after frame for a still camera
On the software adapter, 6 consecutive frames of the coast view at 2 km on the lit path (atmosphere, water, adaptive exposure), drawn from one plan at one water time with the exposure at its target, SHALL be pixel-identical (`Tolerance::Exact`) in three cases: the plain view, the sun ahead of the camera, and the terrain debug overlay on the tiles. A change SHALL fail the test naming the changed pixels, their box and the tiles, and SHALL write the frames and the diff image to `target/review/still-flicker/`.

Verify: C
Status: active
Source: owner live test 2026-10-22 (orbit and dark-pixel flicker, not reproduced) and the glint report, opus-planner (2026-10-29), TEST-068, VIEW-014, ATMO-017

Detail: The world is the in-memory bake with a sea. The exposure is read back, not assumed: a case whose exposure is not at its target after 60 one-second steps fails. A case starts from the exposure the case before left, so it has to adapt and stop on its own. Each case proves it can falsify: plan nodes that carry water, more than 4 drawn tiles, and a sun-ahead picture that differs from the plain one. The tile format is the compact one, the only format (the Tracked WARP device removal with compact plus atmosphere did not show in these frames); the adapter is WARP on Windows and lavapipe on Linux. The meshes are all resident: streaming stability is TEST-068. The renderer has no 3D hover highlight; the editor's hover is a UI composite after tonemap and is not covered. Water time is held; the live window's water moves with the wall clock and is not still by design.

#### Scenario: A still lit camera
- **WHEN** the exposure has reached its target and 6 frames of the same plan are drawn and read back in each of the three cases
- **THEN** every pair of consecutive frames has zero differing pixels

#### Scenario: An exposure that does not settle
- **WHEN** the exposure is not at its target after 60 one-second steps
- **THEN** the case fails naming the adapted and the target EV100
