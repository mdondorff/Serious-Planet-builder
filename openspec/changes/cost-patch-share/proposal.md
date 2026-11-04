## Why
- Planner (2026-11-04): the D1 + D2 cost of the ground detail is measured against REND-034's 1.0 ms. The cost scenes already take a poses file with a `[look]` table (detail, patch, curves) and `--look`; the patch's own share was not separable from the material map's.

## What Changes
- `shadow-cost` logs, with the plan's patch above 0, `no_patch_ms` and `patch_ms` per pose (full minus the draw with the patch 0), and prints it.

## Impact
- Code: `app` (`stills`). REND-055 extended by one sentence (in the ground-detail-patch delta, which is not archived yet). No key, id, picture or golden moves; the budget verdict is unchanged.
