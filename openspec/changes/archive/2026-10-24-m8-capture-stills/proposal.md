# Change: m8-capture-stills

## Why
- Look review 1 (M8-4) and look review 2 (M8-8) judge engine renders of landforms at fixed poses with the A-net chunk stage on (`docs/briefings/m8-plan.md`, exit criterion 1).
- No tool draws a pose over a world with its chunk stage: `capture-flights` draws fixed coastal flights over a world with no chunk stage.
- The tool serves both reviews and later regression checks (owner-approved, opus-planner, 2026-10-24).

## What changes
1. `test-render --scene capture-stills --poses FILE --out-dir DIR [--store DIR | --allow-memory-bake --global-res N] [--size WxH] [--views A,B]`: one PNG per pose and an index `stills.txt`.
2. The poses file (TOML): an anchor (the largest river of the coupled run, or latitude and longitude), per pose the eye (altitude over the ground, distance behind the anchor, heading, pitch) and the sun (local solar hours, declination), and the chunk cell size and the wait timeout.
3. Each still plans the pose, drives the chunk jobs nearest first, and plans again over the field that includes what landed, until every tile of the plan reads only ready chunks; the eye follows the ground the chunks give. A timeout fails with the pose, the missing chunk count and the first missing keys.
4. `docs/milestones/M8/look-review-1/poses.toml`: the poses of `docs/briefings/m8-plan.md` (300 m, 30 m, 1.8 m; sunset and midday).

## Tests
- Tier A: poses file parsing and refusals, the sun for a local time, the camera of a pose, the wait message.
- Tier C (software adapter, small store): two poses drawn with their chunks landed, one pose by name, refusals.

Expected golden changes: none (the stills are review images, not goldens).
