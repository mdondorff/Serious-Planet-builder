# M8 learnings (archived 2026-10-30)

Moved from `docs/learnings.md` at M8's housekeeping: they describe the chunk stage (removed after M8) or the horizon maps (retired by ADR 0085).

## 2026-10-25 The chunk stage's 300 m depth is a clamp of a runaway: its corridor anchors lie far under the base ground
Scope: generators::anet, app::anetworld (GEN-047, GEN-051; look review 1, `docs/milestones/M8/look-review-1-hills/`)
- The unclamped solve of the chunk under the steepest-site channel cuts to -2659 m (16,125 of 369,664 core cells deeper than 250 m, 15,318 at the 300 m clamp); the second site cuts to -1322 m. Every cut channel is the same depth because `ANet::compute` clamps to `bound_m`, with vertical walls, which ADR 0074 caps at coarse tiles (121 to 129 of about 1750 plan nodes) and draws as spikes.
- The corridor beds are anchored at the lower of the coupled run's height and the base; the run's heights lie 1.2 to 2.7 km under the base at those channels (anchor minus base, median -2596 m and -1221 m).
- The coupled run compresses relief against the base: its height minus the base's at cell centres, median over river cells, is +111 m at 0 to 500 m ground, -164 m at 500 to 1000, -933 m at 1000 to 2500, -2686 m at 2500 to 5000, -4649 m over 5000. Anchors above the base are ignored (the minimum takes the base), so only the low band cuts mildly.
- Tools: `chunk_incision_report` and `run_against_base_report` (`cargo run --release -p planet --example chunk_incision -- --store DIR --lat DEG --lon DEG | --run-vs-base`), `capped_nodes` in `stills.txt`.
- Obsolete when the chunk stage's anchors are made consistent with the base field.

## 2026-10-25 The horizon bake is cheaper than the spike measured; the shadow engine has three declared limits
Scope: M8-5 (horizon shadows), `generators::horizon`, `coarsehorizon`, the lit shader
- The bake costs 21 us per texel (16 azimuths, 5 km, 4 m texels, release, one thread): about 5 s on 6 threads for a 24 km2 chunk, against SPIKE-32's 0.7 s per km2 (17 s). The coarse horizon at 512 observer cells per face edge over the 2048-cell run takes 7.7 s on 8 threads.
- Limits stated in the specs (GEN-044, GEN-045, REND-038): the tangent warp makes a texel up to 1.41 times as long along `i` as along `j` near a face edge, so the bake's distances (and the horizon angle) are off by up to that factor there; the coarse azimuths use each cell's own geographic frame, which differs from the fragment's near a pole; the shader reads 16 azimuths exactly.
- On the look-review stills the shadow edge along canyon rims is scalloped (16 azimuths fan near an occluder); the far-field outline at sunset could not be judged on flat sites.
- Obsolete when the bake marches in metres, the coarse map gets a pole-free frame and the azimuth count is decided from stills.


## 2026-10-25 The chunk stage's walls exceed the angle of repose because the hillslope steps skip every pair with a channel cell
Scope: generators::anet (GEN-051), app::stills, app::anetworld; look review 1 second site after R-a
- At the steepest site the 131 capped plan nodes (30 m pose) are all level 16 (153 m): 10 are over the drawable slope 3.42 on the 8 m heights, 12 are between 0.9 and 3.42, 109 are flat (median steepest slope 0.10) and held by ADR 0074's ceiling next to a steep tile.
- The base alone has no neighbour pair over 0.12. In the solved chunks 6,010 of 1.48 million edge pairs (site 1) and 20,612 (site 2) are over 0.9, the steepest 5.72 and 9.39.
- 42 % (site 1) and 22 % (site 2) of the steep pairs have a protected cell (channel, corridor, anchor); `slump_deltas` and `diffuse` skip those pairs, so the bank-to-bed step is never relaxed.
- A slump rate of 0.5 (from 0.09) and 90 steps (from 30) cut the pairs of two unprotected cells from 3,494 to about 820 and leave the protected ones at 2,200 to 2,700: rate-limited and structural, in that order.
- Tools: `capped.txt` of the stills tool (first pose), the pair census of `chunk_incision_report`, whose `--slump-rate` and `--steps` arguments replace the stage's defaults for the report only.
- Obsolete when the hillslope steps relax the bank next to a channel cell.


