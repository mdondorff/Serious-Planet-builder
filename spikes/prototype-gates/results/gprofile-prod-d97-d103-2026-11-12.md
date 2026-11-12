# G-PROFILE after GEN-176: where the sub-sea line cells and the far pits come from (D97, D102, D103)

Run 2026-11-12 by the spikes lane on the architect's orders D97, D102 and D103. Main e129414d (GEN-176 in); the tiles and settings of `gprofile-prod-gen176-2026-11-12.md` (R1h (21, 2), R3a (1, 12), R3b (26, 22); tile 3072, halo 128, margin 200 regional cells, 4 rounds, `pre_breach` None). Reported; no verdict, no K7 or library change. Probe: `gprofile_prod.rs` with probe-local copies of K7's per-cell cutoff data and of the round solver's per-round heights (the breach alone on a copy, then the whole round); spike code on local branches, never merged. Raw output: `gprofile-prod/d97/` (`d97_*` the D97 read, `d102_*` the undershoot and pit read with the plateau histogram, `d103_*` the per-round split and the local detail, `d103b_r3b` the per-cell trace of the 5.001 m cells).

## 1. Origin and connection of the sub-sea mid-line cells (D97)

A mid-line cell is a line cell that is not its line's last cell; sub-sea: start height (B + regional change at the cell, before the burn) at or under 0 m. Components are 8-connected cells at or under 0 m of the start surface in the 3,328-cell window; the sea is the continent stage's land depth at or under 0 (`continent_land_depth`).

| Quantity | R1h | R3a | R3b |
|---|---|---|---|
| Sub-sea mid-line cells in the core | 1,983 | 7,502 | 1,037 |
| B itself at or under 0 m (z0) | 62 (3.1 %) | 71 (0.9 %) | 30 (2.9 %) |
| B above 0 m, lowered under 0 m by the regional change (rounds) | 1,921 (96.9 %) | 7,431 (99.1 %) | 1,007 (97.1 %) |
| Local components holding such a cell; reach the sea; window border only; closed inland | 277; 1; 0; 276 | 947; 4; 0; 943 | 139; 1; 0; 138 |
| Cells connected to the sea / closed inland | 69 / 1,914 | 211 / 7,291 | 41 / 996 |
| Closed inland: regional fill depth p50 / p99 / max (m) | 0.00 / 0.29 / 0.67 | 0.02 / 0.82 / 2.60 | 0.00 / 0.07 / 1.03 |
| Rounds-origin cells whose regional cell is in a kept basin / in a lake's water | 3 / 3 | 138 / 194 | 1 / 1 |
| Rounds-origin cells in no basin: regional pit under 0.5 m or none | 1,905 | 7,284 | 1,003 |
| Lakes / kept basins of the regional hydrology | 69 / 69 | 353 / 353 | 81 / 81 |

