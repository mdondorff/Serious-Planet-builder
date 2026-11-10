## Why
Inbox #119 point 2 (architect, 2026-11-10): briefing item 11 closes on a re-read of G-SURF and G-SURF-I on current main with the canonical K7, plus the two G-SURF-I numbers A3 section 12.1 lists and `k7_region` did not print: the error against the local base slope and the overshoot of the runtime read.

## What Changes
- `bake::k7::SurfaceGate`: `max_error_over_rise`, `p99_error_over_rise`, `nodes_with_rise` (the difference over the largest rise of `B` across one cell, at the vertices whose rise is at least 1 m) and `max_runtime_overshoot_m`, `runtime_overshoot_nodes` (the runtime read `B(node) + C1(L)` outside the range of the four composed cells `B + L`).
- `examples/k7_region` prints both lines.
- GEN-154 (in `openspec/changes/l-store`): the G-SURF-I list names the two numbers.
- `tests/bake_k7`: the new fields are finite, ordered and counted on the smooth tile, with and without a channel.

## Impact
- Code: `generators::bake::k7`, `examples/k7_region`, `tests/bake_k7`. No bit moves; the example is a measurement.
- Re-read on main a90107ba (`k7_region 200 8`, prototype tile (65, 7) of face 1, canonical K7 as standard): G-SURF at the cell centres 5.7e-14 m before quantisation and 0.125 m after; G-SURF-I 192 of 28,900 vertices (0.66 %) over 0.25 m, p50 0.040, p95 0.134, p99 0.225, maximum 2.55 m; by the bake's step height (up to 5 m, over 5 m up to 20 m, over 20 m): 0 of 3,333, 20 of 10,895, 172 of 14,672 over 0.25 m; the largest difference over step height 0.098.
- New numbers: 25,795 vertices with a base rise of at least 1 m across a cell; difference over rise p99 0.057, maximum 0.315; overshoot of the runtime read beyond the four composed cells at most 2.63 m, 39 vertices over 0.25 m (the base re-read from the local cells loses up to 2.64 m: the overshoot is `B` against its own interpolant, not the stored change).
- Without the channels the same tile reads 209 vertices over 0.25 m (0.72 %).
- Not in this change: the ledger rows (they cite a main commit that holds the new numbers: a ledger PR after this merge), the verdict (the architect's).
