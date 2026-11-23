## Why
- G3-1 (gate integrity, D10) is the first row of the pre-production gate: the gate must judge B and RG by the same complete set of bars before any G3-B switch row is judged (lead #186, 2026-11-23; G3 gate §3, risk R2).
- Blind spots named by the reviews (lead's synthesis A4-A6, N13-N16; Carbon M4.7; Fable F2 and F4):
  - only the routed share of the graph/raster agreement is barred; `routed_above_head` is unbounded;
  - uphill and ponds are barred on the 75 km2 raster network only;
  - a kept-basin interior is the whole depression that holds a kept floor, not the lake the graph records;
  - hump cuts (894 over 10 m on R1h), corner-only links and trunk-profile exceptions are in no bar;
  - three bars are keyed on `win == M1`;
  - the S0 preservation bar (decision 3) is printed and not barred;
  - the margin ring outside the region's box (63.1 % of the window) is not measured.
- Bars set after G3-B's rows would be tuned to a candidate (CON-31): each new bar's value is proposed here, beside B's and RG's values, for the owner (O10).
- G3-8 needs 10 unseen windows fixed before anyone looks at their terrain (lead #186a).

## What changes
- BUILD-060:
  - kept-basin interiors become the recorded-spill lake: the cells of the measured surface under the basin's recorded spill joined (8-connected) to its floor cell (#182c);
  - `bar_agree_<A>km2` on `min(routed share, graph share)`; `bar_above_head_<A>km2` bounds `routed_above_head` as a share of the routed cells;
  - uphill and ponds at 1 km2 beside 75 km2 (`bar_uphill_1km2`, `bar_ponds_1km2`);
  - `bar_hump_cuts` (cut > 10 m through a reference hump > 10 m), `bar_corner_links` (corner-only 2x2 links over 5 m added to S0's), `bar_trunk_exceptions` (trunk reaches flagged `rise_over_budget` or `budget_residual`, as a share);
  - relief-indexed thresholds from the window's start relief in place of the `win == M1` literals;
  - `bar_saddles_s0` (decision 3) in `--table`, against B's row;
  - margin columns `m_*` over the window's ring outside the region's box, reported only.
- BUILD-066 (new): `cargo xtask windows-draw`, a seeded stratified draw of 10 windows over the seed-14 world's coarse field, recorded as data.
- The values proposed for the new bars, with B's and RG's five-window values, are in the PR body; the owner confirms them (O10) before any G3-B switch row is judged.

## Expected golden changes:
- none: the gate is a CPU measurement; no rendered output changes.

## Impact
- Spec deltas: BUILD-060 (modified), BUILD-066 and TEST-149 (added).
- Crates: cache (`river_gate.rs`, `river_gate_graph.rs`, `river_suite.rs`, `window_draw.rs`, the `river_gate` and `windows_draw` examples), xtask (`windows-draw`, `river-gate --table`'s `bar_saddles_s0`).
- Data: `crates/cache/data/g3-windows.toml` (the drawn windows, pre-registered).
- Every graph row's bar set grows; the G2 record stays as published.
- A contract rule of the gate changes: the merge waits for the lead's review and the owner's values (O10).
