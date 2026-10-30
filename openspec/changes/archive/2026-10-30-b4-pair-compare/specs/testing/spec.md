## ADDED Requirements

### Requirement: TEST-077 The preview pair comparison
`planet_testkit::pair::compare_pair` SHALL take the stale frame, the preview frame and the exact frame of one pose and a grey threshold, and SHALL report the pixels the stale frame has more than the threshold from the exact frame, the pixels the preview frame has, the mean and largest channel difference of the preview and the bounding box of its differing pixels. The verdict SHALL be Within when the preview leaves at most a twentieth of the changed pixels, Over otherwise, and TooSmall, never a pass, when the edit changes 50 pixels or fewer. Frames of different sizes SHALL be refused.

Verify: A
Status: active
Source: opus-planner (2026-10-30), REND-033, B4-4

Detail: The bound and the 50 pixels are those of `gpu_preview.rs`. A difference is the largest of the three colour channel differences of a pixel. `first_over` gives the first view of a list, in the order given, whose pair is Over; the pair scene (TEST-076) draws the normals, relief and lit views to name the cause.

#### Scenario: At the bound
- **WHEN** an edit changes 400 pixels and the preview leaves 20, then 21
- **THEN** the verdict is Within, then Over

#### Scenario: Nothing to judge
- **WHEN** the edit changes 49 pixels
- **THEN** the verdict is TooSmall
