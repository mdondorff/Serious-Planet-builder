Opened: 2026-11-01
Question: Is the Tracked item "Base LOD refused below about 31 km over a field with only a global bound" (VIEW-032) real on the real store `global16384`, or a harness artefact?
Why it matters: the item has been open since M6 (sonnet-crusader, 2026-10-20: 6.8 to 9.4 % of poses of a 20-31 km sweep over a zero-height field with only the global 9,735 m bound); SPIKE-22 cleared it on a 256-cell generated world, never on the production store.
Time box: 2 h (Sonnet)
Success criteria: refusal share by altitude band and terrain class on the real store against the generated world and the global-bound stub; the cause named (bound, harness or plan); the store unchanged.
Fallback if it fails: report the share on the generated world only.
Code location: `crates/app/examples/spike86_sweep.rs` on branch `spike/base-lod-real-store` (SPIKE-22's sweep ported to today's API; never merged); copy in `proto/`.
Closed: 2026-11-01

## Headline
- **The VIEW-032 item is a harness artefact: 0 refusals and 0 capped leaves in 400 real-store poses and 1,000 generated-world poses between 20 and 31 km, against 51.8 % of poses capped at level 11 over the global-bound stub (1,000 poses).** Over all altitudes, 0 of 1,500 real-store and 0 of 3,000 generated-world poses refuse or cap.
- **Since ADR 0074 the plan no longer refuses there: it caps.** The stub's poses that crusader saw refused at level 12 now plan with leaves capped at level 11 (0 refusals in 2,000 stub poses). A sweep that counts only `Err` sees nothing; this one counts `capped` leaves too.
- **A different finding on the real store: near the ground over steep land the plan caps leaves at levels 16 to 21.** 22 of 400 poses (5.5 %) over land with a bound range of 1,500 m or more in 10 km, all below 1 km altitude (0-100 m: 15 of 199, 7.5 %; 100 m-1 km: 7 of 123, 5.7 %; above 1 km: 0 of 78). The cause is bound looseness: the field's tile bounds are 5 to 37 times the sampled height range of the same tiles.

