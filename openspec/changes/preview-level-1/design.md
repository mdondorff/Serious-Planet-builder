## Does level 1 need the stored-field read (item 3)?
No. Level 1 takes the resident base heights as an input array `z0`, one `f64` per H0 cell (6 n^2 cells, 450 MB at n = 3,072). How `z0` is filled is not this change's concern: today from the generator's base at the cell centres, later from the stored field's read when item 3 closes. The re-route does not read the store. `h0.rs` is not changed (`GlobalGrid::route` is called as it is), so nothing touches the explorer's work on `change/full-build` (its diff against main is empty at the time of writing).

## Shape (all GPU-free, in `generators::bake::preview`; CON-03)
- `PreviewBase`: the grid, the sea level, `z0` and the baseline routing of `z0`, kept for the session. Built once.
- `PreviewEdit`: stamps (`FootprintItem`s, applied by `stamp::apply_stamps` at each cell centre in the item's reach) and climate regions (`ClimateRegion`: a cap with a rainfall factor and a temperature offset; no item kind exists yet).
- `reroute(base, edit)`: the edited heights `z` (cells outside every reach keep `z0` bit for bit), the sea mask `z <= sea level` as outlets, `GlobalGrid::route`, the discharge by the routing weighted with `discharge_weight(runoff_mm(p, t))` per cell. A climate-only edit leaves `z` alone, so it reuses the baseline routing and only reweights.
- `Overlay`: the trunk cells (discharge at least a threshold, as indices), the watershed label of each cell (`outlet_of`), and the cells whose receiver or watershed changed against the baseline. Plain data for the plan (A3 11.2: the plan never requests a solve).
- `PreviewId`: a 64-bit hash of a domain tag, the base's identity word and the content hashes of the edit's items, so a preview never has the key of an authoritative field (CON-07). The preview is stored nowhere authoritative.
- `PreviewSlot`: a counter; `submit` returns a ticket, `accept(ticket)` is true only for the newest, so a result of a superseded edit is dropped (latest wins; the cancel itself is the caller's, a re-route is not interruptible mid-flood).

## Costs
- F6 measured 7 s and 2.2 GB for the planet at n = 3,072 with heights in memory; the step-2 example measures the real number per edit kind on this machine, after the explorer's re-run has ended. Until then the module is tested at n of 8 to 32.
- The stamp pass tests every cell against each item's reach (a dot product); the flood dominates.

## Order of work
1. This design and the module with tier A tests (`test fast`).
2. The latency example and scene (TEST-099), run only when bulldozer says the explorer's re-run has ended.
3. The overlay (VIEW-067).

## Findings for the architect
- No item kind for rainfall or temperature regions exists (`world-def` has stamps and regions that clip a height layer). Level 1 for the climate edit is therefore a function over a `ClimateRegion`; the item kind belongs to the editor path (item 16's later steps).
