# Change: item-index-fine

## Why
- SPIKE-29 fix 4 (opus-planner, 2026-10-22): a height query in a cluster of 5,000 to 20,000 items costs 0.5 to 2.4 ms, all of it the scan of the candidates the 40 km cell index gives (the cells of 7 x 7 rings round the cap, every item in them); the cold plan (60 s at 20k), tile generation and picking pay it per call.

## What changes
- `reach_index`: the items filed by reach class in cells sized for the class; `WorldDefinition` keeps it in step with put, remove and the radius change; the footprint query takes its candidates from it.
- WDEF-014.

## Tests
- Tier A (`crates/world-def/tests/reach_index.rs`): equal to the scan for random sets after every kind of edit; the entries looked at are bounded by the neighbourhood.

Expected golden changes: none (the result of the query is unchanged).
