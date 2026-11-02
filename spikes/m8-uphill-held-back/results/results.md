Opened: 2026-10-28
Question: (1) How far do the drawn river beds rise downstream (uphill or sideways rivers, owner frame-139490), and why; does the erosion after routing change it. (2) How many tiles are held back (ADR 0074) without edits at chunk margins of 0 and 10 m, along the standard flights and the owner-test poses; what caused the ~10 held tiles before the owner's big edit.
Why it matters: baselines for the D0 decision (owner, 2026-10-28; `docs/milestones/M8/owner-test-2026-10-27/README.md`, PR #841).
Time box: 1 day
Success criteria: (a) rise count, length and size at the site and over the 48-chunk set, classed by cause; (b) held-back counts at margin 0 and 10 m along flights and owner poses, offenders named; (c) the ~10 tiles explained
Fallback if it fails: none (measurement)
Code location: branch `spike/m8-uphill-held-back` (local, not pushed), commit b0662599 on `spike/m8-channel-depth`; patch in `proto/probes.patch.txt` (tests `uphill_report`, `held_flights_report` in `crates/app/src/globalperf.rs`)
Closed: 2026-10-28

## Method
- Store `C:/DEV/SeriousPlanetData/global16384` (read-only), one process per probe, objects in memory only. Raw lines in `raw/`.
- **Uphill metric (fixed definition, also sent to sonnet-crusader):**
  - River cells of a chunk's core: corridor cells and channel cells (area over `channel_area_m2` 5e4 m2); paths from every head downstream along the solution's receivers, ending at a cell outside the core, a non-river cell or a cell already walked.
  - Drawn bed: `ChunkField::height_at_level(dir, 16)` = base + chunk change, at 8 samples per link (cell centres and 7 points between).
  - A rise: a maximal run of non-falling samples downstream that gains over 0.5 m (size = gain, length = samples x cell length / 8, cell 8 m).
  - Rate: rises per 100 km of river path. Populations: the 48 random land chunks of `anet_change_report` (209.7 km) and the chunk under frame-139490 with its 8 neighbours (209.3 km).
  - Classes (flags): chunk border (a run cell within 2 cells of the core edge), base rises (base height alone gains over 0.5 m over the run), chunk change rises (drawn minus base gains over 0.5 m), coarse grid rises, cut cap on the path (GEN-069, bed within 3 % of the cap under the base), corridor edge in the run (anchored and free cells). Erosion step: bed before it = lower of the base and the corridor anchor at the same samples.
- Held back: `select_nodes_in_view_field` with the plain field (no chunk stage) and the chunk stage's bounds widened by 0 m, 10 m (both ways) and the built 300 m; counted `capped` nodes (independent of chunk readiness, LOD-013). 275 poses: 224 flight poses (altitudes 30, 100, 300, 1000 m x 8 start places 0.6 deg apart x a pose every 2 s of a 12 s flight at 300 m/s; envelope LOD parameters, cells 32, tau 6, 1440 p), the 14 F9 bundles (own LOD parameters), the last pose of each of the 37 stall reports (the bundle LOD parameters).

## Results
### 1. Rises along the rivers
| Population | River | Rises over 0.5 m | over 2 m | over 10 m | Size p50 / p99 / max (m) | Length of all rises |
|---|---|---|---|---|---|---|
| 48 random chunks | 209.7 km | 209 (99.6 per 100 km) | 36 (17.2) | 25 (11.9) | 0.69 / 19.3 / 19.5 | 1.00 km |
| site frame-139490, 9 chunks | 209.3 km | 476 (227 per 100 km) | 0 | 0 | 0.61 / 1.22 / 1.59 | 2.44 km |

- At the cell centres the drawn bed equals the solution (max difference 0.000 m) and **no river rises** (0 of 52k river cells; GEN-048 holds; the change clamp is never reached). The rises are between cell centres.
- Classes (flags):
| Population | chunk border | base rises | chunk change rises | coarse grid rises | cut cap | corridor edge |
|---|---|---|---|---|---|---|
| random (209) | 6 | 18 | 181 | 0 | 0 | 0 |
| site (476) | 10 | 20 | 389 | 0 | 0 | 2 |
- Primary class by the first cause that holds: random: border 6, base relief alone 18, chunk change 175, neither alone 10; site: border 10, base 17, change 377, both 3, neither 69.
- Erosion step (bed before it against after): random 7 rises before, 3 gone after, 209 after, **175 not there before**; site 25 before, 10 gone, 476 after, **377 not there before**. The rises are made by the chunk stage's change (incision step, banks, slump), not by the coarse path.
- The 36 rises over 2 m (random; 25 over 10 m, up to 19.5 m, 0.15 km in all) lie in the chunk-change class; their site was not looked at.

### 2. Held-back tiles without edits
| Pose set | Poses | No chunk stage | Margin 0 m | Margin 10 m | Margin 300 m (built) |
|---|---|---|---|---|---|
| envelope flights | 224 | 0 | 0 | 56 poses (all altitude 30 m), total 922, max 27 | 224 poses, max 127, mean 91 |
| owner bundles | 14 | 0 | 0 | 2 poses (frame-134050: 98 of 2244; frame-139490: 104 of 2293) | 7 poses, max 141 |
| stall report poses | 37 | 0 | 0 | 0 | 15 poses, max 125 |
- Margin 0 holds back none anywhere: no tile of procedural ground is too steep (the owner's target of 0 is met at 0 m). Margin 10 m: none at 100 m altitude and above; at 30 m altitude all 56 flight poses hold 11 to 27 tiles of level 21, and the two river-site bundles hold 98 and 104 tiles of level 20 (tile edges of a few metres to ten metres; the camera 17 m up).
- The bound term that is loose is the flat widening itself (+/-10 m on tiles a few metres wide, whose morph headroom is smaller); the plain bounds are not.
- The ~10 held tiles of the console before the big edit: at the last pose of each stall report the **300 m margin alone holds 1, 5, 5, 12, 22, 22, 48, 57, 73, 82, 99, 106, 108, 115, 125 tiles (15 poses, none of them edited ground)**; at margin 0 and 10 m the same poses hold none. The small counts are the margin, not steep ground.

## Findings
1. No river drawn at the 8 m cell centres rises (0 of 52k river cells); between cell centres 209 rises over 0.5 m in 210 km (random chunks) and 476 in 209 km (site), nearly all 0.5 to 1.6 m.
2. The cause is the chunk change varying along the path against the base's finer relief (181 of 209 and 389 of 476 rises); coarse path, cut cap and corridor edge contribute none or 2; the chunk border 6 and 10 (about 3 to 5 %).
3. Big rises (over 10 m, up to 19.5 m) occur in the random set only (25 of 209), unexplained here; they are short (0.1 km in all).
4. Margin 0 m holds back none; 10 m holds back tiles only at 30 m altitude or lower (all flight poses at 30 m, the two river-site bundles); 300 m holds back tiles at every flight pose and explains the console's counts without any edit.
5. Limits: the owner's "river runs uphill" at frame-139490 may be the water surface or the coarse path drawn at coarser levels, not the bed at level 16; this probe measures the bed at level 16 only. Rises are measured on the corridor and channel cells together (12,751 of 26,098 cells corridor in the random set, 1,241 of 26,047 at the site). One flight speed (300 m/s), 8 places, no edits.

## Visuals
Nothing to look at: numbers only, no renders.
