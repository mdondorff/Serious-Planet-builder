# k7-bed-coherence: design notes

## Evidence (R3a, r3a-e1, `k7_line_check`)
| Tile | Line cells only in-memory / only stored | Stored B + L vs K7 z (max) | Land steps | Rises > 0.25 m (max) | At a crossing without a join | Burn cutoff binds (diff max) | Final-pass binds (diff max) |
|---|---|---|---|---|---|---|---|
| 89/7 | 0 / 0 | 0.125 m | 12,472 | 37 (41.9 m) | 4 | 1,980 (365 m) | 827 (223 m) |
| 83/7 | 0 / 0 | 0.125 m | 4,885 | 15 (17.1 m) | 0 | 1,230 (62 m) | 374 (40 m) |
| 83/8 | 0 / 0 | 0.125 m | 3,977 | 10 (18.3 m) | 0 | 717 (38 m) | 314 (23 m) |

- Channel 2545 (tile 89/7): it crosses channel 484's trench at one cell (393.3 m) and stays at 435.2 m after it: the burn and the final pass read the heights as found, so a cell another line lowered is not carried.
- The earlier viewer probe (29 % of the steps rising) read the viewer's base under `--commit`, which differs from the bake's B by up to about 3 m: not in the data (VIEW-072, explorer).

## Decision
- A'': the levels of the whole network (`ChannelGraph::deepen_beds`, called in `region_front` before the graph is stored and the channels are made). A function of the network and the start surface, so partition independent by construction.
- A fixed-point final pass was not taken: it propagates beyond the halo (partition dependence) and the crossings cause 4 of 62 rises.
- A' (a crossing ends the higher line into the lower) stays out unless the crossing rises survive.
- B (the incising carve lowers channel cells) was dropped: it removes the 22.6 m dyke of a fixture, but the incision cut line cells up to 300 m on l_graph's profile fixture and left a 189 m rise one cell below a junction. The incision makes the line's heights depend on tile-local stages, which only a bounded final pass can follow. Parked on the local branch `wip/k7-gen186-b`; F3 goes to line placement at L.

## TEST-134 fixture (rugged region of TEST-125)
| | H1 levels | Network levels |
|---|---|---|
| Rises > 0.25 m | 44 (max 5.0 m) | 0 |
| Burn cutoff binds, tile (2, 2) | 33 (257 m) | 0 on every tile |
| Level - ground at land points, p50 / p99 | +4.9 / +194 m | +0.02 / +73 m |
| Levels under the start surface, p50 / max | 24 / 459 m | 36 / 493 m |
- 352 of 2,312 points are coast points: a level under the sea with the ground floored at the sea level (GEN-176).
