# G-PROFILE production run: the rises against the cutoffs, the worst chains, and tiles of R3a and R3b

Run 2026-11-12 by the spikes lane on the architect's order (steps 1 to 3 of the first-tile finding). Main 98d7610c (GEN-169 final pass and the halo margin in). Reported; the verdicts are the architect's. Builds on `gprofile-prod-2026-11-12.md` (first tile). Probe `gprofile_prod.rs` with a probe-local copy of the final pass's inputs (heights per stage, per-cell target and unbounded carry), spike code on a local branch, no K7 change. Release build, 8 threads, BelowNormal, margin 200 regional cells, 4 rounds. Raw output: `gprofile-prod/`.

## Result

**The bounded carry is not the main mechanism on R1h: 71 of 72 rises have the upper cell at or under the sea level before the final pass.** The final pass gives such a cell no value (it neither carries nor is lowered; `final_channel_pass`, GEN-169), while the burn digs the trench below 0 m on land lines. Rises over 0.25 m are in sub-sea trenches and between a trench cell and the next land cell.

| Quantity | R1h (21, 2) | R3a (1, 12) | R3b (26, 22) |
|---|---|---|---|
| Seed, face | 14, 1 | 15, 4 | 15, 2 |
| Line steps in the core | 59,131 | 111,646 | 81,000 |
| Rises over 0.25 m, own heights (largest) | 72 (18.87 m) | 29 (0.556 m) | 13 (1.381 m) |
| Rises over 0.25 m, stored B + L (largest) | 75 (18.80 m) | 39 (0.608 m) | 17 (1.204 m) |
| `cutoff_binds` (cutoff cells in the core) | 1,086 (1,082) | 2,964 (2,775) | 5,966 (5,833) |
| Rises with the lower cell a cutoff | 6 | 1 | 6 |
| Rises with a cutoff cell within 3 steps (not at the lower cell) | 4 | 2 | 0 |
| Rises with no cutoff within 3 steps | 62 | 26 | 7 |
| Upper cell at or under 0 m before the final pass | 71 | 28 | 6 |
| Lower cell at or under 0 m before the final pass | 57 | 23 | 4 |
| No cutoff at either cell, upper cell above 0 m | 0 | 0 | 2 |
| Final pass: lowered / over 1 m / largest | 11,060 / 5,362 / 87.9 m | 20,873 / 726 / 7.7 m | 14,837 / 3,487 / 20.8 m |
| Time (tile; lattice and solve) | 117 s; 46 s | 106 s; 34 s | 73 s; 530 s (other load) |

Tiles: production grid 98,304 local cells a face edge, tile 3072, halo 128, carry of the final pass 128 steps, of the burn 112. Regions: R1h (cells 8056..8675, 760..1379 of 12,288), R3a (168..1187, 4520..5539), R3b (9768..10787, 8104..9123). One tile each; the tile borders are not measured (no neighbour tile run).

## Step 1: rises against cutoffs

A rise is a line step along a segment with both cells in the core where the final height rises downstream by more than 0.25 m. A cutoff cell is a land path cell whose target differs from the unbounded carry (what `cutoff_binds` counts).

- R1h: 6 of 72 rises at a cutoff cell, 4 within 3 steps, 62 with none within 3 (52 with no cutoff cell on the line at all). No rise has a cutoff at the upper cell only.
- The sea-level rule is the common feature: 66 of the 66 rises with no cutoff at either cell have the upper cell at or under 0 m before the final pass (burn levels -0.5 to -26.5 m, start surface -3.8 to +20 m at those cells). Before the final pass 10 of the 72 steps rise by at least 7 m (5 by at least 17 m, up to 30.4 m); the final pass lowers the lower cell and cuts the rise in 7 of them (for example 30.42 to 11.51 m), the other 3 are unchanged (18.87 m; 8.22 and 7.59 m in the burn column). The message to the architect said "17-30 m in at least 7 rises"; the counts here are the measured ones.
- R3a: 28 of 29 rises have the upper cell at or under 0 m (coastal flats, burn levels about -1.3 to 0 m); the largest rise is 0.556 m.
- R3b: 13 rises (largest 1.38 m), 6 at a cutoff cell, 6 with the upper cell under 0 m; 2 rises have neither: pits 1 step upstream at 10.5 and 12.9 m (burn then carve lowered the pit 4 m; the lower cell is 1 m above).
- `cutoff_binds` is large on R3a and R3b (2,964 and 5,966) with few rises (29, 13): most binds are cells whose carry differs by a small amount.

## Step 2: the three worst pieces of R1h

| Piece (segment ID) | Rise | Pit upstream | Pit height | Lower cell, start / burn / before the final pass | Reading |
|---|---|---|---|---|---|
| 11857176086424103893, line 362 | 18.87 m | 160 steps (carry 128; burn carry 112), cell (65473, 8375) | 0.001 m (start 0.397 m) | 97.22 / -2.42 / 16.45 m | cutoff; pit is a flat coastal cell, not a deep start-surface basin; the 161-cell chain is one burn trench at -1 to -2.4 m in terrain up to 128.8 m; the later stages raised the trench cell from -2.42 to 16.45 m, and the final pass cannot lower it (all upstream cells within 128 steps are at or under 0 m, so carry nothing) |
| 10722150568699541066, line 1 | 11.51 m | 3 steps, segment 2703563052782080745 cell (66285, 6611) | 0.001 m (start 17.9, burn 9.97) | 137.09 / -11.51 / 18.91 m | no cutoff (target = unbounded = 0.001); the later stages raised the lower cell by 30 m, the final pass lowered it to 0.001 m, the lowest land value; the upper cell stays at -11.51 m |
| 18217046234410956238, line 29 | 8.22 m | 37 steps, segment 13073305064277476742 cell (66595, 6661) | 0.001 m (start 0.99) | -0.55 / -6.11 / -6.11 m | no cutoff; the rise is in the burn column (line 27 to 30: -26.48, -21.92, -14.33, -6.11, -5.14 m) and no later stage changes it; the source of the -26 m cells is not traced |

- Pit depth against the start surface: the pits of the three are the carve's flat cells at 0.001 m (start 0.4, 0.99, 17.9 m); none is a deep basin of the start surface. The pits of the R3a worst pieces are 146 to 740 steps upstream, coastal cells at 0.000 to 0.001 m (start 0.00 to 0.27 m), the chains sub-sea at -0.3 to -1.3 m.
- A burn trench below its downstream terrain appears on the R1h pieces: lower cells with start heights of 97 to 137 m beside burn levels of -2 to -12 m; the later stages raise such cells (up to 30.4 m, see step 1), and the final pass lowers some of them again (to the lowest land value) but not the sub-sea cells beside them.

## Not read

- The source cells of the -26 m burn levels (burn carry bound on sub-sea cells; not counted by `cutoff_binds`); whether the burn levels below 0 m are meant (levels are the running minimum of the regional heights and of the local start heights).
- Tile-border steps (one tile per region); the other tiles; the stored read `S`; GEN-174's round-0 breach on (the rises are at the coast and in trenches, away from the deep basins).
- R3a's regional window is cut 16 regional cells in from the face edge (the lattice read needs neighbour-face nodes there); the tile (1, 12) lies 352 regional cells in.

## Reproduce

```
PLANET_REGION=14,1,8056,760,620 PLANET_TILES=21,2 gprofile_prod 200 8
PLANET_REGION=15,4,168,4520,1020 PLANET_TILES=1,12 gprofile_prod 200 8
PLANET_REGION=15,2,9768,8104,1020 PLANET_TILES=26,22 gprofile_prod 200 8
```
