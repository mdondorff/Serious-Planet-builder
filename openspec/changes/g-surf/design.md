## Context
- A3 sections 3.1 to 3.3 and briefing item 11 (inbox #104): the local pass K7 reads the base through the store lattice's C1 read at every cell (the stride-4 window of pass 2 is retired), starts from `B + C1(regional change)`, burns the channels of K6 into it, runs the breach of pass 1 at the local grid, then pass 2's carve, thermal, deposition and pit spreading; `L = result - B` is stored in steps of 0.25 m. G-SURF: the runtime's `B + L` against the bake's surface at the lattice nodes, 0 before quantisation and at most 0.25 m after. "This is the hand-over that caused the original pits": the pit share and depth at 102 m with the breach and channels in are reported.

## Decisions
1. `k7_tile(g, id, base, regional, channels, params, threads)` follows `pass2_tile_rock` with: `B` sampled at every cell of the window; the regional change from `RegionField::at` (the Catmull-Rom read of a window of the regional grid, clamped to the four cells around the point as `map::change_in_face` is); the burn-in; the local carve with pass 1's breach (500 m limit, 0.1 % slope; pass 2's own carve has no breach); no rock and no class factor.
2. `burn_channels`: along each segment, between consecutive points, the cells of the 8-connected line (steps of one cell along the longer axis, rounded to the nearest cell) are lowered to the level interpolated linearly along the segment; a cell is never raised. The levels are the running minimum of the regional solved bed along the segment's D8 path, so they do not rise downstream.
3. A channel segment covers its path to the window's border: a trench that ends inside the window is a sink (tier A: a trench ending 8 cells short of the border is a 60 m pit).
4. `pit_stats`: land cells whose fill depth reaches 0.5 m on the window grid (outlets: the sea and the window's border), the share of the land and the 50th, 99th percentile and maximum depth.
5. `surface_gate`: lattice nodes are the centres of the lattice's cells (the base lattice is sampled at the centre of its 16,384-cell grid: a corner of the local grid, 3 local cells past a multiple of 6). Runtime `B(node) + C1(L)(node)` against bake `C1(z)(node)`, with `L = z - B` as stored (steps of 0.25 m) and unquantised; also at the centres of the core's cells.
6. The prototype's lattice is sampled from `BakeInputs::base` at the nodes (f32), read with `read_elevation_c1`; the regional solve starts from the same read, so the whole chain reads one `B`.

## Measurements (`examples/k7_region`, release, 8 threads; the tile (65, 7) of face 1: 1,152 x 1,152 local cells, 117 km, with the trunk of region 1 of results.md in it; the regional window of 1,020 x 1,020 cells, 4 rounds, H1 on the solved surface: 12,101 segments, 250 of them meet the window)
Pits at 102 m (land cells whose fill depth is at least 0.5 m; 1.31e6 land cells):
| Stage | With the channels burned in: pit cells (share), depth p50 / p99 / max | Without the channels |
|---|---|---|
| `B + C1(regional change)` (the hand-over) | 77,857 (5.93 %), 11.2 / 87.8 / 227.9 m | the same |
| the channels burned in (4,518 cells lowered) | 24,858 (1.89 %), 4.7 / 108.3 / 227.9 m | the same as the start |
| the breach of pass 1 alone | 551 (0.042 %), 1.3 / 34.8 / 38.4 m | 998 (0.076 %), 2.9 / 68.6 / 69.4 m |
| the breach and the incision | 1,357 (0.103 %), 1.3 / 34.5 / 37.5 m | 2,205 (0.168 %), 2.2 / 66.1 / 67.1 m |
| the whole pass (thermal, deposition, spreading) | 661 (0.050 %), 4.1 / 59.5 / 62.9 m | 1,061 (0.081 %), 5.6 / 61.1 / 62.9 m |
- The hand-over leaves 5.9 % of the land in pits; the breach removes 99 % of them; the burn-in halves them again after the breach. The deep tail remains: the maximum depth after the whole pass is 62.9 m (with the channels and without), above the 37.5 m after the incision: thermal, deposition or the pit spreading deepen the worst pits (item 10).
- The stored change `L` against `B` runs from -789.3 m to +14.8 m; none is over 8 km (the `i16` range at 0.25 m).
- K7 takes 4.2 s on the tile.

**G-SURF (the finding).**
| Where | Unquantised | With `L` in steps of 0.25 m |
|---|---|---|
| the centres of the core's cells | 5.7e-14 m | 0.1250 m (half a step) |
| 28,900 lattice nodes in the core, with the channels | max 1.852 m | max 1.890 m; p50 0.0325, p99 0.1177 m; 37 nodes (0.13 %) over 0.25 m |
| 28,900 lattice nodes, without the channels | max 2.258 m | max 2.268 m; p50 0.0324, p99 0.1196 m; 48 nodes (0.17 %) over 0.25 m |
- At the cells' centres the gate holds: the C1 read returns the stored value, so the runtime's `B + L` is the bake's surface, and the quantisation costs half a step.
- At the lattice nodes the gate does not hold as stated: 0.13 % of the nodes differ by more than 0.25 m, up to 1.9 m. Two causes were measured or are evident. (a) The clamp of the C1 read (the value kept within the four cells around the point) is not linear: `B(node) + clamp(C1(L))` is not `clamp(C1(B + L))` where the surface has an extremum, a valley floor or a ridge, and `L` is rough (down to -789 m beside the channels). (b) `B` read again from its values at the local cells' centres differs from `B` itself by up to 0.179 m (the lattice read at 611 m resampled to 102 m, then clamped again).
- "0 before quantisation" is therefore true at the centres and not at the nodes. The tier-A smooth tile gives 0.07 m before and 0.15 m after quantisation (the bound of 0.25 m holds there).

## Not done
- The store's `L` (kind 13) and its i16 tile format are not written: the quantisation is applied in the gate only. The lattice is sampled in memory, not read from a store.
- G-PROFILE (the channel after the sampler: no rise above 0.25 m along a segment at the local nodes, continuity across tile borders, a confluence's floors meeting) is not measured.
- The burn-in uses the regional solved bed as the profile; K6 does not yet give a level profile. The channel continuity across tile borders is not exercised (one tile).
- The creases `C` and the items `A` of the composition are not in the surface read.
- Item 10 (thermal, deposition and the pit spreading on the cube grid, G-SED) and the partitioned route on the compact grid are the next items.

## Earth-specific values (ADR 0108 point 4)
| Value | Earth (example) | Unit, source | Parameter |
|---|---|---|---|
| Lattice, regional, local grids | 16,384, 12,288, 98,304 cells a face edge (611 m, 814 m, 102 m) | cells; A3 section 4.1 | the example `K`, `REGIONAL`, `LOCAL` |
| Tile and halo | 1,024 and 64 local cells | cells; calibration | the example `TileGrid` |
| Tile position | (65, 7) of face 1 | the trunk of region 1 (results.md E4) | the example `TileId` |
| Regional window | region 1 plus 200 regional cells | the example `margin` | argument |
| Local cells to a regional cell, to a lattice cell | 8, 6 | A3 section 4.1 | the nesting of the grids |
| Pit depth | 0.5 m | A3 section 5.3 | `Pass2Params::pit_min_m` |
| Quantisation step | 0.25 m | A3 section 3.1 | `QUANT_STEP_M` |
| Breach | 500 m limit, 0.1 % slope | pass 1 (GEN-100) | `CarveParams::default()` |
| Local carve cap | 96 cells of area | pass 2 (GEN-101) | `Pass2Params::local_area_cells` |
| Radius, seed, sea level | 6,371,000 m, 14, 0 m | Earth state; world definition | the example |
