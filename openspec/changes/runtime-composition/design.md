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

## 10. After the stores: the lake water in the frame, then the sampler switch (on paper, 2026-11-10; code read: `frame/src/water.rs`, `cache/src/layer_tiles.rs`)
State after PR 1, 1b, 2a, 2b: the producer makes lake records and river pieces; the basin table (kind 19) and L (kind 13, layer version 3) have formats and tests; the bake does not write either, the runtime reads neither, and the old layer (codec 2 with the fill channel) is still the one the viewer reads (`StoreLayer`, `with_fill_added` at `layer_tiles.rs:598`).
| # | PR | What | Gate |
|---|---|---|---|
| 3 | bake writer | K7 over the tiles writes the L records and, from `lakes_with_tiles`, the basin tables of the six faces, under one `commit_id` (A3 section 7.4, CON-22; open point for the architect); the manifest gets the L versions | a built fixture reads back; the table's lakes equal the producer's |
| 4 | lake water source | a `WaterSource` over the basin table beside the sea source: a tile that lists a lake answers with the lake's body, level and liquid | G-BASIN, and the view from 2 km at three lakes (level against the sill) |
| 5 | sampler switch | the runtime layer source reads L (`load_l_tile`) and adds no fill depth; `with_fill_added`, `fill_depth_m` and the fill channel's readers go (ADR 0101 point 3) | G-SURF at the cell centres, G-PROFILE |
Order is fixed by the dry hole: PR 5 may not merge before PR 4, or a basin shows as a dry hole (A3 section 3.1).
The problem PR 4 has to solve (from `water.rs`): a tile has one level, and the pass draws the tile's grid at that level wherever `W - h_morph > 0`. At a lake that floods everything under the level inside the tile: the valley beyond the sill of an overflowing lake (lower than the sill by construction), a second basin, a dry hollow. The module names this limit and the per-vertex mask as its fix. Options:
- (a) A per-tile water mask, one bit per local cell, run-length coded, only for the tiles that list a lake: computed at bake time on the final surface (`B + L`) as the cells under the level that are the lake's (the connected set from the producer's water cells), stored beside the basin table (a second record kind, or the table's tile entries: a decision for the architect). The pass draws where the mask is set and `W - h_morph > 0` (the mask gives the edge at the 102 m grid, the height refines it). Cost: a few MB planet-wide (lakes are about 1 % of the land), exact, no runtime flood.
- (b) A bounded flood at plan time per lake tile over the vertex heights under the level, seeded at the cell of the lake's floor point or at the tile edge cells shared with a neighbour: no new store, but a lake spanning several tiles needs seeds per tile and a flood that agrees across tile edges at every LOD; this is the part that has failed before (seams).
- Recommendation: (a). Needs from the architect: (a) or (b), and where the mask lives (kind 20 or inside kind 19).
Rivers (PR 6 onwards) come after the switch and use the same pieces: the river's drawn width is the one law of D6 point 2.

## 11. PR 5, the sampler switch: scope and test list (on paper, 2026-11-10)
Scope read from the code: the fill depth is in six places: the fill channel (`LayerFill` kind 17, `layer_fill.rs`, `layer_fillpass.rs`, `generators/bake/fillgeom.rs`), the tile reader that adds it (`layer_tiles.rs` `with_fill_added`, the result's `fill` plane), the channel reader (`layer_channels.rs` `fill_depth_m`, the `ChannelSource` trait in `generators/channels.rs`), the driver block word (`erosion.rs` `fill_m`, GEN-119/120), the bake examples (`bake_layer`, `fill_gate`, `rock_stats`) and their tests (`cache/tests/layer_fill`, `layer_channels`, `generators/tests/runtime_layer`, `render/tests/gpu_shading`). The switch changes the layer source the viewer reads from the old `StoreLayer` (codec 2) to L (`load_l_tile`, codec 3) and deletes the fill path; the old layer's writer (`run_pass2`) stays until the new bake writer (PR 3) replaces it.
Split (lane): PR 5a the new source and the read (a `LayerSource` over `load_l_tile`, behind the viewer's `--erosion` switch, both sources selectable); PR 5b the deletion (fill channel, readers, examples, kinds 17 and 18 numbers kept reserved). 5b only after PR 4 (the dry hole) and after G-SURF holds on the new source.
Tests, 5a (tier A unless stated; TEST-108 continues, ids from the planner):
- T1 the source returns the L tile's core as the layer change: equal to `decode_change_core` of the stored stream, any tile of a fixture store.
- T2 a missing tile reads as a change of 0 (no land), a record of the previous layer is an error naming both versions (STRM-081), a tile under another base is an error.
- T3 `S = B + L` at the cell centres of a fixture tile equals the stored heights within the quantisation step (the C1 read at a node returns the node value): G-SURF at the centres on a fixture, no fill term.
- T4 the decoded-tile cache bound and the one-decode-per-tile property hold as for the old source (`DecodedTileCache`).
- T5 (tier C, WARP) a golden of the viewer on a fixture bake: a basin shows as a dry hole until PR 4 (the picture names it), no fill added back.
Tests, 5b: the deleted modules' tests go with them; `cargo xtask spec-lint` stays green by retiring GEN-119/GEN-120/STRM-076/STRM-077 requirement text in the same PR (status retired, with the successor named); a grep gate in the PR body (no `fill_depth_m`, `with_fill_added`, `KIND_LAYER_FILL` left).

## 12. Rivers (PR 6 onwards): design notes (on paper, 2026-11-10; A3 sections 3.3, 5.2; D6 point 2)
State: K6 gives segments (`up`, `down`, `cells`, `q_up`, `q_down`, `downstream`; centrelines by the receiver chain) and `Q_w`; the producer cuts them at lakes into pieces (PR 1, 1b); K7 burns a channel into the surface as an 8-connected line of cells lowered to the segment's profile (`k7::burn_channels`: one cell wide, no width law). D6 point 2 makes the width law `w = a * Q_w^b` one law for the burn-in and the drawing; today there is none in the burn-in.
| # | PR | What | Gate |
|---|---|---|---|
| 6 | the width law in K7 | `burn_channels` lowers the cells within `w/2` of the centreline (a trench of the law's width, bed level from the profile, banks not raised); `a`, `b` in the K7 key; the PR states the values and where `a` comes from (the burned widths of the existing trenches: measure on R3a/R3b) | K7 stage manifest moves (declared), G-PROFILE on the fixture tile: no rise along a segment above 0.25 m |
| 7 | the channel graph in the store | the segments (and the pieces at lakes) per K6 key unit: a new record kind (the architect reserves the number), sorted by segment id: id, up/down cell ids, `q_up`, `q_down`, downstream id, the cell chain as global cell ids delta-coded; the pieces beside it | round trip, refusals as the basin table; size at k=4 (segments 6.4e6) stated |
| 8 | the river drawing | a ribbon or a water-pass mask along each segment of a tile at the law's width, water level = the segment's profile, through lakes none (the pieces); the tile's rivers read under the same `commit_id` as L and the basin table | G-PROFILE along every segment through the complete read, C off and on; the view from 2 km at three rivers |
Risks: (1) the width law re-keys K7 and moves L, so it lands before any store of L is built for real; (2) a river drawn at the node profile floats over a diagonal link the C1 read lifts by design: the width law covers the reported rise (A3 3.3); (3) segment count at k=4 (6.4e6, 65 per 1,000 km2) makes the store the largest per-unit table: it needs a size gate before PR 7 merges (G-STORE style, measured on a fixture and extrapolated with the land-cell count named).
Needs from the architect: the kind number for the channel graph, and `a` and the way to measure it (my proposal: the median of the burned trench widths on R3a and R3b under a fixed `b = 0.5`, stated in the PR).

## 13. The lake water mask: the bound of the wet rule (architect D17/D18; on paper, 2026-11-10, for the architect's review)
Home: an optional section of the kind-13 v3 tile record (reserved empty in PR 2b), only for tiles that touch a lake; the bake writer (PR 3) fills it. Runs of wet cells carry an index into the tile's basin-id list, which doubles as the tile-to-lake index.
Grids: the H1 grid is the erosion grid (face edge 12,288 cells, 814 m); the L grid is 8 times finer (98,304 cells, 102 m); a local cell has one H1 parent cell (integer division by 8 on both axes). K7's window is the 1,024-cell core plus a halo of 128; the surface is `zl = B + L` on the window after K7.
The rule, per lake `b` with level `v` (the table's level) and per tile:
1. **Water blocks**: the set of local cells whose H1 parent is a water cell of the lake. Water cells are the producer's (`lakes::Water`: the basin cells under the level and the shore cells under the level that drain into it, PR 1, 1b). This is the bound: nothing outside the blocks of the lake's own H1 water cells, except step 3's ring.
2. **Core wet**: a cell of a water block with `zl < v`. No connectivity test is needed inside a block: the block is part of the lake's water at H1 resolution, and the finer surface only decides where the edge falls.
3. **Ring wet** (architect D19/D20): a cell with `zl < v` reached from a core wet cell through at most 8 steps (8-neighbours) over cells with `zl < v`, taking a step only into a cell whose H1 parent drains into this lake (the lake's catchment: its receiver chain reaches the lake's water) and is not a cell of another basin. The ring lets the lake edge lie up to one H1 cell (8 local cells) beyond the blocks where the coarse cell was larger than the shore; the catchment test keeps the outflow valley dry (the cells below the spill drain away from the lake) and keeps the valley sides that drain elsewhere dry.
4. A cell in two lakes' rules goes to the lake with the lower id (one label per cell).
Why this bound: every decision looks at most 8 local cells from the cell (the ring depth), and the halo is 128, so two tiles that share an edge or a corner compute the same label for the cells they share, in any tile order and on any thread count: the rule is a pure function of the window. A connectivity rule over the whole component (the first wording) would depend on the window's reach and fail the edge test for a lake wider than a window.
What each case does: the valley below an overflowing sill is downstream of the spill, so it is not in the lake's catchment, is not a water block and is excluded from the ring: dry. Another hollow inside the tile: not a water block, and the ring cannot reach it unless within 8 cells of the lake: dry. An endorheic lake with level under its sill: only `zl < v` cells of its blocks and ring: wet up to its level. A shelf under the level that H1 did not label: its H1 cells are shore cells of the lake (PR 1b): water blocks.
The consumer: the water pass draws a lake tile's grid where the mask is set and `W - h_morph > 0`; the mask gives the edge to the 8-cell ring of the exact line, the height refines it. A tile with no mask section behaves as today.
Encoding (the writer's, not a version): per tile, per row, runs `(start u16, length u16, label u8)`, the labels an index into a per-tile list of basin ids (the `u64` ids of the table, at most 255 a tile, more is an error naming the tile); rows with no run cost one length byte; the section is brotli-free (runs compress poorly as bytes and are small). Estimate for Earth at 102 m: lakes hold about 1 % of the land (1.4e8 local cells), a lake edge 100 cells long gives runs of about 50 cells, so about 3e6 runs at 5 bytes = 15 MB; a big lake gives few long runs. The fixture measurement states the real figure (budget: under 0.1 GB, A3 section 10).
Tests (tier A, `generators` or `cache`; TEST-108 continues, ids from the planner):
- M1 the valley below an overflowing sill is dry: a bowl with a sill and a valley below it at the lake's level or lower; no wet cell downstream of the spill; and the valley-sides case: cells within 8 steps of the lake with `zl < v` whose H1 cell drains to another basin or the outflow are dry.
- M2 another hollow in the tile, inside the window but outside the lake's blocks and ring, is dry even when its cells are under the level.
- M3 an endorheic lake is wet only up to its level: every wet cell has `zl < v`, and a hollow of the basin above the level is dry.
- M4 tile edges and corners agree: the labels of the cells two tiles share are equal, for a lake that crosses an edge, and for the 2 x 2 corner case; the tiles are computed in every order.
- M5 one thread and eight threads give identical masks (CON-14).
- M5b the diagnostic (reported, not gated): the count and the area of core-wet cells that are not connected to the largest wet component of their lake in the tile window, on the fixture region.
- M6 the encoding round-trips and the byte count on the fixture region is stated against the budget.

## 14. PR 3, the bake writer: design (on paper, 2026-11-10; ids GEN-167, STRM-082, TEST-109 from the planner; for the architect's review)
State after PR 1 to 2b and D22: the producer gives lake records and river pieces (`Lakes`), the basin table (kind 19) and L (kind 13, layer version 3, empty water section) have formats; nothing writes them from a bake. `build_resident` drops the routing (`receiver`), `q` and `qw` at the release; K7 (`k7_tile`) takes `Channel`s (points in face cell-index space, bed levels, `Q_w`), which `examples/k7_region` builds from a held-in-memory H1 (`receiver` chains, `hy.qw`).
**What the writer takes** (one function per layer of the stack, no new kind):
| Input | From |
|---|---|
| `B`, the regional change | the base function and the regional field (as `k7_region`) |
| channels per window | a `ChannelFeed` trait: `channels_near(window) -> Vec<Channel>`; PR 3 implements it over the in-memory H1 result (fixtures, regions); the planet-scale feed is PR 7's channel graph (open point 1) |
| lakes with tiles | `lakes_with_tiles` (the table), plus the lake water list below |
| the world, the specs | `GlobalWorld`, `LTileSpec`, `BasinSpec` |
**New producer output (GEN-167)**: `Lakes` gets two sorted lists, produced in the pre-release gap beside the pieces (they read the routing): `water: Vec<(cell u32, lake id)>` (the producer's water cells: basin cells under the level and the shore cells that drain into it; not the dead ground) and `ring: Vec<(cell u32, lake id)>` (the H1 cells 8-adjacent to a water cell of the lake that are not water, not in another basin, and drain into this lake along the receiver chain: the catchment test of D20 done at H1 resolution where the routing exists; a cell is in the ring of the lake its chain reaches first). Both are at most the perimeter and the basin cells: about 1.5e6 and 1e6 entries at k=4, 8 B each: about 20 MB, inside the D26 bound (pieces 35 MiB + water set). With these the mask needs no routing at bake time of L.
**The mask (GEN-167, pure, `generators::bake::water_mask`)**: `lake_mask(window zl, window origin, parent: &dyn Fn(cell) -> lake status) -> labels`: per local cell the H1 parent is `(i / 8, j / 8)`; status = Water(lake) | Ring(lake) | Dry via binary search in the two lists (or a per-tile hash built once from the lists restricted to the tile's parents: the tile touches at most 130 x 130 parents). Rule as section 13: core wet = Water parent and `zl < level`; ring = up to 8 steps through cells with `zl < level` whose parent is Water or Ring of the same lake; lower id wins. The window needs 8 cells beyond the core: any halo of at least 8 serves (`k7_region` uses 64, the design 128).
**Encoding** (section 13): per tile `[n_lakes u8 | lake ids u64 x n | per row (run count u16, runs (start u16, len u16, label u8))]`; 5 bytes a run; refused above 255 lakes a tile or a run list over `MAX_WATER_SECTION`. The tile's water section is empty when no cell is wet.
**The commit (STRM-082)**: the content keys already say what is read together; `commit_id` is a record naming them. A `CommitRecord` (kind 20 is for the channel graph; this uses the layer manifest, not a kind: open point 2) is written LAST by the writer: `commit_id` (u64: hash of the L key identity, the basin table keys of the six faces and the base id), the six basin-table keys, the L spec bytes, the layer versions. A reader opens a commit and reads L tiles and tables only under the keys it names (`load_l_tile` already refuses another key); a tile or table of another commit is therefore never composed with it (D17's test is in PR 5a). The writer stamps nothing inside the tile records (their key is the content key); a bake that is interrupted leaves no commit record, so the previous commit stays the current one.
**The write order**: (1) the six basin tables (they depend on `Lakes` only), (2) L tiles in tile order with their water sections (K7 per tile, then the mask from `zl = B + L` of the window, then `l_tile_record`), (3) the layer manifest's versions for L, (4) the commit record. A tile whose K7 result touches no lake (no parent in `water` or `ring` within the window) takes the empty section without running the mask.
**Memory**: a tile's window is 1,152 x 1,152 f64 (10 MB) plus labels (1.3 MB); nothing is kept across tiles but the sorted lists; the tiles run in parallel at the K7 level (threads as `k7_tile`).
**Tests (tier A, TEST-109)**: M1 to M6 of section 13 on a synthetic grid (a `lake_cells` fixture with a known sill, the valley below, a second hollow, an endorheic lake), run on windows cut from one analytic surface at every tile offset to prove tile-order and partition independence (M4) and at 1 and 8 threads (M5); M5b the diagnostic printed by an example on the fixture region; W1 the writer round trip on a small fixture world (tiles, tables, commit read back; the table's lakes equal the producer's; the L water section decodes to the mask); W2 an interrupted bake (no commit record) reads as the previous commit; W3 refusals (a lake list over 255 a tile, a corrupt water section) name the tile; W4 the writer's output is deterministic (the same bytes at 1 and 8 threads).
**Open points for the architect**: (1) the channel feed at planet scale: `build_resident` keeps neither `receiver` nor `qw`; the K7 channels need the segment chains and `Q_w` along them. Choice: (a) PR 3 takes a feed over an in-memory H1 (regions and fixtures only), the planet-scale feed is PR 7 (the stored graph); (b) `build_resident` keeps the segments' chains and `Q_w` at the cells of the chains before the release (about the channel cells times 12 B, hundreds of MB at k=4: against D26's margin). Recommendation: (a). (2) the commit record's home: a small record in the layer manifest's directory (an atomic text file, as `LayerManifest::write`) or a new kind (not 20). Recommendation: the manifest file, since it is already the atomic last-written part of a layer. (3) where the six-face `face_of` of a lake is taken from (the face of its floor cell, as the table says).
**Units (architect D33)**: the writer takes `Q_w` in m3/s only. `Channel::q` is a discharge (the width law turns it into metres); the writer's feed refuses a hydrology that records an area for Q (the flag the explorer adds) and states the unit in its signature and doc. PR 3 lists, for its callers, which of `Hydro::q`, `qw`, `Basin::q_in`, `Basin::q_out` and `RiverPiece::q_first/q_last` it reads: the basin table takes `q_in`/`q_out` and the pieces take `qw`, all m3/s when H1 ran with runoff and with a unit-less area when it did not (the standing test fixtures).
