## Why
- The river-graph plan's G2 reads RG and B through one gate build (lead #165); the G0 contracts fix the graph reports, the graph/raster agreement (section 5), the budget (section 6) and the metrics (section 9) before any RG result.
- The gate read rasters only: no canonical graph, no stage fields, no partition pair.

## What changes
- BUILD-060 (new): `river-gate` takes `--graph`, `--stages`, `--partition` (per table row `#graph=`, `#stages=`, `#partition=`) and prints the G2 columns, each with its gate class: graph validity, trunk continuity and outlet connectivity, conservation, bed profile, exceptions, tributary attachment, hierarchy (density and `R_b` against BUILD-059's bar), basin consistency, graph/raster agreement, per-stage budget use, partition diff. A row without them (B) prints none.
- TEST-137: a hand graph in a 12 x 12 window checks each part.

## Impact
cache (`river_gate_graph.rs` new; `river_gate.rs` `GateInput::g2`, `GateReport::g2`, the classes line; the `river_gate` example), xtask (`rivergate.rs` row suffixes and the class row; help); tests.
