## Why
Inbox #116 (architect, 2026-11-09): G-SED's partition test (GEN-155): the same scenario under different partitions, totals identical, no load lost or counted twice, the residual reported per catchment. No spatial cut of the sediment pass exists; the test varies the partitions the pass has and names what stays open.

## What Changes
- `sediment::catchment_terms` returns the balance of every catchment (outlet, cells, eroded, raised, exported, relative residual); `catchment_balance` is the sum of those terms (same bits).
- GEN-155 added (generator-pipeline).

## The partitions varied
1. The representation and cell order: the compact cube against the masked cube (catchments keyed by the outlet's cube id).
2. The thermal step's bands: 1, 3 and 8 threads over the whole sediment pass, on a region of more than two pieces (n = 64).
3. Groups of whole catchments (outlet id modulo 2, 3, 5): the accounting a cut along the divides would hand to separate domains (the pass itself is not cut); the sums are also checked against the heights and the volume change read another way.

## Open
- A spatial cut (tiles or windows) of the sediment pass does not exist: the sweep is one serial pass over the region, and the boundary-as-outlet rule of `sediment::outlets` would turn a cut's border cells into outlets, so a cut needs an outlet closure of its own (not designed).
- Sediment inside the carve (A3 section 6.7) is not in this change; no sediment term in the carve.

## Impact
- Code: `generators::bake::sediment` (a function split, no bit moves), `tests/bake_compact`.
