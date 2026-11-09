## Decisions
1. `K7Params::canonical_flood` (on in `standard`): K7's pit statistics (`pit_stats_with`), its carve rules (`RoundRules { clip_mfd: Off, canonical_flood }`) and its deposition sweep (`deposit_on_own_route_flood`) use one flood. Off, the carve rules are the default ones and K7 is the K7 of before (digests below).
2. `deposit_on_own_route` is `deposit_on_own_route_flood(.., false)`: pass 2's call is bit for bit as before (the test asserts the bits and the report).
3. K7 has no kernel version constant and nothing is stored from it; the version is set when K7 gets a store (item 15). Nothing to bump here.

## Measurement (`k7_region 200 8 compare`, the tile (65, 7) of face 1, 1,024 x 1,024 core cells, 8 threads)
| Item | With the channels burned in | Without |
|---|---|---|
| core cells whose stored L differs, canonical against plain | 10,395 (0.991 %) | 6,343 (0.605 %) |
| largest change of L | 0.948 m | 0.271 m |
| cells over 0.25 m | 113 (0.0108 %) | 2 (0.0002 %) |
| pits after the whole pass, canonical and plain | 205 (0.0197 %, max 20.3 m) both | 456 (0.0438 %, max 27.5 m) both |
| G-SURF at the cell centres | 5.7e-14 m unquantised, 0.125 m quantised | the same |
| G-SURF-I at the vertices (over 0.25 m, p99, max) | 192 (0.66 %), 0.225 m, 2.554 m | 209 (0.72 %), 0.228 m, 2.554 m |
- Every pit count at every stage is the same under both floods; only the deposition and the incision's routes move L.
- The plain-flood K7 digests (z and change bits, smooth tile): 68e2e76ffed7d2b7 with a channel, 7700012a670c5e69 without, taken on origin/main at 3d3ad99a before the change.
- The tile is one area; the share of L that moves would be larger on flatter land.