## Method
- Harness: the window's base LOD (cells 32, tau 6 at 1440 rows scaled with the height, viewport/tau 240, production finest level 22, `max_height_m` 9,735), 1280x720, `plan_terrain_field` per pose, as SPIKE-22.
- Sources: the real store (`WorldTerrain::open_store`, seed 1, 16384 cells, with `bounding_water`, as the viewer opens it); the generated world (`WorldTerrain::open`, 256 cells); the stub `Bound9735` (zero height, only the default global bound).
- Poses: random direction, altitude log-uniform 2 m to 30,000 km (or 20-31 km for the band runs), above the nadir's ground (`max(h, 0)`; SPIKE-22 measured from sea level), random heading, pitch ±0.05 to 1.35 rad. Half the poses redrawn until the nadir is land (`SP86_LAND`); steep runs redraw every pose until the nadir is land with a bound range of 1,500 m or more over 10 km (`SP86_STEEP`).
- Terrain class at the nadir: ocean (height <= 0); else the field's own bound range over a 10 km cap: plain under 300 m, hills under 1,500 m, mountains above. The class is only a label: the eroded field's bound range is itself loose (hills dominate the store's land for that reason).
- Counted per pose: a refusal (`Err`), and a plan with at least one `capped` leaf (ADR 0074), with the capped leaves' levels.
- Probe (`SP86_PROBE`): for up to 6 capped leaves per pose, the largest bound extent `height_bounds(centre, corner angle)` and the largest sampled range (17 x 17 heights) over the leaf and its four same-level neighbours.
- Store read only: no `--prefill-dir`, no writer; all 15 files under `global16384` have the same size and mtime before and after (`raw/store-files-before.txt`, `raw/store-files-after.txt`). All processes at BelowNormal; no perf, no GPU.

## Results
Poses / refused / poses with a capped leaf (the capped-counting runs; `raw/aggregate.txt`).
| Source | Altitude | Poses | Refused | With a capped leaf |
|---|---|---|---|---|
| Real store | 2 m - 30,000 km, all bands | 1,500 | 0 | 0 |
| Real store | 20-31 km | 400 | 0 | 0 |
| Real store, steep land | 2 m - 31 km | 200 | 0 | 8 (4.0 %) |
| Real store, steep land | 2 m - 1 km | 200 | 0 | 14 (7.0 %) |
| Generated world | 2 m - 30,000 km | 3,000 | 0 | 0 |
| Generated world | 20-31 km | 1,000 | 0 | 0 |
| Global-bound stub | 20-31 km | 1,000 | 0 | 518 (51.8 %), all at level 11 |
- Earlier runs counting only refusals: real store 3,000 + 600 (20-31 km) + 400 (steep) poses, generated world 3,000 + 1,000, stub 2,060: 0 refused (`raw/aggregate-refusal-only.txt`). The stub over all altitudes did not finish 60 poses in 25 min (thousands of nodes per plan below 31 km, as SPIKE-22 found).

### By altitude band, real store (all capped-counting runs)
| Altitude | Poses | With a capped leaf |
|---|---|---|
| 0-100 m | 540 (199 steep) | 15 (all steep) |
| 100 m-1 km | 316 (123 steep) | 7 (all steep) |
| 1-10 km | 284 (56 steep) | 0 |
| 10-31 km | 512 (22 steep) | 0 |
| above 31 km | 648 | 0 |

### By terrain class, real store, below 1 km over steep land
| Class | Poses | With a capped leaf | Capped leaves by level |
|---|---|---|---|
| mountains | 387 | 22 | 16: 472, 17: 657, 18: 143, 19: 113, 20: 210, 21: 144 |
| ocean, hills | 13 | 0 | - |

### Probe: bound against sampled range at the capped leaves (77 leaves, `raw/probe.txt`)
| Level | Leaves | Bound extent | Sampled range | Ratio | Plan limit (SPIKE-22 table) |
|---|---|---|---|---|---|
| 16 | 44 | 520-667 m | 26-111 m | 5.3-23.5 | 857 m |
| 17 | 22 | 347-427 m | 22-52 m | 6.9-17.8 | about 430 m |
| 18 | 5 | 140-204 m | 5.4-9.1 m | 22-37 | 214 m |
| 19 | 6 | 55-103 m | 4.2-10.9 m | 5.1-24 | about 107 m |
- The bounds sit just under each level's limit; the true heights of the same tiles are 5 to 37 times smaller. With bounds within 2x of the sampled range no leaf would cap.

## Cause
- VIEW-032 item: **the harness.** A field with only the global bound has a ±9,735 m volume at every tile; from level 12 the morph interval has no room (SPIKE-22's table). The real store and the generated world have local tile bounds and never reach that limit at 20-31 km. ADR 0074 turned the refusal into level-11 caps; the stub still shows them, the worlds do not.
- New finding: **the bound, not the plan or the terrain.** Today's field bounds its fine levels (16 to 21) analytically with a nearly level-independent amplitude (about 600 m at level 16, 400 m at 17), so over mountains the bound approaches the plan's limit while the sampled range stays at 5-110 m. This is SPIKE-82's bound looseness on today's field (SPIKE-82's "today" poses happened not to cap).

## What a fix PR would change
- VIEW-032 item: none in code; close the Tracked line. The tests that use a global-bound field assert `capped_tiles` as they do (ADR 0074).
- New item, one line: tighten the fine-level tile bound of the eroded field (the detail and crease octaves bounded per octave at the tile's scale, or R4's baked min/max pyramid of SPIKE-83 plus analytic runtime-detail bounds), with a tier-A test that a steep real-store tile at level 16-19 has a bound within a small factor of its sampled range.

## Limits
- 1280x720 only (viewport/tau is 240 at every size).
- The steep runs cover 400 poses; the 5.5 % share has a wide interval (about 3.5 to 8.2 % at 95 %).
- Classes come from the field's bound range, itself loose; "mountains" means a 10 km bound range of 1,500 m or more.
- Altitude above the nadir's ground, not above the highest ground in view.

## Visuals
Nothing to look at: plan statistics only, no renders.
