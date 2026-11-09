## Why
Inbox #111 and #113 (architect, 2026-11-09): at 6,144 cells the sediment pass's pit count is 2.2 times H1's on the same surface, and at 12,288 cells 18 times (27.6M against 1.49M cells). Only the flood differs: the sediment pass floods with the plain `priority_flood` (a FIFO queue for the flats, drained before the heap), the rounds, routing and H1 with the canonical flood (GEN-141, A3 section 6.4).

## What Changes
- The sediment pass floods with the canonical flood: `pit_cells`, and the filled surface and route its deposition sweep reads (`sediment.rs`). `deposit.rs` is unchanged.
- GEN-159 added (generator-pipeline).
- A test on a flat chain beside two outlets that fails on the plain flood.

## Impact
- Code: `generators::bake::sediment`, `tests/bake_round`, `examples/landmass_scale` (modes `full`, `diag`, `reconcile`). The default bake path (pass 1 and 2) does not call these functions; K7 (`k7.rs`) and `fill_depth` keep the plain flood (the architect decides them after l-store lands).
- Departure from the architect's list (accepted): `deposit_on_own_route` (`deposit.rs:161`) is called by the default pass 2 and by K7, not by the sediment pass, and stays on the plain flood.
- Not in this change: K7's flood, `fill_depth`, the k=4 reading (it comes with the phase-plan runs).
