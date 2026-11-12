# K7 two-tiling difference with the line cells fixed (re-measure of the 2026-11-11 seam read)

Run 2026-11-12 by the spikes lane (BelowNormal, one process, machine at 2 % load). Measurement only, no verdict; no change in K7. Replaces the number of `k7-seam-2026-11-11.md` (0.67 % of the core cells, maximum 0.087 m, halo 128) with the same fixture on the code after GEN-176 (K7 river beds on or above the sea level, channel cells fixed after the burn), line cells and non-line cells apart.

## Fixture and probe
- Tile (65, 8) of face 1, seed 14 (region R1h; its regional window solved from the C1 read of the lattice, margin 200, 4 rounds with `round_f32` and `window_cut` as `gprofile_prod` does, H1: 13,578 segments, 47,951 channel cells). Tiling A: the tile of 1,024 cells at halo `h` (window 1,024 + 2h). Tiling B: the four tiles of 512 cells inside its core at the same halo; the seams of B are the middle of the core. K7 (`k7_tile`, `K7Params::standard`) runs on each window with the channels of the solved window that touch it (as the production run gives them); the core of A is compared with the mosaic of the four cores of B.
- Code: the local branch `spike/k7-two-tiling` (on `spike/gprofile-d97`, based on main e129414d, which has GEN-176; the probe's regional solve leaves the round-0 breach of GEN-174 off, as the G-PROFILE runs do; main has since taken GEN-175 and GEN-178 changes to the rounds' surface and epsilon, which this run does not have). `k7-two-tiling/gprofile_prod.rs.txt` is the example with the new `PLANET_SEAM=ti,tj` mode (also `PLANET_HALO=128,64`, `PLANET_BIG`), `k7-two-tiling/seam.txt` the raw output.
- Line cells = every cell of every channel line of the solved window (`line_cells`) inside the core of A: 8,548 of 1,048,576 (0.82 %).

## Result (halo 128, the production halo)
| Class | Core cells | Differ in bits | Over 0.25 m | Over 1 m | Among those that differ: p50 / p99 / max (m) |
|---|---|---|---|---|---|
| All cells | 1,048,576 | 5,905 (0.56 %) | 1 | 0 | 0.0000 / 0.0011 / 0.3085 |
| Line cells | 8,548 | **0 (0.00 %)** | 0 | 0 | 0 |
| Non-line cells | 1,040,028 | 5,905 (0.57 %) | 1 | 0 | 0.0000 / 0.0011 / 0.3085 |

By distance to the nearest seam of tiling B (cells in the band / differing in bits; all cells, and none of the line cells differ in any band):
| Band (cells from a seam) | Cells | Halo 128 differ (max m) | Halo 64 differ (max m) |
|---|---|---|---|
| 0 to 7 | 32,512 | 1,102 (3.39 %; 0.0029) | 3,650 (11.23 %; 3.5926) |
| 8 to 31 | 94,464 | 1,398 (1.48 %; 0.0019) | 7,140 (7.56 %; 1.8726) |
| 32 to 127 | 331,776 | 2,893 (0.87 %; 0.0341) | 12,430 (3.75 %; 1.2779) |
| 128 and more | 589,824 | 512 (0.09 %; 0.3085) | 749 (0.13 %; 0.0071) |

Halo 64 (for the growth with the halo): 23,969 cells differ (2.29 %), 319 over 0.25 m, 53 over 1 m, maximum 3.5926 m; line cells 0 of 8,548; all of it in the non-line cells (largest at the seam, rows 25 of the core, 60 to more than 64 cells from any line cell).

- The line cells do not differ at all, at either halo (0 of 8,548 cells in bits; before GEN-176 the 2026-11-11 read did not split them).
- The only cell over 0.25 m at halo 128 is at local cell (66941, 8313): 130 cells from the seam (outside the halo's reach), 1 cell from a line cell; height 1.8268 m against 2.1354 m (diff 0.3085 m). The next largest are 0.0341, 0.0165, 0.0093 m, all at 66 to 76 cells from the seam and 1 to 2 cells from a line cell (`SEAMTOP` lines in `seam.txt`).
- The differing cells near a line (within 2 cells) are the banks; the probe did not trace which K7 step makes them differ (the 2026-11-11 read found the incising carve at halo 128 and deposition adding the maximum).
- Against the 2026-11-11 read at the same halo 128: bits 5,905 (0.56 %) against 7,054 (0.67 %); maximum 0.3085 m against 0.087 m (one cell, 130 cells from the seam); cells over 0.25 m 1 against 0. At halo 64: 23,969 (2.29 %) against 23,839 (2.27 %), maximum 3.59 m against 3.59 m.

## Caveats
- One tile, one region and seed, mountains and rivers; the second tiling is a 512-cell tile (not a half-tile shift); two halos (128, 64); the regional solve is that of main e129414d; no K7 step was switched off in this run (the step table of the 2026-11-11 read stands for the earlier code).
- Bit equality is not reached at halo 128 (0.56 % of the cells, none of them line cells); within the 0.25 m of G-SURF it holds except for the one cell above.

## Reproduce
```
cargo build --release -j 4 -p planet-generators --example gprofile_prod     # branch spike/k7-two-tiling (local)
PLANET_SEAM=65,8 PLANET_HALO=128,64 gprofile_prod 200 8                      # region R1h by default, margin 200, 8 threads; about 3 to 4 min when quiet
```
