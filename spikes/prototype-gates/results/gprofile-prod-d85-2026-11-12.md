# G-PROFILE production run: the D85 mechanism classing and the cutoff difference distribution (before GEN-176)

Run 2026-11-12 by the spikes lane on the bulldozer's order (D85 classing; D87 criterion: bounded minus unbounded target per cutoff). Same probe, tiles and main as `gprofile-prod-d84-2026-11-12.md` (probe code on the main of b6706dff plus the GEN-169 final pass; GEN-174 and GEN-176 not in). Reported; no verdict. Raw output: `gprofile-prod/r1h-d85.txt`, `r3a-d85.txt`, `r3b-d85.txt` (every rise with the heights of both cells after each stage, the flags, the cutoffs over 0.25 m).

## Classing of the rises over 0.25 m (D85)

Flags per rise (a line step with both cells in the core whose final height rises downstream by more than 0.25 m):

- S: the upper cell is at or under the sea level (0 m) before the final pass.
- F (post-burn fill): the lower cell is raised by more than 0.25 m between the incising carve and the final pass (thermal, deposition, pit spreading).
- C (carry cutoff, final pass): a cutoff cell (bounded target differs from the unbounded) is the lower or the upper cell, or lies within 3 steps of the lower cell on the line.
- B (carry cutoff, burn): the rise is already in the heights at the end of the burn (more than 0.25 m).

Exclusive classes, in the order S, F, then C or B, then other (each rise counted in the first class it has):

| Tile | Rises | S | F | C or B | other |
|---|---|---|---|---|---|
| R1h (21, 2), face 1, seed 14 | 72 | 71 | 0 | 1 | 0 |
| R3a (1, 12), face 4, seed 15 | 29 | 28 | 1 | 0 | 0 |
| R3b (26, 22), face 2, seed 15 | 13 | 6 | 5 | 2 | 0 |

With overlaps (a rise counts in every class it has) and the flag combinations (S F C B):

| Tile | S | F | C | B | Combinations |
|---|---|---|---|---|---|
| R1h | 71 | 23 | 10 | 51 | S--B 42, SF-- 19, S-CB 6, SFCB 2, SF-B 1, SFC- 1, --C- 1 |
| R3a | 28 | 1 | 3 | 28 | S--B 26, S-CB 2, -FC- 1 |
| R3b | 6 | 5 | 6 | 8 | S--B 5, -FC- 5, ---B 2, S-CB 1 |

- 51 of the 72 R1h rises (28 of 29 on R3a, 8 of 13 on R3b) are already in the heights at the end of the burn; all 51 R1h and all 28 R3a rises with the B flag also have the S flag (6 of 8 on R3b).
- R3b is the tile where S is not the whole: 5 rises carry F and C and not S or B (`-FC-`), 2 carry B only (`---B`).

## The cutoff difference (D87)

For every cutoff cell in the tile core, the bounded carry target minus the unbounded target (metres; the target is the lowest height within the carry along the channel tree). Final pass: carry 128, land path cells (a cell at or under 0 m has no value); burn: carry 112, every path cell in the core, the probe's own copy of the burn's target computation (heights as the burn finds them, no sea-level exclusion).

| Quantity | R1h (21, 2) | R3a (1, 12) | R3b (26, 22) |
|---|---|---|---|
| Final pass: cutoff cells in the core (`cutoff_binds` of the tile) | 1,082 (1,086) | 2,775 (2,964) | 5,833 (5,966) |
| Final pass: difference p50 / p99 / max | 0.0011 / 1.848 / 16.45 m | 0.0011 / 0.199 / 0.255 m | 0.0005 / 1.405 / 1.569 m |
| Final pass: cutoffs over 0.25 m | 20 | 8 | 1,159 |
| Burn: cutoff cells in the core | 5,928 | 15,544 | 8,258 |
| Burn: difference p50 / p99 / max | 6.59 / 22.15 / 24.71 m | 0.527 / 2.096 / 2.615 m | 1.681 / 4.231 / 5.274 m |
| Burn: cutoffs over 0.25 m | 5,402 | 12,097 | 7,283 |

Final-pass cutoffs over 0.25 m (the tile and the cells; each run is consecutive cells of one segment):

- R1h: segment 11857176086424103893, line indices 363 to 365, cells (65481, 8535), (65480, 8536), (65479, 8537): bounded 16.451, 13.364, 3.871 m, unbounded 0.001 m (differences 16.45, 13.36, 3.87 m). Segment 17629034109215487030, line indices 41 to 48 and following, cells (65597, 7805) to (65604, 7812): bounded 1.850, unbounded 0.002 (1.848 m). 14 more cells of the 20; the full list is the raw output (15 printed).
- R3a: segment 1947946260677881028, line indices 63 to 70, cells (5388, 37947) to (5388, 37954): bounded 0.283, unbounded 0.027 (0.255 m, 0.005 m over the step).
- R3b: 1,159 cells; the largest: segment 8820796763774010825, line indices 214 onward, cells (80878, 69034), (80877, 69035) and following: bounded 1.570, unbounded 0.001 (1.569 m). 15 of 1,159 printed in the raw output.

## Reading the numbers

- A final-pass cutoff counts a bind (target differs) however small; the difference is mostly tiny (p50 0.001 m on all three tiles). The cells over 0.25 m are few on R1h (20) and R3a (8), many on R3b (1,159 of 5,833).
- The burn's differences are large (p50 6.6 m on R1h): the unbounded target of the burn is the lowest value over the whole tree upstream, and it lies well below the lowest value within 112 steps for most cells with a bind. Why it lies that far below was not read (the value of a cell is the lower of the segment's level and the start height).
- Not measured: the burn's difference with the sea level floor (GEN-176), the pass criterion as a gate (rises over 0.25 m are listed above and in the D84 note), tile borders (one tile per region).
- The probe's burn target is a copy of `burn_channels_bounded`'s computation (same `tree_carry`, same values) taken inside the function; the unbounded target is `tree_min_unbounded` of the same values.

## Reproduce

Same commands as `gprofile-prod-d84-2026-11-12.md`; the probe branch adds the classing, the difference distribution and the burn probe (local only).
