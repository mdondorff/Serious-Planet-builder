## 1. Relief view

- [x] 1.1 `TerrainView::Relief` (appended), shader branch with hillshade, tint, metric grid and fine contours (REND-006)
- [x] 1.2 Colour-rule arm in the every-view test; two first goldens queued in `pending.txt` and classed `lit` (REND-006)
- [x] 1.3 World-anchored grid: `grid_phases` per node, contours from 1 m, relief only when selected, wrapped lighting; tier-A phase test, relief determinism test (REND-006)

## 2. Flight output

- [x] 2.1 Flight defaults to `relief`; `contact_sheet.png` (`sheet.rs`) and the `finest_level` column (TEST-013)
- [x] 2.2 Tier-A tests for sheet indices, labels, layout; tier-C tests for the sheet and the index (TEST-013)
- [x] 2.3 Legibility check (edge density) with a `tile-id` negative control (TEST-017); default-flight determinism test (TEST-013)

## 3. Close-out

- [x] 3.1 Golden review (both candidates escalate: first goldens of a lit view), verifier, spec-guardian, numerics-reviewer (findings fixed: tile-anchored grid, 1 cm contours, relief in every view, missing determinism tests, classes, spec deltas)
- [x] 3.3 Archive, status line, learning
- [x] 3.2 Relief flight rendered on the RTX 3080 Ti for the owner (`target/review/flight-relief-nvidia/`, not committed; re-rendered after the review fixes)
