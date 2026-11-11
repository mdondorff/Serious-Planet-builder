## Why
- Spikes' re-read of G-SURF-I (#1351) saw burnt trenches 2.5 to 3.0 cells wide where the width law gives 1.92, and gaps at 3 of 10 stretches.
- Stage measurement (explorer, 2026-11-11): `burn_channels_wide` leaves no gap and raises no cell; the widening comes from the carve, which is the valley shaped around the burnt channel (architect, 2026-11-11).

## What changes
- GEN-168 states the burn's geometry as a requirement (no gap, no raise, mean width within the lattice tolerance of max(1, w)).
- TEST-110 tests it at tier A on a straight, a diagonal, a curved and an on-the-row channel at w = 0.5, 1.5, 2 and 3 cells.
- No code change in `burn_channels_wide` (architect, 2026-11-11).

## Ids
- GEN-168, TEST-110 (planner).
