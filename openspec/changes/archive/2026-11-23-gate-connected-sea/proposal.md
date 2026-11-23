## Why
- The gate's outlets were every cell at or under the sea level plus the edge (`Grid::outlets`), whether or not the cell is joined to the ocean.
- The chain uses the connected sea mask since readings 12 and 39: cells at or under the sea joined to the edge or the ocean.
- R1h's kept basin b38c730410e25493 has its floor at -20.87 m inland, under the sea level, with a recorded spill of 69.02 m.
  - No stage opens it (crusader outbox 2026-11-23 07:45).
  - The gate called its floor sea: dry, 89.9 m under its spill, g_spill_mismatch 1.
  - The external review named the gate's sea test a blind spot.

## What changes
- BUILD-055: the gate's outlets are the window's edge and the ocean (`river_gate::connected_sea_outlets`).
  - The ocean is the cells at or under the sea level on the start surface, joined to the edge or to an H1 sea cell inside the region, as the chain decides it; an outlet is an ocean cell still at or under the sea on the measured surface.
  - An inland pit under the sea level is no outlet, and the floods fill it.
  - A first form that joined the sea on the measured surface took sea cut off by a raised coast for inland pits (R1h's kept-lake area 778 → 30,711 km2). A second form that kept every start-ocean cell an outlet ended rivers on a raised shelf (R3a uphill 0.02 → 1.20, agreement 88.89 / 99.21 → 79.07 / 82.59).
- Land stays the cells above the sea level, unchanged: an inland cell under the sea fills as a pit and stays out of the land-only measures.
- BUILD-059's raw-grid input names BUILD-055's outlets in place of the old rule.
- Every flood-based column can move on a window with inland cells under the sea. The before/after rows on g1f15's five windows list each column that changes (crusader outbox).
- The G2 record's numbers stay as published; the changes go into the G2 report as an addendum (lead, 2026-11-23).

## Expected golden changes:
- none: the gate is a CPU measurement; no rendered output changes.

## Impact
- Spec deltas: BUILD-055 and BUILD-059.
- Crate: cache (`river_gate.rs`, the `river_gate` example's two call sites).
- Test (tier A): `an_inland_pit_below_sea_level_is_land_to_the_gate_not_sea`.
- A contract rule of the gate changes, so the merge waits for the lead's review (routing, rule 4).
