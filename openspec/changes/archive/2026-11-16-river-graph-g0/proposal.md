## Why
- The river-graph plan's G0 contracts are agreed (owner, 2026-11-16). Lead #155 orders the G0 harness:
  - the contract's entities and identities become types;
  - canonical serialisation, diff, rasterisation and the validators each get an injected-failure test;
  - the fixture of TEST-139 gets its expected graph.
- No KS, KG, KV or KC algorithm.

## What changes
- GEN-189: `bake::river_graph` holds:
  - nodes, reaches, basins, domains and statuses;
  - ids (`node_id`, `reach_id`, `basin_id`, on GEN-145's mixer);
  - `Graph::canonical` and `Graph::parse`;
  - `diff` (topology apart from attributes, by (id, field));
  - `rasterise` and `validate_raster`;
  - `validate` (the contract's section 8, with the readings of its section 12);
  - `validate_budget` (section 6, on f64 with the residual rule);
  - `slice`.
- TEST-140: an injected-failure test for each validator. The fixture's expected graph is built by hand from TEST-139's terrain. It validates, round-trips, gives the same bytes on 1 and 4 threads, and runs under both partition layouts.

## Impact
generators (a new module, `bake/river_graph.rs`; no existing code changed); tests `river_graph.rs` and `g0_graph.rs`.
