# K7 two-tiling: the 0.31 m cell traced to its step

Run 2026-11-12 by the spikes lane (BelowNormal, one process, about 10 min). Read-only measurement, no verdict, no change in K7. Order: the bulldozer (the K7 continuity work list, ADR 0119; due 2026-11-14, before the G-SEAM reading).

## What was traced
- The one cell over 0.25 m in `k7-two-tiling-2026-11-12.md` (halo 128): local cell (66941, 8313) of face 1 (core cell (381, 121) of tile (65, 8), 1,024 cells at halo 128), 130 cells from the seam of the second tiling (outside the halo's reach), 1 cell from a line cell, not itself a line cell. Height 1.8268 m under the one 1,024-cell tile against 2.1354 m under the mosaic of the four 512-cell tiles (diff 0.3085 m).
- Same fixture and probe as that note (the local branch `spike/k7-two-tiling`, `PLANET_SEAM=65,8`, `PLANET_HALO=128`), plus `PLANET_SEAM_ONLY` (K7 steps switched off as in the 2026-11-11 read: `k7-seam-2026-11-11.md` section 3) and `PLANET_CELL=66941,8313`. Raw: `k7-cell-trace/seam-variants.txt`, the example in `k7-cell-trace/gprofile_prod.rs.txt`.

## Result: the step is the incising carve; deposition adds the last 0.10 m
| K7 variant (halo 128) | Cells differing in bits (of 1,048,576) | Over 0.25 m | Max diff (m) | The cell: tile A / mosaic / diff (m) | Line cells differing |
|---|---|---|---|---|---|
| Full K7 | 5,905 | 1 | 0.3085 | 1.8268 / 2.1354 / 0.3085 | 0 of 8,548 |
| Thermal off | 5,905 | 1 | 0.3085 | 1.8268 / 2.1354 / 0.3085 | 0 |
| Pit spreading off | 5,905 | 1 | 0.3085 | 1.8268 / 2.1354 / 0.3085 | 0 |
| Deposition off | 5,807 | 0 | 0.2057 | 1.5603 / 1.7660 / 0.2057 | 0 |
| Burn, breach and incision (thermal, deposition, pits off) | 5,807 | 0 | 0.2057 | 1.5603 / 1.7660 / 0.2057 | 0 |
| Burn and breach (one step, `k_dt` 0) | **0** | 0 | 0 | 2.9571 / 2.9571 / 0 | 0 |
| Burn only | **0** | 0 | 0 | 2.9571 / 2.9571 / 0 | 0 |
- The burn and the breach are bit-identical under the two tilings (the cell is at 2.9571 m in both); the difference opens with the incising carve (`k_dt` on, 5 steps): 5,807 cells differ and the cell is 0.2057 m apart (1.5603 against 1.7660 m).
- The deposition sweep adds 98 cells and 0.10 m at this cell (0.2057 -> 0.3085 m) and makes it the only cell over 0.25 m; thermal and the pit spreading change nothing at this cell or in the count.
- Under the incision the cell falls from 2.9571 m (burn and breach) to 1.5603 m (A) and 1.7660 m (B): the two windows incise it by 1.397 and 1.191 m. The deposition sweep then raises it by 0.267 m (A) and 0.369 m (B), to 1.8268 and 2.1354 m. The 2026-11-11 read found the same step (the incision) at halo 128 for the 7,054 cells then.
- Distance bands (all cells, halo 128): the full-K7 counts per band are 1,102 / 1,398 / 2,893 / 512 for 0 to 7 / 8 to 31 / 32 to 127 / 128 and more cells from the seam; with the deposition off 1,109 / 1,328 / 2,862 / 508: the same shape, so the incision carries the whole pattern.

## Reading (no verdict)
- The cell is 130 cells from the seam and the halo is 128, so the difference there cannot come from a halo cut at the seam alone; the cell is next to a line cell (a bank), where the incision depth follows the drainage area and the slope of the line it drains to. The area accumulated inside a window differs between the 1,024-cell window (1,280 cells) and the 512-cell one (768 cells) wherever the upstream path of the cell leaves the smaller window; the 2026-11-11 note gave the same guess. It is not proven here: the probe did not read the area of the cell under the two windows.
- Line cells are fixed after the burn (GEN-176) and are identical in every variant; the differing cells are bank and slope cells under the incision.
- Not done: the area and slope of the cell under the two windows (K7 would need an area dump), other tiles and halos, the regional solve of a later main (this run is on main e129414d, GEN-176).

## Reproduce
```
cargo build --release -j 4 -p planet-generators --example gprofile_prod     # branch spike/k7-two-tiling (local)
PLANET_SEAM=65,8 PLANET_HALO=128 PLANET_SEAM_ONLY=0,1,2,3,4,5,6 PLANET_CELL=66941,8313 gprofile_prod 200 8
```
Variants: 0 full K7, 1 thermal off, 2 deposition off, 3 pit spreading off, 4 burn+breach+incision, 5 burn and breach (one step, `k_dt` 0), 6 burn only. A run is about 10 min when quiet (the regional solve of 2 min, then 7 variants of 1 min).
