## Why
- The inlet floor (lead #179, spike) raises a line's graph bed to a kept lake's level. Where the raise stops at the depression's edge, the rise is the contract's residual: a need not done (G0 contracts section 6).
- An `exception` marker excuses a rise at its vertex, but a reach's last vertex is its `down` node, a plain node; a marker may share that anchor only as the up node at vertex 0. On F1, 6 of 36 such rises have no place for a marker.
- Reading 41 (lead, 2026-11-22): a rise into the reach's own down node is excused when the reach carries `budget_residual` of at least the rise. This extends reading 32's form (a rise excused by the reach's record) to `budget_residual` at the down node.

## What changes
- GEN-189:
  - The bed rule allows a rise into the reach's own `down` node of at most its `budget_residual` value.
  - Still a bed rise: a larger rise, a rise at another vertex, or a reach without `budget_residual`.
  - `excused_down_rises` counts the excused rises and their metres.
- BUILD-060:
  - `g_excused_rises` and `g_excused_rise_m` (correctness) print the excused rises beside the violations.
  - The `RIVER-GATE graph` line names them.

## Expected golden changes:
- none: the validators and the gate are CPU checks; no rendered output changes.

## Impact
- Crates: generators (`river_graph.rs`), cache (`river_gate_graph.rs`).
- Tests:
  - tier A `each_injected_bed_and_conservation_failure_is_found`: the excused case and the three refused cases;
  - tier A `a_rise_into_a_down_node_excused_by_its_residual_is_printed_beside_the_violations`.
- A contract rule changes: the merge waits for the lead's review (routing, rule 4).