Reading: 96 to 97 % of the sub-sea cells are in closed inland components, not connected to the sea; 97 to 99 % were lowered under 0 m by the regional rounds; almost none lies in a kept basin or a lake (H1 keeps a basin at a fill depth over 0.5 m; the cells' regional fill depth is mostly 0).

## 2. The rises without a sub-sea start cell, and the cutoffs far from sub-sea stretches (D97)

| Quantity | R1h | R3a | R3b |
|---|---|---|---|
| Rises over 0.25 m with no start cell at or under 0 m on either side | 8 | 12 | 33 |
| by mechanism: carry cutoff within 3 steps / burn column / post-burn fill / other | 7 / 1 / 0 / 0 | 11 / 0 / 0 / 1 | 32 / 1 / 0 / 0 |
| Final-pass cutoffs in the core; of them on land line cells at least 10 steps from any sub-sea stretch (of such cells) | 4,363; 2,358 (53,211) | 11,380; 6,597 (88,551) | 6,177; 5,581 (79,994) |
| Share of the final-pass cutoffs far from sub-sea stretches | 54 % | 58 % | 90 % |
| Far final-pass cutoffs: bounded minus unbounded p50 / p99 / max (m); over 0.25 m | 9.91 / 11.53 / 13.11; 2,096 | 0.49 / 1.66 / 2.10; 5,502 | 1.52 / 3.63 / 3.74; 4,985 |
| Far burn cutoffs (carry 112): count; p50 / p99 / max (m); over 0.25 m | 1,584; 0.86 / 8.42 / 8.42; 1,308 | 4,895; 0.33 / 2.39 / 2.62; 3,035 | 6,888; 1.77 / 4.03 / 4.70; 6,153 |

Distance is over the channel tree (along the lines and across the shared end cells).

## 3. The C1 read and the unreached pits (D102)

- **C1 read undershoot: none.** The C1 read of the regional change is below the lowest change of the four surrounding regional cells by more than 0.25 m for 0 of 1,983 / 7,502 / 1,037 sub-sea cells (all within the range of the four). With the read clamped to the four cells the start height is still at or under 0 m for 100 % of them: the read is not the source.
- **The unreached pits** (the lowest pre-final-pass height over the whole upstream tree of each far final-pass cutoff cell; the pit finder agrees with the pass's own unbounded targets on every node): few and sub-sea.

| Quantity | R1h | R3a | R3b |
|---|---|---|---|
| Distinct pits under the far cutoff cells | 10 | 35 | 13 |
| Pits with a start height at or under 0 m | 10 | 32 | 6 |
| Pits where the start height is below the lowest height of the four surrounding regional cells by more than 0.25 m | 10 (100 %) | 30 (85.7 %) | 9 (69.2 %) |
| Far cutoff cells whose pit is such a cell | 2,358 (100 %) | 5,819 (88.2 %) | 4,743 (85.0 %) |
| Pits on a change-read undershoot cell | 0 | 0 | 0 |

Largest pits by the far cutoff cells that hang on them: R1h (65609, 7977), segment 616159974087397798, start -10.67 m, 1,798 cells; R3a (5128, 38472), segment 5847207040131885285, start -1.36 m, 971; R3b (80460, 68876), segment 1587848788436153174, start 4.46 m and 0.15 m after the burn, 2,161.

## 4. The regional lowering by mechanism (D103)

The round solver's heights per round: before the round, after its breach alone (a copy run with `steps` 0), and after the round (breach and sweeps). Lowering sums are over distinct regional cells, in metres summed; the sweeps' negative values are rises (the pit uplift).

| Regional cells under | R1h | R3a | R3b |
|---|---|---|---|
| sub-sea mid-line cells: cells | 717 | 2,486 | 356 |
| round 1: breach / sweeps | 39,570 / 26,378 | 8,975 / 1,668 | 1,921 / 919 |
| rounds 2 to 4: breach / sweeps | 4,965 / 8,781 | 902 / -181 | 148 / -6 |
| breach share of the lowering; round 1's breach alone | 55.9 %; 49.7 % | 86.9 %; 79.0 % | 69.4 %; 64.4 % |
| round 1's breach share of a cell's lowering, p50 | 0.78 | 1.00 | 1.00 |
| far pits: cells | 10 | 35 | 13 |
| far pits: breach share; round 1's breach alone | 63.4 %; 54.4 % | 102.7 %; 86.9 % | 96.0 %; 91.0 % |
| reference, every line cell of the core: cells | 9,102 | 16,560 | 12,799 |
| reference: breach share; round 1's breach alone | 34.1 %; 27.4 % | 29.9 %; 26.2 % | 34.0 %; 32.1 % |

Round 1's breach is most of the lowering under the sub-sea stretches and the pits (50 to 91 %), against about 30 % under line cells in general, where the sweeps dominate rounds 2 to 4.

## 5. The 5.001 m plateau (D103)

After the rounds, the most frequent regional height among the land cells lowered by more than 50 m is 5.001 m: 4,369 cells (R3b), 1,666 (R3a), 1,059 (R1h); the next most frequent heights hold 4 to 8 cells.

Per-cell trace (R3b, cells (10376..10385, 7905..7914); `d103b_r3b.txt`): before the round / after the breach / after the sweeps, every round: 5.001 / 0.001 / 5.001 (first round for two of them 302.207 / 0.001 / 5.001, and 302.825 / 302.825 / 223.926 then 223.926 / 0.001 / 5.001). The cells are in kept basins (H1 label), their receivers at 5.001 or at a ridge cell of 301.8 m.

Rule, from the code (`crates/generators/src/bake/round.rs`, `catchment_round`, main line numbers):
- the breach (lines 55 to 69): `want = (z[c] - breach_slope * dist[c]).max(floor)` (line 61), `z[rec] = want.max(z[rec] - breach_limit(p, z[rec])).max(start - breach_limit(p, start))` (line 67), `floor = sea_level + epsilon` = 0.001 m (line 47): the cell is lowered to the floor at the start of each round;
- the pit sweep (line 82): `if p.keep_pits && !p.incise_only && z[c] < hr { (z[c] + up).max(floor) }` with `up = up_unit[c] * taper` (line 80), `up_unit` = `u_over_k * k_dt` = 20 x 0.05 = 1.0 m (lines 171 to 173; read from the code, the 1.0 not printed) and taper 1 (a cell that started the solve above 60 m, `uplift_taper_m` 60): a pit cell gains only its uplift, 5 sweep steps (`reroute_every` 5) x 1.0 m = 5.000 m, so 0.001 + 5.000 = 5.001.

A limit cycle: the breach drops the cell to the floor each round and the pit rule lifts it exactly 5 m. The R3b far pits end the rounds at 5.00 m (p50 = p95).

## 6. Local detail against the erosion depth (D103)

Local cells under the sub-sea mid-line cells (the far pits in brackets); `B` the local base at the cell, `zb` the regional base, erosion depth `zb - zs`.

| Quantity | R1h | R3a | R3b |
|---|---|---|---|
| abs(B - zb) p50 / p95 (m) | 0.40 / 17.87 [1.47 / 19.54] | 0.10 / 0.93 [1.35 / 2.46] | 0.10 / 0.97 [0.54 / 1.99] |
| B range inside the 8 x 8 local cells of the regional cell, p50 / p95 (m) | 1.79 / 76.22 [5.33 / 37.88] | 0.65 / 3.14 [3.66 / 6.16] | 0.55 / 3.11 [3.05 / 5.18] |
| erosion depth p50 / p95 (m) | 6.0 / 631 [18.6 / 372] | 2.7 / 11.8 [17.4 / 25.6] | 3.1 / 16.1 [272.7 / 425.0] |
| abs(B - zb) over the erosion depth, p50 / p95 | 0.046 / 0.41 [0.059 / 0.115] | 0.051 / 0.19 [0.086 / 0.145] | 0.045 / 0.21 [0.004 / 0.075] |
| Local cells whose B is below zb by more than the regional height after the rounds | 1,189 of 1,983 | 4,495 of 7,502 | 559 of 1,037 |
| Regional height after the rounds, p50 / p95 (m) | 0.14 / 9.86 | 0.04 / 0.70 | 0.04 / 0.69 |

Reading: the local detail is metres (p50 0.1 to 0.4 m, p95 1 to 18 m) on regional cells the rounds pushed to 0 to 1 m (the sub-sea stretches) or by 20 to 430 m (the pits); about 60 % of the sub-sea cells go under 0 m by arithmetic: B below the regional base by more than the regional height.

## Not read

- Why the breach cascade ends at the floor (the donors at the floor, the cascade length); whether the 5.001 cells are in kept basins on R1h and R3a (the R3b cells are); the stored read `S`; tile borders; one tile per region, one seed per region.
- The two-tiling difference with line cells fixed (queued).
