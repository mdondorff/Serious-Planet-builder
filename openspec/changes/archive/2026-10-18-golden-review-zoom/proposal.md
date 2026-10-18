## Why
The owner finds the golden | candidate | diff triptychs (3x, goldens of 128x96 to 256x160 px) too small to judge a change (opus-planner, 2026-10-18).

## What Changes
- `golden-review` writes, for every candidate whose golden changed (every adapter, not only escalations), two images next to the report in `target/review/golden-review/<adapter>/`:
  - `<name>.png`: golden | candidate | diff, enlarged 4x to 8x with nearest-neighbour (the largest of 8, 6, 4 whose width stays within 2,400 px);
  - `<name>.crop.png`: the same three panels cropped to the bounding box of the differing pixels plus a margin, enlarged further (8x to 32x).
- `report.md` names both files per candidate and prints the bounding box of the differing pixels.
- The `golden-review` and `bless` skills show these images instead of the raw thumbnails.
- A full-size inspection render (RTX, 1280x720 or larger, old code against new) is not made by default; the skill names it as an option when a diff looks odd and the owner or a reviewer asks.
- TEST-038 added.

## Classes and bounds (ADR 0035)
No field and no shader value is added; test tooling. No golden moves.

## Out of scope
The verdicts, the driver gate, the bound tests, `--apply`, the extra-adapter apply path (separate item).
