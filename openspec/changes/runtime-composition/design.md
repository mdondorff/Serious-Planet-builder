# Item 15 design notes: sampler composition, lakes, rivers, G-RT

Sources: `docs/design/terrain-target.md` (A3) sections 3.1, 3.3, 5.2, 5.3, 5.5, 11.2, 12.1 (G-SURF, G-PROFILE, G-BASIN, G-RT); ADR 0101 (fill depth), ADR 0041 and 0056 (water field and coastal band), ADR 0072 (stale pre-emption); briefing items 7, 11, 14, 15, 16. Written 2026-11-10 from the documents and the code on main; nothing measured for this note.

## 1. State on main
| Part | What exists | Where |
|---|---|---|
| Base `B` | the lattice's C1 read, `read_elevation_c1` and the cached read, pure in the direction, f32 once (GEN-107) | `crates/generators/src/global.rs` |
| Local change `L` | kind-13 layer of the old two-pass bake; the runtime adds `change + fill depth` (GEN-112, ADR 0101 point 3) | tile source, `terrain.rs`, `erosion.rs` |
| New chain | K4 (rounds), K6 (H1: channels, basins, `Q_w`), K7 on a tile, the complete build on the phase plan (GEN-158); none of it reaches the runtime | `crates/generators/src/bake/` |
| Water | the water field comes from ADR 0041's shoreline and the coastal band (ADR 0056); a `WaterBook` holds the tile water; no lake object, no river drawing | `crates/frame/src/water.rs`, `crates/app/src/worldfield.rs` |
| Creases `C` | evaluated at read time (GEN-109) | `terrain.rs` |
| G-SURF | gate at the cell centres holds (GEN-148); vertices reported as G-SURF-I | explorer, item 11 |

## 2. Step 1: sampler composition (A3 section 3.1)
Goal: the runtime reads `S` with `L` from the new chain's K7 (total change against `B`) and no fill depth.
- Switch point: A3 3.1 makes the retirement of the fill depth wait for G-BASIN and the lake producer: until a lake is drawn, a basin would show a dry hole. So the switch cannot ship before step 2 for any world with basins; the order is step 2 first, then the switch.
- The input is a stored layer: item 14 (G-STORE) defines the serialized `L` and the key table; item 15 reads it through the existing tile source and folds `base_id` into the key (A3 3.2). A mismatch is refused naming both keys (STRM-065).
- Gate: G-SURF and G-SURF-I on a tile, then G-PROFILE (section 4).