## 2026-10-25 The chunk fade is bounded on screen, not per frame: the real run's entry step is up to 58 m, not 17.55 m
Scope: m8-chunk-fade (REND-039, STRM-047), criterion 3 of M8; app::globalperf `anet_steps_report`
- On the shared store (bake 16384, stored fill-only run, chunk of the largest inland river, face 0, 14.1 million km2 at its coarse cell) the change at the entry level is all incision: p0 -58.3, p50 -25.9, p90 -0.4, max +0.15 m over 69,696 vertices; the entry step is p99 56.9 m, max 58.3 m. The 256 bake's figure was 3.90 m p99 and 17.55 m max, for a smaller river.
- A fade over 1.5 s at 60 fps moves a vertex by 0.65 m a frame at most (p99 0.63 m); 0.25 m a frame would need 3.9 s. On screen at the entry distance (about 4 km at tau 6 px, 1440p, 60 degrees: 3.2 m a pixel) that is 0.20 px a frame, inside the amended 0.5 px.
- The steps between pyramid levels inside the chunk range are inside 0.25 m p99: 0.126 m (15 against 14), 0.030 m (16 against 15).
- The screen numbers are computed from the field and the camera geometry; no rendered frame measures the pixel step.
- Tool: `PLANET_ANET_STORE=DIR PLANET_ANET_RES=N PLANET_ANET_MIN_HEIGHT_M=150 cargo test -p planet --release --lib anet_steps_report -- --ignored --nocapture` (read only).
- Obsolete when the criterion is judged on rendered frames of the fade (look review 2).

