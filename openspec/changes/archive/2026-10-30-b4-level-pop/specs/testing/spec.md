## ADDED Requirements

### Requirement: TEST-071 A descent draws no level pop
`planet_frame::popcheck::judge` SHALL take the per-frame trace of fixed probes (level, drawn height, distance to the eye) and the focal length in pixels, and SHALL measure the jump of a probe's drawn height, in pixels, at every level change. The verdict SHALL be FAIL when the p99 is over 0.5 px or the largest over 2 px, NOT EXERCISED when fewer than 10 level changes occurred, PASS otherwise. A failure SHALL name the frame, the probe, both levels and the jump in metres and pixels. A scripted descent over the morph model of `LodParams` (3000 km to 2 m, 2 % a frame) SHALL pass, and the same descent without morphing SHALL fail.

Verify: A
Status: active
Source: opus-planner (2026-10-30), owner test 2026-10-27 (level boundaries), B4-2

Detail: The pixel bounds are proposed from the morph design (LOD-008: a node is fully morphed into its parent where the parent stops splitting) and are not an owner number: the owner's metre condition (step under 0.25 m p99) is the gate's, at its sites. The drawn height of a probe is `(1 - m) * h_own + m * h_parent` with `m = clamp((d - start) / (end - start), 0, 1)` over `LodParams::morph_interval`. A sample with a NaN or infinite height, or a distance that is not positive and finite, FAILS the verdict (it never passes by being skipped); a probe is compared with its last covered frame. A later change feeds the trace from the real plan on the real store.

#### Scenario: Morphing hides the switch
- **WHEN** a descent runs through the morph model
- **THEN** at least 10 level changes occur and the verdict is PASS

#### Scenario: A hard switch
- **WHEN** the same descent draws each level on its own grid without morphing
- **THEN** the verdict is FAIL naming a frame, a probe and the levels

#### Scenario: Nothing exercised
- **WHEN** a trace has fewer than 10 level changes
- **THEN** the verdict is NOT EXERCISED, never PASS
