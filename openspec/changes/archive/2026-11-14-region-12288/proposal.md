## Why
Inbox #100 step 1: the prototype's solver, G-PART and H1 were measured at 3,072 cells a face edge (3.26 km). The round and H1 are specified on the erosion grid (12,288 cells, 814 m). This change measures the prototype region there: the gates, the real channel density and the peak memory, and bounds the landmass from the windows.

## What Changes
- `examples/region_scale`: face 1's prototype region at 12,288 cells with a margin on every side (the window's border an outlet), `gates` (G-ROUND, G-DET, G-PART, H1 with G-CON and the conservation, the channel density) and `mem` (the peak working set after the set-up, the 4 rounds and the H1 pass).
- `design.md`: the measurements for margins 200 to 4,000 cells and the bound for the whole landmass.
- `docs/design/terrain-target.md` section 6.6: the measured memory at the erosion grid, and the note that the 3,072-cell channel share is a grid effect.

## Impact
- Code: an example only. No library change; no requirement (the gates are the tier-A tests of GEN-141 to GEN-145, repeated at scale).
- Not in this change: the landmass at 12,288 resident (the cube grid's arrays over 906e6 cells do not fit); a compact landmass layout; the edit cases (step 3).
