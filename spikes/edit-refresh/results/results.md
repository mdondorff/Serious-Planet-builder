# SPIKE-23 results: why the edit-latency case at 200 m with a 150 m stamp takes 508 ms to drawn-exact (edit-refresh)

Diagnosis only (planner brief, 2026-10-20). Spike code: branch `spike/edit-refresh` (never merged). Data: explorer's raw tables `sonnet-m4/target/perf18/{settle0,settle40}.txt` (edit-latency, RTX, 16384-cell store, 2560x1440, cells 32, tau 6 px, the two settle runs), `docs/perf/window-16-17.md`, and the code (`ManagedLoop::edit`, `Streamer::refresh`, `request_ahead`, `dispatch`, `MeshSource::load_staged_timed`, `compact_tile_field`). No `cargo xtask perf` was run; the counts are tier B (CPU only); one rough single-thread timing ran on a loaded machine and is reported as ratios.

## Answer in one paragraph
**The time is tile generation throughput: each drawn tile costs about 40 ms of one thread, almost all of it the evaluation of the eroded base field at 5,446 points, and the stamp is about 0 % of it.** The 150 m stamp at 200 m altitude re-keys 291 tiles of which 151 are drawn (68 of level 18, 83 of level 19); 18 workers generate them at about 2.75 ms per tile of wall time, drawn tiles first, so "drawn exact" is `80 ms + 2.75 ms x drawn tiles` in all nine cases (508 ms for 151 tiles; 178 ms for the 31 drawn tiles of the 50 m stamp; 96 ms for the 9 of 2 km / 150 m). The refresh regenerates the whole tile (the base field and the stamp) although only the stamp changed. There is no serial stage on the critical path and the queue order is right; the cost is per tile and per drawn tile.

## Break-down of 200 m / 150 m (settle 0, explorer's run)
| quantity | value |
|---|---|
| tiles re-keyed (named / resident) | 291 of 9,334; by level: 3 to 12: 45 (4 to 6 per level, none drawn), 13 to 17: 70 (none drawn), 18: 93 (68 drawn), 19: 111 (83 drawn) |
| drawn tiles to refresh (previewed nodes) | 151, all of level 18 and 19 (spacing 1.2 and 0.6 m) |
| first job dispatched | 2.9 ms after `set_definition` |
| plan delivered | 98.5 ms (worker 94.5 ms; 50.6 ms with a settle window: the plan worker competes with 18 busy workers for the 20 logical CPUs) |
| R (residency set) delivered | 501.7 ms (the plan worker, one thread, after the plan; not on the path to "drawn exact") |
| half of the named tiles refreshed | 268.9 ms |
| all named tiles refreshed / uploaded | 852.2 ms |
| drawn exact (all drawn tiles hold the new key) | 508.1 ms |
- Workers: 18 (`GenWorkers::machine_reserving(GEN_RESERVE = 2)` of 20 logical CPUs), `max_in_flight` = workers. All named tiles are requested in the one `edit` call (`Streamer::refresh` -> `request_ahead`): rank 0 in their class, the draw class (the drawn nodes) before the residency class (the named ancestors, resident only); nothing but the pool size limits the dispatch (`dispatch`: in-flight below the job limit). The 151 drawn tiles therefore go first: the data agree (a case with many resident-only tiles and few drawn ones, 2 km / 150 m with 57 and 9, is exact at 95 ms, not at 57 x 2.75 + 80).
- The settle window (ADR 0072, 40 ms) adds 36 ms (508 to 544 ms): not the cause.

## The fit over the nine cases (settle 0)
| altitude / stamp | drawn tiles | drawn exact ms | `80 + 2.75 x drawn` |
|---|---|---|---|
| 2 km / 50 m | 6 | 82.7 | 96 |
| 2 km / 150 m | 9 | 95.5 | 105 |
| 2 km / 500 m | 26 | 144.1 | 151 |
| 200 m / 50 m | 31 | 178.4 | 165 |
| **200 m / 150 m** | **151** | **508.1** | **495** |
| 200 m / 500 m | 281 | 848.0 | 853 |
| 1 m / 500 m | 309 | 919.4 | 930 |
| 1 m / 150 m | 309 | 912.5 | 930 |
| 1 m / 50 m | 321 | 953.0 | 963 |
The 200 m / 150 m case is not special: it sits on the line at 151 drawn tiles. The budget is 300 ms for the 50 and 150 m stamps and 1,000 ms for 500 m, so it is the only case whose tile count (about 285 ms of generation) falls in the band over a 300 ms budget; the 500 m stamp has twice the tiles and a 3.3 times larger budget.

