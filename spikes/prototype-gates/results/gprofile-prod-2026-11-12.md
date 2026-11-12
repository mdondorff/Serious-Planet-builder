# G-PROFILE production gate run, first tile (R1h, tile (21, 2))

Run 2026-11-12 by the spikes lane on the bulldozer's order. Main b6706dff (GEN-169 final pass and the margin from the halo in). Reported; the verdict is the architect's. TEST-116: production runs report `cutoff_binds`; a non-zero count goes to the architect.

## Result

**`cutoff_binds` is 1086 on the first production tile, and 72 line steps rise over 0.25 m (largest 18.87 m).**

| Quantity | Value |
|---|---|
| Region | R1h: seed 14, face 1, regional cells i 8056..8675, j 760..1379 (12,288 cells a face edge), margin 200, 4 rounds of 5 steps |
| Tile | face 1, (21, 2) of the production grid (98,304 local cells a face edge, tile 3072, halo 128, window 3328) |
| Segments meeting the window | 1,918 (13,659 in the solved window) |
| Line steps with both cells in the core | 59,131 |
| Rises over 0.25 m, K7's own heights | 72, largest 18.8718 m |
| Rises over 0.25 m, stored surface B + L (L in steps of 0.25 m) | 75, largest 18.8044 m |
| Final pass | 11,060 path cells lowered, 5,362 by over 1 m, largest lowering 87.92 m |
| `cutoff_binds` | 1086 |
| Tile-border steps | not measured (one tile run) |
| Time | 905 s for the tile; 490 s for the lattice sample and the regional solve; BelowNormal |

30 segments carry a rise (the probe prints at most 40). By the rise on K7's own heights, the ten largest, with the cell of the largest step:

| Segment ID | Own rise (m) | Stored rise (m) | Cell |
|---|---|---|---|
| 11857176086424103893 | 18.8718 | 18.8044 | (65481, 8535) |
| 10722150568699541066 | 11.5128 | 11.4451 | (66282, 6614) |
| 18217046234410956238 | 8.2231 | 8.4317 | (66582, 6698) |
| 3686018435613417579 | 6.9668 | 7.0559 | (66487, 6839) |
| 1559862561067055109 | 5.9479 | 5.9701 | (65596, 8090) |
| 11723595042664600910 | 5.4725 | 5.5929 | (65601, 7937) |
| 71403741678584392 | 5.1285 | 5.0474 | (66431, 6641) |
| 238581010216145903 | 3.9573 | 4.1251 | (66416, 6648) |
| 4791949282217337836 | 3.8164 | 3.7519 | (66670, 7190) |
| 13534443420299523973 | 3.2049 | 3.1902 | (66884, 6631) |

The other 20 pieces rise 0.10 to 3.04 m. The whole list is in `gprofile-prod/r1h-tile-21_2.txt`. Segment IDs are those of the H1 solve of this window.

## Method

Probe `gprofile_prod.rs` (spike code, local branch, never merged), the pipeline of `k7_region.rs` with the production tile geometry:

- base: the lattice C1 read (lattice rectangle clamped to the face); regional solve of the window with the round rules, f32 rounds, window cut; H1 with runoff and evaporation; every segment a channel (`q` from `qw`);
- `k7_tile` on the tile with the channels near the window (a point within 16 cells), standard K7 parameters, 8 threads;
- piece rises: along `line_cells` of each channel, every step with both cells in the tile core, the surface at the second cell minus the first; the stored surface is `B + round(L / 0.25) * 0.25`;
- `cutoff_binds`: `K7Result.final_pass.cutoff_binds` of the tile.

## Not read

- Whether the 1086 binds lie on the cells of the 72 rises (the deep-fixture test asserts rises at most binds).
- The other 16 tiles: R1h (22, 2), (21, 3), (22, 3); R3a (1..2, 12..13); R3b (25..27, 21..23). About 15 min a tile plus 8 min setup a region (about 4.5 h in all).
- Tile-border steps, the path of the rises through the stored read `S` with `C` on.

## Reproduce

```
cargo build --release -j 4 -p planet-generators --example gprofile_prod   # from the probe branch
PLANET_REGION=14,1,8056,760,620 PLANET_TILES=21,2 gprofile_prod 200 8
PLANET_DRY=1 lists the tiles of a region without running
```
