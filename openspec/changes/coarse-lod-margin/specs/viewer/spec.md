## ADDED Requirements

### Requirement: VIEW-032 The coarse view is never refused by the plan it makes, and a late refusal does not end the window
`LodParams::coarse` SHALL return parameters that the plan accepts wherever it accepts the base ones at the levels whose node radius is at least 4 times `max_height_m`: the morph interval of every level keeps 5 % of its end plus a margin of 1 % by the node radius, and at those levels where the base parameters meet the plan's own start (`split(l) + 2 x extent(r (1 + H / R), H)`, `LodParams::morph_interval_worst`) the coarse ones meet it too with the same margin. `LodParams::validate` SHALL stay the node-radius rule. A plan failure whose request carried other LOD parameters than the frame that receives it SHALL be dropped and counted (`PlanPipeline::dropped_plan_errors`), not returned; a failure of the frame's own request SHALL be returned as before.

Verify: A
Status: active
Source: opus-planner (2026-10-20), SPIKE-19, VIEW-014, LOD-003, LOD-011

#### Scenario: The window's coarse view over the eroded world's bound
- **WHEN** the window's LOD at 320x200 over a field whose global height bound is 9,735 m is made coarse and planned from five orbital poses
- **THEN** the plan accepts every pose (it refused the coarse LOD of the former rule at all five)

#### Scenario: A late refusal
- **WHEN** the plan of parameters too coarse for 16 cells is asked for and the frame then asks again with the base parameters, and later asks for the coarse parameters themselves
- **THEN** the first refusal is dropped and counted while the frame draws, and the refusal of the frame's own parameters is returned