## Where the 40 ms of a tile goes (`spike23_tilecost`, 5 runs x 24 tiles of level 19, one thread, median of the five runs)
| stage | median ms per tile |
|---|---|
| `compact_tile_field`, base field (no stamp) | 39.7 |
| `compact_tile_field`, field with the stamp | 40.0 |
| the 5,446 height evaluations alone (1 + 33 x 33 lattice points + 4 stencil points each) | 45.5 |
The machine was **not quiet** (the other coder sessions were building; the top CPU processes were the `claude` processes and the Razer app), so read the ratios: the stamp stage adds 0.3 ms (under 1 %), the height evaluations are the whole job. 40 ms x 151 tiles / 18 workers = 336 ms against the 415 ms observed (efficiency about 0.8 on a hybrid 20-thread laptop; the harness's CPU calibration reads 2.1 x slower per job at 18 threads than alone). The absolute per-tile figure with no load is open (one line in a window).

## Counts that bound the cheap fixes (`spike23_counts`, tier B: the harness's own plan and footprint, the 16384 store; the drawn counts equal explorer's exactly)
Drawn tiles named, and of those the number changed by the stamp by more than 1.0, 0.5, 0.25 of the screen-space tolerance at their distance (`distance x tau x pixel angle`, tau 6 px, 8.0e-4 rad per pixel), and the number changed at all (5x5 samples per tile, so a lower bound of the changed ones):
| case | drawn | > 1.0 tol | > 0.5 tol | > 0.25 tol | changed at all |
|---|---|---|---|---|---|
| 200 m / 50 m | 31 | 15 | 17 | 19 | 20 |
| **200 m / 150 m** | **151** | **102** | **109** | **116** | **125** |
| 200 m / 500 m | 281 | 211 | 220 | 233 | 252 |
| 1 m / 150 m | 309 | 232 | 244 | 255 | 276 |
| 2 km / 150 m | 9 | 2 | 3 | 4 | 4 |

## The cheapest fixes and their gain (drawn exact of 200 m / 150 m: 508 ms; model `80 + 2.75 x drawn`)
1. **Do not refresh what the stamp does not change, and not what the eye cannot tell** (the owner's decision on the meaning of "exact", status.md). Skip the named drawn tiles whose delta over their cap is zero (up to 26 of 151: the footprint adds a halo) and, as a second step, those below half the screen-space tolerance (a further 16 of 151, down to 109). Gain: 151 to 125 tiles: about 425 ms; to 109 tiles: about 380 ms. The preview shows the rest within 4 ms and an unrefreshed tile is within tolerance by construction. Cost: a tile-cap delta range (the stamp stage has `delta_range`) in the footprint find and the refresh; no format change. Not enough alone to reach 300 ms.
2. **Derive the normals from the lattice, not from four more evaluations per vertex.** 4,356 of the 5,446 evaluations (80 %) are the normal stencil (`normal_stencil`, +-t1 and +-t2 at one cell); evaluating the lattice with a one-cell border (35 x 35 = 1,225 points) and taking the normals from the neighbours' heights cuts the evaluations by 4.4: 40 ms to about 10 ms a tile. Gain: drawn exact about 80 + 0.65 x 151 = 180 ms (budget 300, all nine cases within budget), and **every** tile generation gets about 4 x cheaper (start-up load, dives, edits). Cost: the normals change a little (the stencil is in the tangent frame, the lattice in the face axes), `MESH_VERSION` bump, a spec change on GEN-025 and the numerics review, the goldens of lit terrain move at rounding level (`golden-review`), the key of every tile changes once (every cache regenerates).
3. **Cache the base samples of the tiles of an edit, so a refresh only applies the stamp.** The stamp stage costs about 0.3 ms of the 40; the 5,446 base evaluations are the rest and do not change when only an item changes. Keeping the base heights and climate of a tile (44 KB as f64, bit-equal to a re-evaluation, CON-14) for the tiles inside an item's reach, filled at the first refresh, makes every later edit of the place (a drag, a revert, the next step) cost the stamp only: about 1 ms a tile, drawn exact about 100 to 130 ms (bounded by the plan, 59 to 98 ms, and the uploads). The **first** edit of a place, with the cache empty, is unchanged (508 ms); the scene's median of three repeats would show the later runs. Cost: a bounded cache in the stamp stage (`add_stamps` in `stamp.rs`), memory 20 to 60 MB for the few hundred tiles of a 150 to 500 m stamp, invalidation when the base changes.
Order of cost to build: 1 (small), 3 (medium), 2 (medium, wide effect). 2 alone meets the budget; 1 and 3 compose with it (1 + 2: 80 + 0.65 x 109 = about 150 ms).
Not fixes: more workers (18 of 20 logical CPUs already; two more would give +11 %), a larger queue (no worker idles), the settle window (it adds 40 ms).

## Open
- The per-tile cost with the machine quiet (one line in a window; the stage ratio does not depend on it).
- The first-edit cost under fix 3.

## Files (spike branch `spike/edit-refresh`)
`crates/app/examples/spike23_counts.rs` (the drawn and changed counts), `crates/app/examples/spike23_tilecost.rs` (the single-thread stage timing).
