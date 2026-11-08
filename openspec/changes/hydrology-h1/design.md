## Context
- A3 section 5.2: channel cells, the discharge `Q` and segments are kept apart from the diffuse field; section 5.3 to 5.5: basins, identity and lakes. The routing is GEN-141's (clipped MFD, canonical flood), on a plane grid or the cube grid (GEN-144).

## Decisions
1. H1 reads a routing and its filled surface; it computes nothing about the surface itself. The weights of the routing (GEN-129's `discharge_weight`) make `A_d` the runoff-weighted area; `Q` uses the runoff in m/s as the local term.
2. Channel cells: initiated (`A_d >= T`, not an outlet), then every D8 receiver downstream. The mouth (the outlet cell the channel ends in) is a channel cell and belongs to no segment.
3. Segments: from a head (a channel cell with no channel donor) or a confluence (two or more channel donors) to the next confluence or the mouth; the end cell is the next segment's first cell. `segment_id = hash(body, down cell, up cell)` over the grid's global cell ids.
4. Basins: the 8-neighbour components of the cells whose filled surface stands more than `pit_min_m` above the ground, found on the filled surface of the routing. This is the spill graph's level at the working resolution (the canonical flood is the minimax level plus the epsilon ramp); `bake/spill.rs` is not used here. Spill point: the rim cell of the lowest filled level, ties by cell id. Floor: the lowest cell, ties by id.
5. Class and level (section 5.5): `A(h) E_net = Q_in` on the 16-step curve; overflow (`A(spill) E_net < Q_in`) is a lake with the spill point as active outlet and `Q_in - A(spill) E_net` continuing; otherwise endorheic with the level interpolated on the curve. `Q_in` is the sum of `Q` over the basin's cells whose receiver is outside it. Net evaporation is an input (`None`: 0, every basin overflows).
6. Identity: `basin_id = hash(body, global id of the floor cell)`. Nothing else enters, so an edit elsewhere or deeper in the same cell keeps it and a new lowest cell moves it.
7. `Q` is the potential discharge before basin losses: it is not reduced at an endorheic basin or a lake (A3 section 5.2 requires the sum at the outlets to equal the sum of `a r`). The basin's `q_out` is the discharge that continues below the spill point; an endorheic basin passes none. No channel ends at an endorheic terminal cell here; the mouths are the outlet cells.
8. Departures from A3 section 5.3, recorded for the planner: (a) the basins come from the filled surface of the routing, not `bake/spill.rs` (that module is the tile-seam minimax and returns no spill points or basin objects); (b) two nested basins with different spill levels that touch are one component with one floor and one spill (the lowest rim). Not tested against `spill.rs`.
9. `q_in` is the sum of `Q` over the basin's exit cells, except an exit whose flow re-enters the basin within 256 cells (over a shallow margin or an island under the water): that exit is inside the next exit's `Q`. It includes the runoff of the basin's own cells; the net evaporation `max(PET - P, 0)` already nets the rain, so the balance is approximate. A net evaporation needs a runoff field in m/s.
10. The pit limit and the flood epsilon couple: a flat of 500 cells drained over the epsilon ramp (1 mm a cell) stands 0.5 m above the ground and counts as a pit. A component whose depth from floor to rim is under the pit limit is a ramp, counted in `ramps` and not kept. The curve at the spill leaves out the shallow margin cells (flooded, not pit cells) and members at or above the spill level.
11. Segment and basin ids carry a kind word, so they never collide.
12. Presets used (ADR 0108): `T` 75 km^2 (A3 range 50 to 100), `pit_min_m` 0.5, `min_area` 1 km^2, `min_depth` 2 m. At the prototype's 10.6 km^2 cell the artefact criterion (below 1 km^2 and shallower than 2 m) cannot be met: no basin is an artefact (0 of 202,360). Corrected in the h1-corrections change: the limit is `min_cells` (4) and `min_depth_m` (2 m).

## Measurements (`examples/hydro_landmass`, release, 8 threads, the prototype landmass of GEN-144: 9,308,154 land cells, runoff of the orographic climate, seed 14)
| Value | Result |
|---|---|
| Time | runoff and weights 10.1 s; weighted route and filled surface 22.6 s; H1 2.3 s |
| Channel cells | 3,678,508 (39.5 % of land): 1,107,282 heads, 854,511 confluences (donors of two or more, mouths included), 24,613 mouths |
| Segments | 1,955,381; 1,108 span a cube edge; longest 124 cells |
| Largest mouth | face 5 cell (2899, 1315): 38,463 m^3/s |
| Basins | 202,360 kept (178,821 lakes, 23,539 endorheic), 0 artefacts, 28 ramps rejected, 2,732,120 cells in basins; 15,588 kept basins are shallower than 1 m |
| Largest basin | floor 3 m, spill 429 m, area at the spill 457,619 km^2, volume 103,411 km^3, inflow 29,461 m^3/s, lake |
| **G-CON** | 0 disconnected segments, 0 channel cells without a downhill channel receiver or an outlet, 0 unaccounted cells |
| Confluence `Q` | largest relative error 4.6e-16 |
| `Q` at the outlets against the sum of the local terms | 2.8e-13 |
| `A_d` at the outlets against the weighted true areas | 1.6e-13 |
| `A_d` at the outlets against the true areas, unweighted routing, across cube edges | 8.0e-14 |

## Peak memory of the resident round (`hydro_landmass mem`, process working set read by PowerShell; the cube grid has 56,623,104 cells, 9,408,756 active; the arrays cover all of them)
| After | Peak | Now |
|---|---|---|
| Set-up (heights f64, active, outlet) | 653 MiB | 600 MiB |
| One resident route, kept | 2,328 MiB | 1,464 MiB |
| 4 rounds on 8 threads (91.7 s), heights copy held | 2,981 MiB | 1,037 MiB |
- The resident route is dropped before the rounds, so route and round are not resident together; the H1 pass is not in this run. One run; the Windows working set, not the commit charge; read by PowerShell, so Windows only. The 12 GB is ADR 0115 (proposed).
- 55 bytes a cell of the sphere at the peak (the example's arrays included: two f64 height copies, the masks). Against the proposed 12 GB: 25 %.
- The mask does not shrink the arrays; a compacted landmass layout would hold 9.4e6 cells.

## Earth-specific values (ADR 0108 point 4)
| Value | Earth | Parameter (source) |
|---|---|---|
| Initiation `T` | 75 km^2 of runoff-weighted area | `HydroParams::initiation_m2` |
| Pit limit | 0.5 m | `HydroParams::pit_min_m` (A3 section 5.3) |
| Artefact limits | 4 cells, 2 m | `HydroParams::min_cells`, `min_depth_m` |
| Reference runoff | 300 mm a year (weight 1) | `runoff::RUNOFF_REF_MM` (Earth's mean continental runoff) |
| Flood epsilon | 1 mm | `CarveParams::epsilon` (calibration; couples to the pit limit) |
| Body word | 0 | `HydroParams::body` |
| Seconds a year | 31,557,600 | the example's `SECONDS_PER_YEAR` (the library reads m/s) |
| Curve steps | 16 | `CURVE_STEPS` (A3 section 5.3) |
