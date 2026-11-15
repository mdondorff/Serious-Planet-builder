# SPIKE-123: basins and spill relations as first-class objects (P3 E5, D-4)

Opened: 2026-11-15 (lead #145)
Question: can basins, their hierarchy, spill relations, classes and water levels be computed on any surface, deterministically, and what do they show on the A stores?
Why it matters: D-4 (lakes and basins) and its gate G-LAKE; E4's design note §5 needs basin id, spill point, active outlet, class, level and `q_out` from D-4.
Time box: 1 day of agent work.
Success criteria: the objects computed on the A stores of all five windows, counts per class, and a G-LAKE fixture designed for the whole chain.
Fallback if it fails: the flat basins of H1 (`Hydro::basins`, GEN-145) stay the only model.
Code location: branch spike/lakes-d4 (local, never merged): `crates/cache/examples/lakes_d4_fsm.rs.inc`, `river_gate --lakes`, `lakes_selftest`, `lcell_latlon`.

## Method
- **Sources:** Barnes, Callaghan and Wickert, "Computing water flow through complex landscapes", Part 2 (depression hierarchies, 2020) and Part 3 (Fill-Spill-Merge, 2021). The steady form is used here. The papers were not downloaded; the algorithm follows their published structure.
- **Hierarchy:** one sweep over the cells in increasing (height, cell id) order, with a union-find of components.
  - A cell with no processed neighbour starts a pit (a leaf).
  - A cell that joins two depressions is their saddle: it creates their parent at its height.
  - A depression that meets an outlet, or land already drained, is linked. Its spill point is that cell; its spill target is the node, or the outlets, on the other side.
  - Every cell is put in the node it floods in.
- **Water:** runoff on the steepest-descent watershed of each pit (the raw surface) feeds the leaves. The climate is H1's: runoff and open-water evaporation `E_open = PET - (P - r)` per regional cell.
- **Steady Fill-Spill-Merge:**
  - a body fills until `E_open` over its area equals its inflow;
  - a full body overflows to its spill target;
  - a full child fills its sibling;
  - two submerged children merge into their parent.
- **Classes**, applied in this order (min depth 0.5 m and min size 16 cells, the gate's basin metric):

  | Class | Rule |
  |---|---|
  | artefact to drain | floor to spill under 0.5 m, or under 16 cells at the spill, whatever water it holds |
  | kept lake | full: its spill point is the active outlet |
  | dry | no water reaches it |
  | endorheic | the rest: closed, level below the spill |

- **Outputs per body:** basin node (`floor_cell` names it, as `basin_id` does), spill cell, spill target, class, water level, `q_in`, `q_out`. These are the fields E4 §5 names.
- **Determinism:** single-threaded, with a fixed visiting order and sums in cell order (CON-14), in f64 arithmetic only. Two runs on the two R1h A stores (crusader's `r1h-round-k8` and explorer's `a-r1h`) gave identical output.
- **Store mode:** the cells outside the commit's tiles drain. They stand below the sea, as in the gate's routing; otherwise the higher start surface around the tiles dams them.
- **Fixtures** (`lakes_selftest`, two pits behind a saddle behind a rim with a gap):

  | Inflow | Result |
  |---|---|
  | none | both pits dry |
  | small | both endorheic |
  | 8 | the left pit fills and passes 0.57 m3/s into the right, which stays endorheic |
  | 20 | both submerge, the parent spills at 6 m with 8.57 m3/s out |

## Counts per class and window
Bodies whose floor lies inside the region's box (620 regional cells); water areas in km2.

| Window | Surface | Nodes (pits / merges) | Kept lakes | Endorheic | Dry | Artefacts | Artefact depth p50 / p90 | Time | Peak |
|---|---|---|---|---|---|---|---|---|---|
| R1h | today's start surface | 87,113 (58,176 / 28,937) | 5,887 (11,911) | 80 (338) | 0 | 5,539 (448) | 0.46 / 3.73 m | 263 s | 6.5 GB |
| R1h | A (explorer `a-r1h`) | 32,423 (29,327 / 3,096) | 35 (11.8) | 0 | 0 | 15,513 (275) | 0.05 / 0.14 m | 27 s | 5.5 GB |
| R3a | A | 39,228 (36,943 / 2,285) | 2 (0.9) | 0 | 0 | 27,926 (747) | 0.04 / 0.11 m | 35 s | 6.5 GB |
| R3b | A | 42,126 (38,812 / 3,314) | 40 (24.8) | 0 | 0 | 25,876 (479) | 0.05 / 0.14 m | 26 s | 6.5 GB |
| M1 | A | 107,360 (82,387 / 24,973) | 1,056 (665) | 0 | 0 | 39,480 (874) | 0.07 / 0.19 m | 31 s | 5.5 GB |
| F1 | A | 66,183 (60,146 / 6,037) | 25 (9.6) | 0 | 0 | 47,279 (896) | 0.05 / 0.14 m | 28 s | 5.5 GB |

- Time is the analysis after the surface is built: the hierarchy takes 14-23 s; peak is the whole gate run. The R1h start-surface row is slower (263 s) because it has 58,176 pits and its steady solve runs over many more merged depressions.
- Kept-lake depths: R1h today p50 10.0 m (up to 287 m); M1 A p50 13.2 m (up to 194 m); the other A stores p50 0.7-1.1 m.

## Findings
1. **The round at L drains the lakes.** On R1h today's surface holds 5,887 kept lakes (11,911 km2, about 4.7 % of the region); the A store keeps 35 (11.8 km2). The round's breach drains every closed basin (SPIKE-120 item 2), real lakes included. Only the mountain window M1 keeps lakes (1,056, 665 km2, up to 194 m deep).
2. **Artefacts outnumber lakes everywhere** (15,513-47,279 per A store). They are a few centimetres deep (p50 0.04-0.07 m), at the scale of the stored change's 0.25 m steps. The classifier drains them by its rule.
3. **No endorheic basin on any window:** with this climate's `E_open`, every body that receives water overflows. An endorheic test needs an arid window, or a fixture with a set `E_open`.
4. **Spill relations:** 3,400-11,000 depression trees per A store spill into another depression rather than the outlets; the spill graph carries their overflow there.
5. **For D-4:** basins have to be identified before the D-1 stage, and a kept lake has to be protected through it, for example as an intentional basin (GEN-146) whose cells the breach may not lower. Otherwise the chain keeps no inland lake.

## G-LAKE fixture (designed now; run when E3 has a winner)
**The lake:** R1h, face 1, seed 14, L cell (65823, 7542), at -37.1001, -164.7368 (degrees).

| Property (today's start surface) | Value |
|---|---|
| Floor | 438.43 m |
| Spill | 535.90 m (depth 97.5 m) |
| Area at the spill | 12,548 L cells (130.0 km2) |
| Inflow / outflow | 3.61 / 3.20 m3/s, to the outlets |
| Class | kept lake (it overflows: a through-flow lake) |

On the A store it is gone.

**Mark:** the lake's cells at today's spill level as an intentional basin (GEN-146, A3 section 5.3), before the D-1 stage.

**Checks along the chain:**

| Stage | Check |
|---|---|
| Base (B + regional) | the basin exists: floor within 1 m, spill within 1 m, area within 5 % of the table |
| Rounds / D-1 stage | no lake cell lowered below its start by more than 1 m; the spill cell within 1 m of 535.90 m |
| H1 | a kept basin with this floor; class Lake, `overflows` true, level within 1 m of the spill; the channel continues below the spill (`q_out` > 0) |
| K7 | no channel cell burnt inside the lake (GEN-163); the burnt bed at the lake entry at or under the level; the rim unlowered |
| Store | the lake record (`Lake`: id, floor, spill, level, class, tiles) in the commit |
| Runtime read | the water mask shows the lake at its level over its area (within 5 %), with the river leaving at the spill |
| This spike's classifier on the stored ground | class kept lake, level within 1 m |

**Pass:** every check holds on E3's winning variant. One failing stage names the stage that drained it.

**Flight brief** (#146, filled in after E3):
- commit folder: the winner's store;
- command: `planet editor --commit <DIR> --site -37.1001,-164.7368 --marker <lat,lon>` with one `--marker` per kept lake of the window (from this classifier's kept-lake list, converted by `lcell_latlon`);
- what to judge: the water lies in a closed hollow at a level, not on a slope; the river enters and leaves at the spill; the shore follows the ground, with no straight cut through the rim.
- A variant that changes the base (E) needs its stated route into the viewer (VIEW-069 opens a commit over the in-memory production base). The command is smoked once by crusader before hand-over.

## D-4 notes (lead #150, 2026-11-15; from the owner's external review, F1)
**Level against H1, a reported number** (lead, 2026-11-15): a protected lake keeps the fine surface's own spill level, not H1's (raising the rim would build an artificial shore step). D-4 reports the difference per lake.

| Lake | H1 level | Level at L (SPIKE-124, R1h) | Difference |
|---|---|---|---|
| H1 lake 0 (0563b44fc93a2d1d) | 4.60 m | 4.05 m (main body, 47.2 km2) | -0.55 m |

**Area bookkeeping (review F1.3):**
- `hydro.rs` builds `curve_a` over the basin's members only (`filled - z > 0.5 m`).
- `lakes.rs` adds the shallow shore cells to `Lakes::water`.
- For H1 lake 0 that gives `Lake::area_m2` 49.9 km2 against a wet mask of 107 regional cells (about 71.0 km2 at 814.4 m). The L footprint below the spill is 5,317 cells (55.1 km2).
- SPIKE-124's "half the area" was not this bookkeeping. Survival was measured at L by SPIKE-123's classifier on the stored ground, and the stage trace showed a real leak: K7 lowered the rim by up to 7.6 m, and the lake held 25.7 km2 before the K7 fix and 55.8 km2 after.
- D-4 should give the record and the mask one definition of a lake's area.

**Into D-4's scope (not now):**
- **Dry-slope routing in an endorheic basin (F1.1, `lakes.rs:237-252`):** the routing runs on the filled surface, whose grade inside a basin points to the spill, so a channel entering on the sill side never reaches the water and runs uphill on the ground to the dry sill. K7 would burn it as an uphill trench. The review's fix: re-route the dry depression cells of a non-overflowing basin on a depression-local flood seeded from its water.
- **`e_open` (F1.2, `hydro.rs:323, 356`):** an unweighted mean over every depression cell up to the spill, against ADR 0114's true cell areas, and over dry walls rather than the water at each level. The review's fix: an area-weighted evaporation curve `evap_m3s[k] = sum over z <= h_k of a(c) E_open(c)` beside `curve_a`.
- Both are latent today: SPIKE-123 found no endorheic basin in this climate.

**The failing tier-A test** (on branch spike/lakes-protect, `crates/generators/tests/bake_lakes.rs`, commit 76b7c02c, never merged; review F1's bowl, heights shifted by +500 m):
- It **fails on main: 333 dry channel cells of the basin step uphill toward the sill.**
- Its setup holds: one kept basin, endorheic.
- It is D-4's test to make pass.

```rust
/// The external review's F1 (2026-11-15) as a tier-A test, heights shifted by +500 m so nothing lies below the sea outlet: a 32 x 32 asymmetric bowl, floor 300 m at (8, 16), spill 550 m at
/// (28, 16) (a notch to the east border), a tributary trough from (24, 6) south to (24, 16) at about 510 m, then west, descending to the lake. An open-water evaporation keeps the lake endorheic.
/// Every channel cell of the basin that stands above its water must flow downhill on the ground (toward the shore), never up to the dry sill.
// spec: GEN-145, GEN-163 (D-4: review F1; fails on main: the filled routing sends the dry slope's channel uphill to the sill)
#[test]
fn a_tributary_on_the_sill_side_of_an_endorheic_lake_flows_down_to_the_shore_not_up_to_the_sill() {
    const N: usize = 32;
    let at = |i: usize, j: usize| j * N + i;
    let mut z: Vec<f64> = (0..N * N)
        .map(|p| {
            let (i, j) = ((p % N) as f64, (p / N) as f64);
            300.0 + 15.0 * libm::hypot(i - 8.0, j - 16.0)
        })
        .collect();
    // the rim: the border high, but one notch east of the spill
    for p in 0..N * N {
        let (i, j) = (p % N, p / N);
        if i == 0 || j == 0 || i == N - 1 || j == N - 1 {
            z[p] = 900.0;
        }
    }
    z[at(28, 16)] = 550.0;
    z[at(29, 16)] = 540.0;
    z[at(30, 16)] = 530.0;
    z[at(31, 16)] = 520.0;
    // the tributary: south along i = 24 from j = 6 (520 m) to j = 16 (510 m), then west to the lake (510 m down to 360 m at i = 12)
    for j in 6..=16 {
        z[at(24, j)] = 520.0 - (j - 6) as f64;
    }
    for i in 12..24 {
        z[at(i, 16)] = 360.0 + 150.0 * (i - 12) as f64 / 12.0;
    }
    let g = Grid::new(N, N, 800.0);
    let outlet = g.outlets(&z, 0.0);
    let routing = route_rules(&g, &z, &outlet, EPS, 3, 0.0, None, RoundRules::ROUND);
    let filled = priority_flood_with(&g, &z, &outlet, EPS, true);
    let runoff = vec![1.0e-8f32; N * N];
    let evap = vec![2.0e-7f32; N * N];
    let p = HydroParams { initiation_m2: 3.0 * 800.0 * 800.0, ..HydroParams::default() };
    let hydro = hydrology(&g, &z, &filled, &routing, &outlet, Some(&runoff), Some(&evap), &p);
    assert_eq!(hydro.basins.len(), 1, "one kept basin");
    let b = &hydro.basins[0];
    assert_eq!(b.class, BasinClass::Endorheic, "the evaporation keeps the lake closed: level {} m, spill {} m", b.water_level_m, b.spill_m);
    let lakes = lakes_from_hydro(&g, &routing, &hydro, &z);
    let water: std::collections::HashSet<u32> = lakes.water.iter().map(|&(c, _)| c).collect();
    let mut uphill = Vec::new();
    for c in 0..N * N {
        if !hydro.channel[c] || hydro.label[c] != 0 || water.contains(&(c as u32)) || z[c] < b.water_level_m {
            continue;
        }
        let r = routing.receiver[c];
        if r != NONE && z[r as usize] > z[c] + 0.25 {
            uphill.push((c % N, c / N, z[c], z[r as usize]));
        }
    }
    assert!(uphill.is_empty(), "{} dry channel cells of the basin step uphill toward the sill: {:?}", uphill.len(), &uphill[..uphill.len().min(6)]);
}
```

## Visuals
None: counts only. Pictures come with the G-LAKE flight on E3's winner.
