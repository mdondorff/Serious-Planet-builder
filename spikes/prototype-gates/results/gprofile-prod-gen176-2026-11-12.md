# G-PROFILE production re-run after GEN-176 (fail)

Run 2026-11-12 by the spikes lane on the bulldozer's order (architect D97 context). Main e129414d (GEN-176 merged: K7 beds floored at sea level on land cells, channel cells fixed after the burn, every cutoff reported). Reported; the verdict is the architect's: **the gate does not hold on any of the three tiles, and the rise counts are higher than before GEN-176.** Raw output: `gprofile-prod/gen176-r1h.txt`, `gen176-r3a.txt`, `gen176-r3b.txt` (the K7 MARK line, the criterion lines and the 20 worst rises with the heights of both cells).

## Method

Probe `gprofile_prod.rs` (spike code, local branch, no K7 change; the probe of the earlier notes in this folder, with the report of the refined criterion). K7 on one production tile of each region: 98,304 local cells a face edge, tile 3072, halo 128 (window 3328), margin 200 regional cells, 4 rounds of 5 steps, `pre_breach` None (set explicitly), base from the lattice C1 read, 8 threads, BelowNormal. Rises: line steps along the stored channel lines (`line_cells`) with both cells in the core, downstream height minus upstream height on K7's own heights and on the stored surface B + L (L quantised to 0.25 m). A mid-line cell is a line cell that is not its line's last cell; sub-sea: its start height, B plus the regional change at the cell before the burn, at or under 0 m.

## Result

| Quantity | R1h (21, 2) | R3a (1, 12) | R3b (26, 22) |
|---|---|---|---|
| Seed, face | 14, 1 | 15, 4 | 15, 2 |
| Line steps in the core | 59,107 | 111,646 | 81,000 |
| Rises over 0.25 m, own heights (largest) | **255** (26.50 m) | **351** (1.36 m) | **120** (2.74 m) |
| before GEN-176 (largest) | 72 (18.87 m) | 29 (0.556 m) | 13 (1.38 m) |
| Rises over 0.25 m, stored B + L (largest) | 271 (26.50 m) | 411 (1.35 m) | 122 (2.72 m) |
| Line cells in the core | 59,138 | 111,707 | 81,025 |
| Mid-line cells with a start height at or under 0 m | 1,983 (3.35 %) | 7,502 (6.72 %) | 1,037 (1.28 %) |
| of them with a final height under 0 m | 1,983 | 7,502 | 1,037 |
| Line cells with a final height under 0 m (lowest) | 1,998 (-26.50 m) | 7,535 (-1.36 m) | 1,046 (-1.49 m) |
| Rises touching a mid-line cell that was at or under 0 m before the burn | 247 of 255 (97 %) | 339 of 351 (97 %) | 87 of 120 (73 %) |
| Rises with a cell at or under 0 m after K7 | 248 | 339 | 88 |
| Rises with no sub-sea start cell on either side | 8 | 12 | 33 |
| Burn cutoffs (all path cells), diff p50 / p99 / max (m) | 2,336; 0.88 / 22.17 / 24.72 | 7,332; 0.34 / 2.39 / 2.62 | 7,503; 1.69 / 4.03 / 4.70 |
| Final pass cutoffs (all path cells), diff p50 / p99 / max (m) | 4,418; 9.68 / 11.53 / 20.37 | 12,551; 0.49 / 1.66 / 2.10 | 6,703; 1.30 / 3.63 / 3.74 |
| Final pass: lowered / over 1 m / largest | 1,388 / 615 / 8.42 m | 3,823 / 383 / 2.34 m | 4,215 / 2,195 / 3.77 m |
| Channel cells fixed after the burn; thermal and deposition exported to them | 71,973; 6.8e8 and 1.2e8 m3 | 137,384; 0 and 3.9e7 m3 | 112,968; 1.8e6 and 1.5e8 m3 |
| Time (the K7 tile; the whole run with the lattice and the regional solve) | 169 s; 221 s | 154 s; 224 s | 197 s; 268 s |

Gated criteria (architect's refined criterion): no line step rises over 0.25 m: **fails** (255 / 351 / 120); every cutoff's bounded minus unbounded target at most 0.25 m: **fails** (maxima above). The cutoff counts are reported.

## The rises

Reading from the 20 worst rows per tile (raw output) and the counts above:

- Most rises lie at the entry from a sub-sea stretch into land. The sub-sea mid-line cells are burnt to the carried profile (the lowest value within the carry over the tree, the deep sub-sea cells upstream, -6 to -26 m); the first land cell downstream is floored at 0 m (GEN-176). The step from the trench to the floor is the rise: R1h segment 71403741678584392, upper cell (66530, 6628) start -6.13 m final -26.50 m, lower cell (66531, 6628) start 0.25 m final 0.00 m (26.50 m).
- Worst rises per tile (tile; segment; cells; heights start to final): R1h (21, 2): 71403741678584392, (66530, 6628) to (66531, 6628), 26.50 m; 21.84 m on segments 4791949282217337836 (66676, 7203), 2828062209702778677 (66691, 7197) and 2526942390144646385 (66715, 7188). R3a (1, 12): 5847207040131885285, (5130, 38474) -0.15 to -1.36 m and (5131, 38475) 0.63 to 0.00 m, 1.36 m. R3b (26, 22): 15880625672994133918, (80457, 69996) 8.48 to 0.00 m and (80458, 69996) 8.54 to 2.74 m (no sub-sea start cell), 2.74 m; 15483567781688059444, (80154, 70612) 4.50 to 0.71 m and (80155, 70612) 4.72 to 3.05 m, 2.35 m.
- 8, 12 and 33 rises have no start cell at or under 0 m on either side; not classed here.

## Not read

- Why the sub-sea mid-line cells exist (origin: the base at or under 0 m or lowered by the rounds; connection to the sea; basin or lake): the architect's D97 read, in progress.
- The rises without a sub-sea cell; the cutoff differences away from the sub-sea stretches; tile borders (one tile per region); the stored read `S` with `C` on; the two-tiling difference.
- Single runs, one seed per region.

## Reproduce

```
PLANET_REGION=14,1,8056,760,620 PLANET_TILES=21,2 gprofile_prod 200 8
PLANET_REGION=15,4,168,4520,1020 PLANET_TILES=1,12 gprofile_prod 200 8
PLANET_REGION=15,2,9768,8104,1020 PLANET_TILES=26,22 gprofile_prod 200 8
```
