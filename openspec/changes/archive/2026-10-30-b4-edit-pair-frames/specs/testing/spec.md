## ADDED Requirements

### Requirement: TEST-076 The edit pair
`planet::editperf::pair_frames` SHALL draw, per debug view, the stale frame of an edit case at an altitude, the first frame after the edit as the editor makes it (the definition swapped, the edited tiles named, the REND-033 preview set), the exact frame once the refresh has settled, and a control frame after the edit is taken back, which SHALL equal the stale frame bit for bit. Each pair SHALL be judged by TEST-077 and its verdict reported, not asserted. The tier-C test over the smoke world SHALL assert the control and that the edit is seen in at least one view.

Verify: C
Status: active
Source: opus-planner (2026-10-30), owner test 2 (drag against rest), REND-033, REND-045, B4-4

Detail: The pair is a measurement of the preview, not a gate: a pair over the bound is a finding with its pictures in `target/review/edit-pair/`. The real-store scene with the lit frame, the horizon off and on and `pair.md` extends this requirement.

#### Scenario: A trustworthy pair
- **WHEN** a 50 m hill is edited at 200 m on the smoke world
- **THEN** the control equals the stale frame in every view and the edit changes more than 50 pixels in at least one

#### Scenario: A verdict over the bound
- **WHEN** the preview leaves more than a twentieth of the changed pixels in a view
- **THEN** the test still passes and the verdict is printed
