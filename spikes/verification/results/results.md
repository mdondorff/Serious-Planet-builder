# SPIKE-120: the critical reads' verification items (P3 E2)

Opened: 2026-11-15 (lead #142)
Question: what the river gate says on the baked R1h store with one round at L, which part of the round drains and which incises, how Schott fair reads through the same gate, and what the worst-case main-stem edit costs.
Why it matters: P3 E3 (D-1, how drainage becomes valid at L) compares its variants against these readings.
Time box: 1 day of agent work.
Success criteria: the four items measured with main's river gate (BUILD-055) on the canonical windows.
Fallback if it fails: none needed (a measurement).
Code location: branch spike/verification (local, never merged): `bake_eroded`, `erosion_at_l` (`PLANET_ROUND_PART`, `PLANET_EDIT=mainstem`), `canyon_diag`, `river_gate --surface`.

## Method
- Windows (620 regional cells, margin 200, L cells of 101.8 m):

  | Window | Face | Regional cells i, j | Seed |
  |---|---|---|---|
  | R1h | 1 | 8056-8675, 760-1379 | 14 |
  | R3a | 4 | 368-987, 4720-5339 | 15 |
  | R3b | 2 | 9968-10587, 8304-8923 | 15 |

- The gate is main's (`planet_cache::river_gate::measure`), unchanged.
- Valley depth at ring 1-2 and 2-4, and the burn, come from `canyon_diag` (the split of DEC crusader-4). Explorer's gate is to print these parts itself.
- Columns are shares of river cells (%):
  - **valley 1-2:** more than 50 m below the p50 of the surface 102-204 m out;
  - **valley 2-4:** the same, 204-407 m out;
  - **burn:** the surface more than 50 m above the river level;
  - **erosion:** the river level more than 50 m below today's surface;
  - **basins:** at least 0.5 m and 16 cells;
  - **lakes:** at D = 20 m.
- The round is the spike's (SPIKE-115/117): `solve_rounds` once at L, steps 5, `RoundRules::default`, `window_cut`.

## Item 1: the baked R1h store with one round at L
Re-baked at kernel 8/10 (after the GEN-173 cap) with the production K7 (canonical flood). The store is `target/stores/r1h-round-k8` in crusader's worktree, commit 39038c59df52b635, 52 tiles. The gate ran in store mode with `--no-key-check` (a spike carve).

| Measure | Baked product (round + K7) | Round-only surface (river review) |
|---|---|---|
| Basins | 0.58 | 0.98 |
| Valley 2-4 (the gate's canyon) | 4.82 | 16.50 |
| Erosion | 37.11 | 30.75 |
| Lakes | 0.11 | 0.00 |
| Relief 2x2 / 10x10 | 136 % / 127 % | 114 % / 112 % |
| Edge | 34 of 44 | 36 of 44 |

- Cost: 520 s for the whole bake (front 36 s, the round 91 s, L channels 88 s, K7 over 52 tiles); peak commit 5.22 GB.
- The baked store is counted over its 52 tiles, not the region's box.
- Valley 1-2 on the store waits for explorer's gate; `canyon_diag` reads surfaces, not stores.

## Item 2: breach-only against carve-only at L
- Breach-only: `k_dt` 0, which also sets the uplift to 0.
- Carve-only: `breach_max_m` 0.
- Both: the round of the river review.

| Window | Part | Basins | Valley 1-2 | Valley 2-4 | Burn | Erosion | Lakes | Relief 2x2 |
|---|---|---|---|---|---|---|---|---|
| R1h | breach only | 0.00 | 8.76 | 12.65 | 0.00 | 11.68 | 0.00 | 102 % |
| R1h | carve only | 16.61 | 4.77 | 14.45 | 0.04 | 20.94 | 4.66 | 112 % |
| R1h | both | 0.98 | 5.29 | 16.50 | 0.00 | 30.75 | 0.00 | 114 % |
| R3a | breach only | 0.00 | 0.02 | 0.05 | 0.00 | 0.03 | 0.00 | 104 % |
| R3a | carve only | 14.73 | 0.19 | 0.74 | 0.00 | 0.99 | 0.00 | 118 % |
| R3a | both | 1.78 | 0.20 | 0.76 | 0.00 | 1.10 | 0.00 | 120 % |
| R3b | breach only | 0.00 | 0.92 | 1.37 | 0.00 | 1.78 | 0.00 | 102 % |
| R3b | carve only | 19.36 | 0.60 | 2.59 | 0.00 | 3.55 | 1.09 | 111 % |
| R3b | both | 4.40 | 0.74 | 3.20 | 0.00 | 6.00 | 0.00 | 113 % |

- **The breach drains.** Breach-only leaves 0.00 % basins on all three windows; carve-only leaves 15-19 %.
- **The breach makes the narrow incision on R1h:**
  - breach-only gives 8.76 % at valley 1-2, more than the whole round (5.29 %) and nearly twice carve-only (4.77 %);
  - the breach cuts V-notches through sills, which the carve partly widens when both run.
  - On the flat windows breach-only stays at or under 0.92 %.
- **The carve makes the depth and the relief:** most of the erosion depth (20.94 % against 11.68 % on R1h) and the relief gain (112 % against 102 %).

## Item 3: Schott fair through the gate
- Source: spikes' final SPIKE-118 surfaces (the D4 hand-over, then per-scale rounds at 407/204/101.8 m, 2 a scale, retargeting, multi-scale breach).
- Format: whole windows, f64 (R1h widened from f32, about 0.5 mm at 3 km). Each is a start surface, not baked.

| Window | Basins | Valley 1-2 | Valley 2-4 | Burn | Erosion | Lakes | Relief 2x2 / 10x10 | Edge |
|---|---|---|---|---|---|---|---|---|
| R1h | 0.00 | 6.47 | 21.80 | 0.00 | 22.86 | 0.00 | 121 % / 113 % | 41 of 44 |
| R3a | 0.00 | 0.07 | 0.62 | 0.00 | 1.15 | 0.00 | 117 % / 102 % | 41 of 84 |
| R3b | 0.00 | 0.07 | 2.64 | 0.00 | 8.41 | 0.00 | 123 % / 115 % | 70 of 93 |

- Schott fair drains every window fully.
- Its valley 1-2 on R1h (6.47 %) is between the round's (5.29 %) and D4 + round's (9.95 %), so it still exceeds Earth at matched relief (0.06-1.82 %, river review).
- It has the best R1h edge of any variant (41 of 44).

## Item 4: the worst-case edit (R1h, a ridge across the main stem of the largest catchment)
- The edit: a ridge of +60 m, 16 km long and 1.2 km wide (3,135 cells), across the stem halfway up. That point has 1,509,694 cells (15,600 km2) upstream.
- The catchment: 5,072,607 L cells (52,569 km2, 7.6 % of the window), with a stem of 4,480 cells.

| Step | Value |
|---|---|
| Dirty set | 1 catchment, 5,072,607 cells; its box 3,074 x 3,270 cells |
| Discovery of the dirty set | the roots before and after in 0.42 s, but the "after" roots need the full re-route: 85.9 s |
| L round over the dirty box | 14.31 s |
| Re-route of the dirty box | 7.9 s |
| Barnes 2016 local part | 2 tiles of 768 cells, 0.26 s (plus the perimeter graph) |
| Fresh solve of the window | full L route 85.9 s; the L round 65-99 s (river review) |
| Peak commit (whole run, round and edit) | 5.93 GB |

- **Discovery dominates:** today's tool finds the dirty set by re-routing the whole window, so a main-stem edit costs a fresh route before any local work starts.
  - An incremental discovery, walking the edit's receivers downstream and its donors upstream on the old routing, is not built.
  - The local work itself is 14.3 s for the round plus 7.9 s for the re-route, against 85.9 s for the full route.
- **Tool limit:** the receivers of the box re-route matched the full re-route on 2,746,184 cells and differed on 2,326,423. The box is built as a fresh grid without the window's origin, so the jitter of the receivers is hashed from other cell ids. The agreement count is not an exactness result. An exact local re-route needs the grid's origin (`Grid::with_origin`), which is not measured here.

## Findings
1. The baked product with the round drains R1h: 0.58 % basins, 0.11 % lakes, edge 34 of 44. K7 lowers the gate's valley 2-4 from 16.50 % to 4.82 % and raises the relief further (136 % at 2 x 2).
2. Which part drains: the breach, everywhere (0.00 % basins alone). The carve alone leaves 15-19 %.
3. Which part incises narrowly: the breach (R1h valley 1-2 8.76 % alone, 5.29 % with the carve). The over-incision of the river review comes from the breach's notches more than from the carve. The carve gives the depth (erosion) and the relief.
4. Schott fair drains fully and matches the best edge (41 of 44), but its R1h valley 1-2 (6.47 %) also exceeds Earth at matched relief.
5. The worst-case edit: the local work is about 22 s against 86 s for a full route. Discovering the dirty set today needs that full route; an incremental discovery is the missing piece.

## Visuals
None: a pure measurement. Spikes' pictures of the variants are under `spikes/river-review/results/images/`.