## 3. Step 2: lakes (A3 section 5.5, K11)
Goal: a basin kept by H1 is drawn as water at the level of the balance; the fill depth stops being a surface term.
- Producer: from the basin object of the build (floor, spill, area curve, class, `Q_in`, `E_open`), the level per section 5.5's three cases, `q_out` at the spill. All of it is bake output (tile data, not per-frame work, A3 11.2): the producer is a bake-time step writing the level and the class per basin into the store; the runtime only reads.
- Runtime side: the water field gets a source besides the shoreline: per tile the cells below the basin's level at the tile's resolution (the area curve answers "what is flooded at level x" without a raster, A3 5.5).
- Endorheic playa and `Q_in = 0`: the floor, no water surface.
- It needs item 11's pass (the surface the basins are taken on), G-BASIN's fixtures for the levels (1e-9), and the finding of item 13 (results section 6b, finding 2): the straight-edged wedges of the 1:1 sheets are H1 channel cells inside kept lakes, so a lake draws as water and the channel inside it is not drawn as a river line (A3 5.2: a channel passes through a basin and continues below its spill point); the producer drops those cells from the stored segments (decision 5, section 7).
- Gate: G-BASIN (the explorer's fixtures) plus a view of the prototype region from 2 km at three lakes: the level against the sill.

## 4. Step 3: rivers (A3 sections 3.3, 5.2)
- Source: K6 segments (centrelines with the level profile and `Q_w`), stored as tile data (item 14).
- Drawing: a ribbon or a water-pass mask along the segment from the one width law `w = a * Q_w^b` of decision 2 (section 7), the same as K7's burn-in; the river's water level is the segment's profile, which K7 burned into `L` (burn-in rule), so the ground and the water agree to the quantisation step.
- Gate G-PROFILE (A3 12.1): along every segment through the complete read `S` at the local nodes, `C` off and on; no rise above 0.25 m; tile-border step at most 0.25 m. It needs the stored segments and `L`; first a test on the fixture tile (K7 of one tile exists: the stage manifest's `k7-f1-t3-4`).
- Risk: a C1 read lifts diagonal links between nodes by design (reported, not gated); a river drawn at the node profile can float over a lifted link by the reported rise, so the width law must cover it.

## 5. G-RT: the region's flight
No held-back tiles; frame within BUDG-001 and BUDG-002. The flight is the prototype region at 12,288 cells per face edge (item 13's windows R3a and R3b are candidates for a second flight). Measured by `test-render --perf` on the lit path with the new `L`: a perf window (the planner authorises, bulldozer coordinates). The smoke runs on the software adapter first (the run-time stamps of TEST-106 and the streaming smoke's marks tell where a run is).

## 6. Order and sizes (estimate, one lane)
1. Lake producer and the lake water source (bake step, store field, water source): the largest step, about 3 PRs (producer with fixtures, store field, water pass source).
2. Sampler switch (retire the fill depth): 1 PR after step 1 and item 14's store field.
3. Rivers: segment store read, ribbon drawing, G-PROFILE: about 3 PRs.
4. G-RT flight and the report: 1 PR plus a perf window.
Each PR carries a requirement or two in `streaming-cache`, `renderer` or `generator-pipeline`; ids from the planner.

## 7. Decisions (architect D6, 2026-11-10; inbox #109) and what they fix
| # | Question | Decision | Consequence in the steps above |
|---|---|---|---|
| 1 | Lake producer | at bake time in `generators`: level, class and `q_out` per basin in the store; the runtime only reads | step 2 is a bake step plus a store field; the runtime has no balance code |
| 2 | River width law | one law `w = a * Q_w^b`, `a` and `b` bake parameters in the bake key, shared by K7's burn-in and the drawing; start `b = 0.5`, `a` from the burned trench widths; the PR states the value and its source | the ribbon and the burned trench agree by construction; a change of `a` or `b` re-keys K7 |
| 3 | `L` in the store | a version of kind 13, a bump that refuses older stores, keyed with `base_id`; a new kind would come to the architect first | step 1's key; old stores are refused, not read |
| 4 | G-RT's flight | the prototype region; R3a and R3b as a second flight, reported | section 5 |
| 5 | Channels inside kept lakes | dropped at the producer; the renderer draws what is stored | step 2's producer removes the channel cells inside a kept basin from the stored river segments; step 3 draws segments only |

Start: code begins after item 11's rows (explorer, after GEN-162 merges).

## 8. First PR: the lake producer, test plan (on paper, 2026-11-10; ids from the planner)
Finding in the code: `bake/hydro.rs` `Basin` already holds `water_level_m`, `q_out`, `overflows`, `class`, and the area and volume curves at 16 steps. So the producer is not a new balance; it is the step that turns H1's basins into stored lake records and drops the channel cells inside them.
| # | Test | Tier | Falsifies |
|---|---|---|---|
| 1 | A fixture basin (bowl with a known sill, `Q_in` and `E_open` chosen): level equals the section 5.5 case value to 1e-9 for the three cases (full to the sill, below the sill, `Q_in = 0`) | A | wrong level; shares G-BASIN's fixtures with the explorer |
| 2 | Level is never above `spill_m` and never below `floor_m`; `q_out = 0` iff the basin does not overflow | A | balance leaking past the sill |
| 3 | Endorheic and `Q_in = 0`: record flags "no water surface"; level = floor | A | a playa drawn as a lake |
| 4 | Lake record per kept basin only: count and ids equal `Hydro::basins` ids (`basin_id`); a ramp (pit under the limit) yields none | A | stray or missing lakes |
| 5 | Segments after the producer: no channel cell with `label` in a kept basin remains (decision 5); segment ends at the lake's inlet and the downstream segment starts at the spill with `q_out` | A | fans inside lakes (item 13 finding 2) |
| 6 | The record round-trips through the kind-13 store layer (decision 3): encode, decode, equal; an older version is refused naming both versions | A | store format break |
| 7 | Determinism: the same basin set in a shuffled build order gives identical records and bytes (CON-14) | A | order dependence |
| 8 | Key: a change of the producer's parameters re-keys the layer | A | stale store read as current |
| 9 | Stage manifest: the fixture bake lists the lake step with its input and output keys | B | step missing from the phase plan |
Ids (planner, inbox #110): GEN-163 the lake producer, TEST-108 its tests.
Split (decision of the lane, reason below): the first PR holds the producer in memory, tests 1-5, 7 and 9 (`lakes_from_hydro(&Hydro) -> Vec<Lake>` and the segment drop); the second PR holds the store field and the kind-13 version bump, tests 6 and 8.
- Reason: the bump refuses every older store, an effect on all worlds that deserves its own review and its own revert point; the producer's numerics (levels to 1e-9) are reviewed apart from format work.
- Cost: one more PR; the second is small (encode, decode, key, refusal).
Order inside the first PR: tests 1-5 red, the function, then the manifest entry (test 9).

## 9. PR 2: the basin table and the version of kind 13 (architect D16, 2026-11-10; inbox #115)
Facts from the code (`crates/cache/src/layer_tiles.rs`): kind 13 is one record per tile, `TILE_LAYER_VERSION = 2`, payload `codec u8 | tile u32 | stats | key 16 B | brotli(row-delta i16 counts of 0.25 m)`; the layer version is in the key (`TileLayerSpec::bytes`), so a bump changes every key and an older store is refused by key (STRM-065), not read. Item 14 (D11) settled no format of the new chain's `L`.
Decisions (D16):
| # | Question | Decision |
|---|---|---|
| 1 | `L` | the same kind-13 record, `TILE_LAYER_VERSION = 3`, new codec byte 3 (payload = item 14's measured encoding, GEN-156; stats block = the new chain's K7 stats; key includes `base_id`; version 2 refused naming both versions; the fill channel is gone) |
| 2 | Lakes | not in kind 13: a basin table per K6 key unit, records sorted by basin id (id, floor, spill level and point, level, class, flags, `q_out`, tile range); a new cache kind `Kind::BasinTable = 19`, requirements STRM-080 |
| 3 | Reading | the runtime loads a region's table once and indexes by tile; per-tile copies are rejected; the table and kind 13 are read under one `commit_id` (A3 section 7.4, CON-22) |
| 4 | River pieces | with the channel graph in the rivers PR, not PR 2 |
Split (lane): PR 2a the basin table (kind 19, encode and decode, key, refusals, tile ranges from the producer); PR 2b kind 13 version 3 with codec 3 and the key with `base_id`; the shared `commit_id` read in whichever merges second.
Tests, 2a (TEST-108 continues; tier A, cache crate):
- T6 round-trip: encode then decode a table of 0, 1 and 3 lakes: equal records in id order, whatever the input order.
- T6b a table of another version, key or commit is refused naming both.
- T6c the tile range of a lake across a tile border holds both tiles.
- T6d a decoder never reserves more than the declared count allows: a count larger than the payload is refused.
- T8 re-key: a change of a producer parameter changes the table's key; the same parameters give the same key.
- T9 size: `n` lakes take exactly the header plus the fixed record size times `n` plus the tile list entries.
