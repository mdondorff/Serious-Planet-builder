## Why
- GEN-189's f64 budget validator (`river_graph::validate_budget`) is the budget's correctness gate (the G0 contracts' section 6), but no gate row calls it. BUILD-060's u16 stage fields are diagnostic only.
- The validator needs the f64 stage surfaces; dumping them costs about 2 GB a row. So the chain runs it and writes its records beside the stage fields, and the gate reads the count (lead, 2026-11-21).

## What changes
- BUILD-060:
  - `b_validator_overrun` (correctness): the lines of `budget-violations.csv` in the stage directory, one per overrun or raise in a lowering stage; `-` without the file.
  - `bar_violations` adds it beside `g_spill_mismatch` and `g_spill_rim`.
  - The `RIVER-GATE budget` line names it.
  - A line that is not four fields is refused, naming the file.

## Expected golden changes:
- none: the gate is a CPU measurement; no rendered output changes.

## Impact
- Crate: cache (`river_gate_graph.rs`: `read_validator`, the column, the bar; `river_gate.rs`; the `river_gate` example), with the tier A test in `river_gate_graph.rs` extended.
- Rows whose chain writes an overrun now fail `bar_violations`. The chain's writer lives on the spike branch (rg_chain).
