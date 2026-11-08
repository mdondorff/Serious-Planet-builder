# turned-noise (A2, GEN-140)

## What
- `height::set_turned_noise` switches the lattices of the hill band, the ridged mountain band and the erosion creases to a rotation per octave (`turn`, `layer_noise_turned`, `layer_noise_bounds_turned`). Off by default: every height, key and golden is unchanged.
- On, `BakeInputs::id` and `ErosionField::climate_version_word` mix a constant, so every key differs.
- Tests: `turned_noise` (switch off, rotation, bounds over caps, planes, band range).

## Not in this change
- The re-tune of the band factors waits for the prototype (inbox #91).
- The re-tune and the choice of turned lattice against gradient noise wait for the prototype. Sheet: `docs/dev_screenshots/2026-11-08-turned-noise-sheet.jpg` (`turned_noise_sheet` example): each octave keeps straight cell edges, now in its own direction.
