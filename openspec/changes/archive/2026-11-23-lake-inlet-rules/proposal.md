## Why
- D5's remainder (lead #182, 2026-11-22): every `lake_inlet` must enter something. On g1f15's graphs R1h holds 146 and M1 2,443 inlets that no reach ends at and no basin spills to.
- On R1h, 993 line-end inlets lie in no kept basin, and on M1 12,797 (#182a, DEC crusader-32). A spill target between reported basins is a hierarchy record, not a lake entry.

## What changes
- GEN-189 (lead #182a): a `lake_inlet` that no reach ends at and no basin spills to is a violation (`OrphanLakeInlet`). An open slice does not check it.
- BUILD-060:
  - the lake inlets inside, by origin: `g_lake_inlets`, `g_inlet_line_ends`, `g_inlet_spill_targets`, `g_inlet_orphans`;
  - the line-end inlets in no kept-basin interior: `g_inlet_no_basin`, and `g_inlet_no_basin_far` for those not within one cell of one.
  - These are columns only; no bar changes.
- Which basin region the G3 gate holds a line-end inlet to (this interior, or the region below the recorded spill) is the lead's open question (crusader outbox 2026-11-23 05:25).

## Expected golden changes:
- none: validators and the gate are CPU checks; no rendered output changes.

## Impact
- Crates: generators (`river_graph.rs`), cache (`river_gate_graph.rs`, `river_gate.rs`).
- Tests (tier A): `an_orphan_lake_inlet_is_found_and_a_referenced_one_is_not` and `a_line_end_inlet_outside_every_kept_depression_is_counted_and_spill_targets_are_not`.
- A contract rule changes, so the merge waits for the lead's review (routing, rule 4).
