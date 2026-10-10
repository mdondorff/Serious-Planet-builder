## Why

ADR 0009 review follow-up 6 (docs/status.md on change/adr-0009-review, owner asked): draw calls, triangles, resident tiles and upload bytes were only observable from a renderer run, so budgets could be asserted only at tier C (TEST-011 was planned).

## What Changes

- `frame`: `TerrainPlan::counts()` derives draw calls, triangles, distinct resident tiles and cold-start upload bytes from the plan alone; `CountBudget` with `REFERENCE_1440P` (6,000 draws; measured worst case 5,108 at 1 m); `PlanCounts::check` names every exceeded count with its value and budget.
- Tier A: the ten scripted views at 1440p with production-like parameters (cells 32, tau 6 px) stay within the budget; a tight budget fails with the count and the budget.
- Tier C: `gpu_terrain` and `gpu_renderer` assert that what the renderer drew and uploaded equals the plan's counts.
- Modified: TEST-011 (active for draw calls, triangles, resident tiles, upload bytes; allocated VRAM is not covered yet).

Known limits (verifier): tier C compares the renderer's own counters, which sit next to the draw and upload calls, so a call that skips its counter is invisible to it; and with all tiles the same size, the triangle, resident-tile and byte budgets only trip together with the draw budget (they become independent once cells vary per tile or tiles are shared). Tier A pins the per-tile constants (2,304 triangles, 1,221 vertices x 52 bytes at cells 32).
