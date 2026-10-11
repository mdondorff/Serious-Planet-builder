## 1. Relief view

- [x] 1.1 `TerrainView::Relief` (appended), shader branch with hillshade, tint, metric grid and fine contours (REND-006)
- [x] 1.2 Colour-rule arm in the every-view test; two first goldens queued in `pending.txt` (REND-006)

## 2. Flight output

- [x] 2.1 Flight defaults to `relief`; `contact_sheet.png` (`sheet.rs`) and the `finest_level` column (TEST-013)
- [x] 2.2 Tier-A tests for sheet indices, labels, layout; tier-C tests for the sheet and the index (TEST-013)
- [x] 2.3 Legibility check with a `tile-id` negative control (TEST-017)

## 3. Close-out

- [ ] 3.1 Golden review (candidates escalate: first goldens of a lit view), verifier, spec-guardian, numerics-reviewer (shader), archive, status line
- [ ] 3.2 Relief flight rendered on the RTX 3080 Ti for the owner (`target/review/flight-relief-nvidia/`, not committed)
