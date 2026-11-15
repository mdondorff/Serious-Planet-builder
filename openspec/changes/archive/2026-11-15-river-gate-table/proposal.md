## Why
- #1523 named a gap: the suite's store path had no tier-A test; it lived in the `river_gate` example only (lead #152, P3 E1).
- E3 compares six river variants on five windows: it needs every gate and suite number of many runs in one table (lead #152).

## What changes
- BUILD-057: every run prints a `RIVER-GATE fields:` line (every number, a count before every percentage); `cargo xtask river-gate --table <rows...>` runs the gate once per row and writes `target/review/river-gate-table.md` and `.csv`; the store path's tile laying is a library function (`lay_store_ground`) that the example and the test share.
- TEST-137: a small baked store read through the reader gives the report of the same surface; the fields and the table's parse and render are checked.

## Impact
cache (`river_gate.rs`, the `river_gate` example, tests); xtask (`rivergate.rs`, `main.rs`). The printed gate and suite lines are unchanged.
