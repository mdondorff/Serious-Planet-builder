# ADR 0084 option 1: a stored valley carve along the run's trunk chains (design sketch, no code)
Status: sketch (crusader, 2026-10-28), for the owner's choice; nothing is built. Inputs: SPIKE-49 (`spikes/m8-coarse-carve/results/results.md`, #829), ADR 0084, `docs/milestones/M8/owner-test-2026-10-27/README.md` D0, the held-back probe of the same day.

## Goal (measurable)
| Quantity | Today | Goal |
|---|---|---|
| Held-back tiles at frame-116591 | 112 of 1340 (widening 300 m both ways) | 24 at a down bound of 50 m, 0 at 10 m (probe `held_back_report`) |
| Chunk change against its base, 48 land chunks, levels 15 to 16 (`anet_change_report`) | p99 48 m, p99.9 138 m, max 207 m, signed -206.7 to +1.2 m | max |change| against the carved base under the bound that holds the goal above |
| Entry step at coast frames 41356 / 41595 | 1.9 m max at 41356 (p99 1.3 m) | unchanged or smaller |
| Valleys above level 14 | in no tile (explorer's note: valleys of 0.6 to 5 km exist only at chunk levels) | a valley of the run's trunks in every tile from level 4 |

## The channel
- One new channel of the bake store, beside the ocean channel (`StoreOceanSource`): the carve depth in metres at the bake's own lattice (16384 cells a face edge, 611 m), sparse (zero outside the carved cells), 8 bits at 0.5 m or 16 bits.
- Made from the coupled run (2048 cells, `z`, `area_m2`, receivers): the cells of the run whose area is over a threshold (SPIKE-49: 284,000 over 1e10 m2, 50,000 over 1e11), each a polyline along its receiver to the next, rasterised on the 611 m lattice (about 8 bake cells per run cell on a trunk).
- Depth: the depth law of GEN-057 at the run's area, `d = min(3 m (A / 1e8)^0.5, 100 m)`, evaluated at the polyline's cells with the area interpolated along the chain, so that it never decreases downstream (a monotone profile: no pits made by the carve); never below sea level (the floor of the field); zero on cells the run calls ocean or lake.
- Stored per bake block like the heights (compressed, verified, keyed by the bake key and the run's hash); a change of the law or the threshold is a new key.

## How the base adds it at every level
- `base height = bake height (bilinear, ADR 0027) - carve (bilinear over the same lattice)`, inside the base field's read of the bake, so every consumer sees it: tiles of every level, the plan's bounds, the horizon, the chunk stage's input. The carve is a smooth field at 611 m; a coarser level samples it the way it samples the bake, a finer level interpolates it.
- The tile key and the plan: the base's key (`bake_key`) covers the channel's hash, so every tile re-keys once; the bounds (`height_bounds`) take the carve's own range per block (stored in the block header), so the plan stays independent of the chunks (LOD-013, CON-14).
- CON-11: the carve is read camera-relative like the bake; no planet-radius values in shaders (the field is CPU-side).

## How the chunk's change is measured against it
- The A-net's input for a chunk is the base's heights over the region (`region_heights`, GEN-087): they include the carve now, so the solve starts from the carved ground and `change = solved - carved base`.
- The stage's stated bound (`bound_m`) is set from the measurement, not from 300 m: land the 48-chunk sample (`anet_change_report` over the carved base), take max |change| per level with the 10 % margin of GEN-039, and clamp the stage to it (`clamped_change`). The goal is a down bound of 50 m or less: the held-back count follows the down bound only (probe: 300/300 gives 112, 250/10 gives 89, 100/10 gives 65, 50/10 gives 24, 10/10 gives 0). The up bound can be 10 m (measured +1.2 m).
- SPIKE-49 warns that a carve of the 2048 run alone (one number per 4.9 km cell) holds almost none of the chunk's change (hills: 9.7 to 7.9 m median); this option differs: the carve is at the bake's 611 m lattice along the trunk chains, so a 611 m wide valley is a valley of the base. What it cannot hold is what the chunk resolves below 611 m and off the trunks: the bound must come from the measured remainder.
- Test first (no build): compute the carve for the 48 sample chunks' regions in memory (depth law over the run's trunk polylines, rasterised at 611 m, bilinear), re-run the chunk solve against it, report p50, p99, max of the change before and after and the bound it allows. If the max stays above about 100 m the option does not buy the held-back tiles and the owner should know before the build.

## The rebuild
- The coupled run stays (about 20 s); the carve channel is added to the bake store (a pass over the trunk cells: seconds to minutes, to be measured); every chunk re-solves (the base key changes the chunk key; the disk cache of chunks is invalid once); the pre-filled coarse tiles of the pack (levels 3 to 4) are rebuilt (`generate --prefill-dir`); the coarse horizon map reads base heights and is made again (`generate --coarse-horizon`); look-review stills and golden candidates go through golden-review (ADR 0017).
- Order: channel and its reader behind a zero law (today's bits), the measurement, then the law on; a zero depth law is today's field (ADR 0084 consequence), so the reversal is a key change.

## Risks and open points
- Aliasing: a bilinear carve over 611 m cells makes a V of 1.2 km; the chunk must still dig its own valley under it (the change is negative where the chunk cuts deeper, positive where it cut less: the signed range matters for the up bound).
- Hydrology: the carve follows the run's receivers (2048 cells, 4.9 km); the chunk's network routes on the 8 m grid and may leave the run's line by the router's band: the carve then lies beside the chunk's channel (a ridge of 0 to 100 m next to the cut): measure the lateral offset on the sample.
- Items and stamps apply above the base and are unaffected; GEN-069's cut cap and a river over a plateau edit (finding 15) are separate.
- Seams: a carve is one more thing the bake's block seams can show (finding 3 coast cuts matched the block seams in SPIKE-49); the channel must use the bake's own block interpolation across seams.
- Cost: the channel is sparse; the base read adds one more block lookup per height (the cold-block cost of the editor's first read applies to it too: `height-warm` reads it).
