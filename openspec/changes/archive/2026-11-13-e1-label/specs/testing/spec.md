## MODIFIED Requirements

### Requirement: TEST-128 Owner-facing review images carry a scale bar and the pose
Every owner-facing review picture written by the review tooling SHALL carry a scale bar (the longest round length of 1, 2 or 5 times a power of ten that fits a quarter of the width, in metres or km) and the pose (altitude, pitch, vertical field of view, metres per pixel at the centre), drawn on the `target/review` copy, never on a golden.
- The metres per pixel are `2 d tan(fov / 2) / height_px` for the distance `d` along the axis to the ground or sea.
- A view whose axis meets no ground has the label and no bar.
- Golden-review's difference sheets carry no pose and are not annotated.
- Notes (`--note`) go above the label.

Verify: A
Status: active
Source: Owner via architect, 2026-11-12 (ADR 0118 point 5)

#### Scenario: A still
- **WHEN** a still or a flight frame is written for the owner
- **THEN** it shows the bar and the label, and the golden of the same view is unchanged
