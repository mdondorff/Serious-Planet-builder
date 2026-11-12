## Why
The owner judges looks on pictures of unknown scale (ADR 0118 point 5, owner via architect, 2026-11-12): every owner-facing review image gets a scale bar and the pose, drawn on the `target/review` copy, never into a golden.

## What Changes
- `planet_render::annotate`: `PoseLabel` (altitude, pitch, vertical fov, centre distance), the metres per pixel, a round-length bar (1, 2 or 5 x 10^k that fits a quarter of the width), a 5 x 7 pixel font with a black outline, `annotate`. The text scale follows the height and shrinks until the label fits the width.
- `app::stills`: `centre_distance_m` (a ray march along the view axis to the ground or sea, 600 km at most) and `pose_label` of a camera; the stills (`capture stills`) and the flight frames (`capture flights`) are written annotated. The owner package (`cargo xtask package`) takes its stills and flights from these.
- Not annotated: goldens, golden-review's difference sheets (no pose), debug-view dumps that are not owner-facing.

## Impact
- Code: `render::annotate`, `app::stills`, `app::litperf`, tests. No golden, key or format moves.
