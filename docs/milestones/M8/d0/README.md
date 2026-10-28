# M8-D0 decision measurement (crusader, 2026-10-28)

Variant: SPIKE-50 d1 channel depth (`PLANET_ANCHOR=depth_m=0.27,exponent=0.3,cap_m=15,cap_low_m=15,cut_cap_m=15`) + the coarse carve in replace form (`r = c + min(carve, max(-c, 0))`, scale tuned per run: 2.68 d1, 5.23 today) + detail-only correction (`r - S`, S the box mean of the replaced level-16 change over 2h+1 raster samples of 4.77 m).
Store `global16384`, in memory, probes `d0_variant_report`, `residual_change_report`, `held_back_report` on `change/m8-group-d` (unmerged).
Sets: 10 owner-test poses (nearest 4 chunks of the level-15 nodes), site9 (frame-139490 chunk and its 8 neighbours), random 48 land chunks (`anet_change_report`'s). Raw output beside this file.

## Definitions
- Level 14 reads the carved base, so the entry step (15/14) is `|r15 - S|`; the step 16/15 is `|r16 - r15|`.
- "Box-filtered to the apply level" is ambiguous. Level 15's vertex spacing is 9.5 m, a quarter octave over the 8 m raster, so the literal reading is h=1 (9.5 m). h=16 (157 m) and h=64 (615 m, the bake cell) show what a correction at a scale the base can carry gives.
- Rivers: SPIKE-51's metric (spikes' `uphill_report`, rises over 0.5 m per 100 km, drawn bed at level 16, 8 samples per link). Reproduced: today's parameters give 209 rises (99.6 per 100 km, max 19.5 m, random) and 476 (227 per 100 km, site9).

## 0.71 m against 0.390 m
- 0.390 m reproduces (default parameters, entry chunk, all vertices, pyramid top level 11): it is the step between pyramid levels 15 and 14 of the chunk's own pyramid, which production does not have (the pyramid ends at 15; level 14 reads the base).
- The production step 15/14 is the entry step against the base: p99 110 m on that chunk (63 m with d1). Over the 48 random chunks: p99 49.7 m (today), 14.1 m (d1); at site9 34 m (both).
- 0.71 m is in no stored output; the probe of that figure is not on the branch. Not reconciled beyond the above.

## Conditions (pooled; d1 variant; entry step is `|r15 - S|`)
| Condition | h=1 (literal) | h=16 | h=64 | Pass |
|---|---|---|---|---|
| Entry step p99, 48 random | 0.12 m (max 2.2) | 2.6 m (21.7) | 4.8 m (37.7) | h=1 only |
| Entry step p99, site9 | 0.28 m (max 3.2) | 5.9 m (17.4) | 10.9 m (30.3) | none (0.25) |
| Entry step p99, 10 poses | 0.21 m (max 3.5) | 4.3 m (21.7) | 8.4 m (27.9) | h=1 |
| Step 16/15 p99 | 0.09-0.22 m | same | same | yes |
| Chunk change after correction, 48 random | -2.8 to +3.3 m | -22 to +14 m | -38 to +21 m | margin yes |
| Rises over 0.5 m per 100 km (random / site9) | 367 / 156 | 282 / 152 | 275 / 153 | no |
| Held back at frame-134050 / 139490, flat 5 down 5 up | 59 / 50 | | | no (target 0) |
| Held back at frame-116591, flat 5/5 | 0 | | | yes |

Today's reference (d1 parameters, no carve): entry step p99 14.1 m (random), rises 163 (89.8 per 100 km, max 3.2 m) random and 581 (276) site9. Today's parameters: 209 (99.6) and 476 (227).

## Findings
1. **Level boundaries pass only at h=1, and only because the chunk then adds nothing coarse.** The correction at h=1 equals the whole change (p99 12.4 m random, 34 m site9, max 79 m); what the chunk keeps is detail under 9.5 m (p99 0.19 to 0.45 m, max 4.5 m). The carve alone cannot carry the valleys (carve-only entry step p99 12.4 m random, 34 m site9: the carve line is the 610 m bake lattice). At a scale the base can carry (h=16, h=64) the step is 2.6 to 10.9 m p99: fail.
2. **Rivers fail in every form.** Replace-carve raises the rises on the random set from 163 to 530 to 666 (max 20 m, 5 to 6 over 10 m; today 0.0 over 10 m with d1): the carve line is the coarse run's polyline, which does not lie on the stage's routed channel, so it cuts a second groove beside it. Site9 improves (581 to 329) but stays at 156 per 100 km, against today's parameters' 227.
3. **Chunk change after the correction is small (4.5 m at h=1) but held-back tiles are not 0 with a flat bound.** At frames 134050 and 139490, a flat 5 m both ways still holds back 59 and 50 tiles (98 and 87 with up 10 m, 117 and 119 at 30/25 m). SPIKE-51: the margin must scale with the tile. A per-tile bound from the chunk data was not run: it needs the plan to take bounds per tile (code, not measurement).
4. **Correction size against the detail.** |S| p99 12.4 m (d1) and 34 m (today) random, 34 m site9; the detail kept at h=1 is p99 0.19 to 0.45 m: the correction is 60 to 100 times the kept detail.
5. **Residual 76 m named** (`residual_change_report`, chunk (4, 73, 1949), d1): the eight largest changes (-78 to -80 m) are cells at 4.9 km height, not corridor, not anchored, drainage area 7.2e4 m2 (over `channel_area_m2` 5e4 m2), 0 cells from the next channel: the stage's own stream-power incision of steep small channels in alpine ground, lowering the cell by 80 m. Depth laws and cut caps touch anchored corridor cells only, so the figure is the same under d1, d2 and d3. 540 cells of that chunk are over 60 m, 12,541 over 40 m. The lever is the channel incision (the stage's erodibility and the steepness), not the anchor law.

## Verdict against the table
| Condition | Result |
|---|---|
| Level boundaries (under 0.25 m p99) | pass at h=1 on 48 random (0.12 m) and 10 poses (0.21 m), fail at site9 (0.28 m); h=16 and 64 fail; passing removes the chunk's coarse change |
| Rivers (no rise over 0.5 m) | fail: 275 to 367 per 100 km (random) against 90 today (d1) |
| Chunk change, held-back 0, correction small | change margin yes; held-back 50 to 59 at two poses with a flat 5 m (not 0); correction not small against the kept detail |

Tweaks that might pass, not run: a per-tile bound for the held-back count; a carve laid on the stage's routed channel (not the coarse polyline) for the rivers. Neither is a parameter change; each is a build.