## 2026-10-26 Sequential fade epochs do not survive a real flight: the rebuild of the tiles under a landing takes seconds
Scope: m8-chunk-fade (STRM-052, STRM-053), the closed `m8-chunk-fade-publish` (branch `change/m8-chunk-fade-publish-impl`, PR #714 closed); app::window, frame::managed
- The idea: publish landed chunks to the tile pipeline one fade at a time, so a tile never sees two fades (the crack of overlapping landings, numerics review of #711). The gate, its queue, the fresh-tile wait and the 5 s fallback were built and tested on the fake clock.
- The real window, the gate on, the shared store (RTX 3080 Ti), the scripted orbit to 1 m flight repeated, two attempts. All resident readers rebuilt before the clock: 14 forced publications in 3 min (11 by the readers' timeout), longest landing to drawn 80 s, 133 to 295 tiles deferred. Only the plan's drawn readers: 12 and 14 forced in 4.5 min (10 and 13 timeouts), longest 10 s and 40 s, 352 and 174 tiles deferred. Frames over 1 s: 0 in both.
- Cause: in a real flight the refresh of the hundreds of tiles that read a landed chunk shares the streamer's workers with the new tiles of the descent and takes seconds, not the dozen frames of the test rig. A rule that waits for it before the next epoch makes the 5 s fallback the normal path.
- The same latency is in the merged fade: a chunk's change is drawn only when the tiles that read it are rebuilt; the clock of #711 waits for the stale tiles that are drawn.
- Tool: `PublishStats` on the closed branch (title, stall report, log line every 10 s with what forced a publication).
- Obsolete when the tile rebuild after a landing is fast enough (a priority for refreshes of drawn tiles, or the increment on the CPU without a rebuild).

## 2026-10-26 The horizon march is bound by arithmetic per step, not by loads: a block-max skip gains nothing
Scope: `generators::horizon` march, 8 m raster of 1824^2 texels, 16 azimuths, range 4.8 km.
- Measured offline (4 threads, busy machine): an exact skip of steps whose block max cannot beat the best slope skipped 60 % of the steps and took 6.5 to 8.7 s against 6.8 s without it.
- Cause: each step costs two divisions, a floor and a division for the slope; the table of distances and drops and an integer floor take the march to 2.6 s with identical bytes (GEN-085).
- 2026-10-28, vectorised (GEN-086): the same exact skip drops 45 % of the 4-lane steps, a map stays 2.06 to 2.3 s on one thread; now bound by the raster reads. Faster: cache-blocked or per-azimuth sweep.

## 2026-10-27 The A-net chunk change is almost only incision, up to 207 m; the stated bound of 300 m is two-sided, the data one-sided
Scope: `chunkfield` stage bound (`bound_m` 300 m), `ChunkField::height_bounds`, the plan's cost with a chunk stage; `app::globalperf` `anet_change_report`.
- 48 random land chunks of the 16384 store, 65 x 65 samples a level (11 to 16): |change| p50 0.9 m, p99 48 to 49 m, p99.9 137 to 138 m, p99.99 190 m, max 201 to 207 m; signed range -206.7 to +1.2 m. The largest is at face 1, chunk (25, 1968).
- `height_bounds` widens by `bound_m` on both sides; the upper 300 m is unneeded for this stage (+1.2 m seen). An upper bound of a few metres (with the stage clamping its positive change there) would halve the slack that defeats `certified()` and stays sound; not built (the plan is at most 0.5 s of the M6 gap, window 14).
- A split or morph on bounds that leave the chunk change out is unsound (numerics review, 2026-10-27): the camera can be treated as higher than the ground, and a border vertex can lie outside the morph margin (a crack). The widening is what keeps both honest, and LOD-013 forbids a widening that depends on readiness.
- A tile costs 2 to 3 ms with the chunk layer and 2 ms without (`tile_cost_report`): the 340 to 1100 ms of an edit's tile refresh is not the tile build.
- Tool: `PLANET_ANET_STORE=DIR PLANET_ANET_RES=N PLANET_ANET_CHUNKS=48 cargo test -p planet --release --lib anet_change_report -- --ignored --nocapture` (read only, about 7 min).
- Obsolete when the stage states an asymmetric bound or the sample grows past 200 chunks and changes the maximum.

## 2026-10-27 The tail of an edit's tile refresh is four level-3 tiles, and the chunk scene's M6 column had been reading all of them
Scope: `edit-latency` with a chunk stage (TEST-056), the M6 and M8 criterion 6 comparisons; `app::editperf`, `app::globalperf` `tile_cost_report`.
- Corrects the line above ("a tile costs 2 to 3 ms"): that was height only at 32 cells, levels 15 to 9. On the mesh path of a real tile job (64 cells) with / without the chunk layer: level 15 12 / 12 ms, 9 20 / 21, 7 40 / 36, 5 56 / 59, 4 103 / 59, 3 96 / 64. The coarse cost is the base field's; under 18 parallel loads a level-3 tile takes 120 to 510 ms.
- Every edit names the four level-3 ancestors (6,5), (6,6), (7,5), (7,6) of face 0 (resident only, "change below spacing"); they end last, one at a time. Lock waits are not the cause (windows 18, named: all under 55 ms in all per case).
- The chunk scene set `drawn exact = uploaded` (every named tile), so those four gated the M6 column (windows 13 to 20: 7 of 13 gated rows over). Window 21 reads ADR 0072 point 1 (the named tiles the plan draws, from the job log): 53 to 758 ms, 1 row over (the preview column). Numbers before window 21 are on the old reading.
- Obsolete when a coarse tile's refresh is deferred to a lower class or its build gets cheaper.

## 2026-10-27 A scene with few chunks hides main-thread work that a flight over many chunks multiplies
Scope: `ChunkManager::update_with`, `WorldChunkVersions::version`, the editor's chunk step; `app::globalperf` `chunk_frame_cost_report`.
- A chunk's version with an item in a stamp layer needs its read set (0.2 to 0.85 s at 8 m cells); it was made on the main thread for every new chunk: 37 stalls of 1 to 15 s in the owner's hour. The edit-latency scene (7 chunks, cached after frame 1) cannot show it; a probe that moves the camera over new chunks can (21 of 30 frames over 20 ms before, none after).
- Obsolete when the read set takes a few ms.
