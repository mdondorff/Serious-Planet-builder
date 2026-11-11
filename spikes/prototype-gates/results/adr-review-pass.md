# ADR review pass: A3, the briefing and ADRs 0110-0115 against the ADRs, specs and code on main

Status: read-only review, 2026-11-10 (briefing item 1, "a further ADR review pending"; bulldozer inbox #1 of 2026-11-10). Main at 59a7deaa. No code run, no new ADR, no requirement ID. Verdicts are the architect's.

Scope: `docs/design/terrain-target.md` (A3, cited "A3:n" by line), `docs/briefings/terrain-prototype.md`, `docs/adr/0110` to `0115`, and every accepted ADR, CON rule and requirement they cite (about 60 ADR citations, about 72 requirement IDs). Method: the reviewer read A3, the briefing and ADRs 0110-0115, and checked the claims against the cited ADRs, specs, `docs/gates/ledger.md`, the open changes' designs and the Rust code. Three read-only sub-agents did the bulk lookups (ADR citations, requirement IDs, code facts); the findings marked **V** were re-read by the reviewer in the source. This reviewer built none of items 9 to 15.

## Result

A3 and the ADRs are coherent and their measured numbers are right (every number checked against its design, spec and ledger row matched: the 7,858 and 8,854 MiB rows, 55 and 62 bytes a cell, 26,041 of 26,265, 65 tile floods, 0.66 %, 11,363 against 11,444 MiB, the grid sizes of section 4, the store arithmetic of section 10). The problems are of four kinds:

1. **Three amendments of accepted ADRs and one constitution rule are not listed or not honest** (section 1).
2. **A3 has fallen behind the architect's decisions of 2026-11-10** (lake producer, width law, `L` and kind 19 layout, lake water mask) and the merged item 15 work (section 2).
3. **Statements about code that are not true on main** (section 3).
4. **Stale state in the briefing and `docs/status.md`, and wording that can be read two ways** (sections 4 and 5).

No amendment as worded edits an accepted ADR in place (CON-23, ADR 0078); every one is made by a new ADR.

## 1. For the architect's verdict (contradictions and unlisted amendments)

| # | Where | Claim | What the source says | Why it matters |
|---|---|---|---|---|
| 1 | A3:273, A3:30, ADR 0113 point 7, A3 §13 ("CON-08 reading") | the regional solver is read as allowed by CON-08 | `docs/constitution.md` CON-08: regional = "per tile or per chunk of a fixed grid, with a bounded halo". A region is a land-mask component of any size (A3:273) with no fixed grid and no bounded halo; K5 reads K6's previous round (A3:30, "the one feedback loop"). **V** | A reading cannot supply a bounded halo. ADR 0077 point 6 also called itself a reading, and the constitution was then amended (v1.6). CLAUDE.md: stop when the constitution would have to change. CON-08 also still names "chunk" (ADR 0086 removed it). Needs an owner amendment proposal, not a reading |
| 2 | A3:94, A3:40, ADR 0110 point 4 | K7 runs "the breach (SPIKE-104 D-A) at the local grid" | ADR 0088 point 4.2 (0088:21): pass 2 is "a local carve ... that only incises: no fill, no breach, no uplift". 0088 point 4.2 is in no Amends line (A3 §13, 0110, 0112). **V** | An unlisted amendment of an accepted point. 0110 point 2 retires "pass 2's stride-4 base", which also lives in 0088 point 4.2, not in the points listed (2, 5) |
| 3 | A3:446, A3:449, ADR 0113 points 1 and 2 | constructed features stay above erosion as "ADR 0089 point 11's override stage"; height items (sculpt, raise, lower) move below erosion | 0089:24: the override stage holds "height edits" ("height edits already do"). Moving height edits below erosion departs from 0089 point 11. 0089 is in no Amends line. **V** | Unlisted amendment |
| 4 | A3:210, A3:257, ADR 0111 point 7 | an endorheic basin by water balance, "ADR 0105 point 2" | 0105:14: "where aridity is below a threshold, a kept basin is a sink". 0105:15 point 3 (the fill-depth channel keeps its meaning) is overridden by 0110 point 5 and 0111 point 9. Neither point is in an Amends line. **V** | The rule differs, and the citation reads as support. ADR 0111 does list 0101 but not 0105 |
| 5 | A3:451, ADR 0113 | amends 0066 points 1 and 4 only | 0066 point 5 bounds tiles by `reach` and `max_delta_m`; point 2 makes the footprint the invalidation unit. Items below erosion change `B` far beyond reach; A3 states no bound rule for them. 0066 point 4 is already amended by ADR 0068 | Points 2 and 5 are affected and unlisted |
| 6 | A3:639, ADR 0112 consequences | ADR 0109 and ADR 0105 point 1 "superseded by 0112 if the gate passes" | Both accepted. 0109:16 and 0105:2: deposition no longer fills basins and stays for fans, deltas and valley floors. A3 §6.7 keeps deposition as a post-round pass, so the process is moved, not superseded; ADR 0112's seven points say nothing about deposition, the infill or the sediment pass; the replacement of the infill sits only in 0111 point 5. The interim state (which rule governs until the gate) is not stated | A supersession with no decision text |
| 7 | A3:112, ADR 0115 point 1, A3:637 | "k follows from the target local cell size: the largest k whose local cell is at least the target (ADR 0045 point 2 made normative)" | 0045:23 (a **proposed** ADR): `res = 2^ceil(log2(face_edge / cell))` from the bake cell (default 0.6 km): a ceil, so the grid is at most the target cell, on the bake cell not the local cell. A3's rule is a floor on the local cell and gives a different k. With a 100 m target Earth gives k = 14 (101.8 m); with "102 m" as the table prints it, k = 13. A3 §13 says "Amends", 0115 "becomes normative"; 0045 is edited in place (0088:36) or not, nothing says which | A different rule is called normative. The target cell size and the tolerance (101.8 against 102 m) are not defined |
| 8 | A3:539, ADR 0114 point 4 against the code | "K7 and pass 2 ... Round surfaces are f32, narrowed once at the end of a round" (ADR 0114 point 4, A3 §9 contract) | Main is f64 across rounds everywhere: `bake/round.rs:263-267`, `:439-442` write `z[c] = v` (f64); `solve_rounds` takes `&mut [f64]`; `build.rs:193`; the G-ROUND reference `carve_rock` is f64; the memo key hashes f64 bits (`round.rs:374`). No narrowing exists and no test narrows. The A3 sentence "today's bake holds the carve in f64" is accurate; the contract is unimplemented | Every G-ROUND, G-PART and G-DET result so far holds for f64 rounds. G-EQ's f32 form is unmeasured (ledger: G-EQ partial). The "fresh build = re-solve" claim for the stored form has no evidence yet |
| 9 | A3:310 and A3:491, 0112 | the two kernel rules "carry a kernel version bump"; `RoundRules` "default off" | `BAKE_KERNEL_VERSION` is 5 (`bake/mod.rs:9`); `openspec/changes/round-kernels/design.md` decision 1: "the kernel version is not bumped"; the layer key gets bytes `0xC4, clip, canonical` only when a rule is on (`cache/src/layer_store.rs:73`). A3:314 says "key byte only when on", so A3 contradicts itself. `RoundRules::default()` is off, but `PlanParams::standard` (`build.rs:55`) turns it on and the shipped layer bake (`layer_tiles.rs:343`) has it off | A hash-compatibility statement. The prototype path and the shipped path run different kernels |
| 10 | A3:599 (G-SURF), A3:62 | pass: "0 before quantisation; at most 0.25 m after" | GEN-154 and the ledger: at most 1e-9 m before and half a step (0.125 m) after. A3:62 itself says "half a step" | The gate row is looser than the spec it quotes. BUILD-040 makes a change of the row a contract-change line |
| 11 | A3:189, A3:225, A3:601 (G-CON), A3:226 | a channel "passes through a basin ... inside a lake it is a line under water"; "a channel never ends mid-course ... through a basin"; segment ends are "confluences, heads, outlets" | GEN-163 (lake-producer) and the architect's decision of 2026-11-10 (`runtime-composition/design.md` section 7 row 5): the producer drops channel cells inside kept lakes and cuts segments into pieces that end at the last cell before the water and restart at the spill cell; the labelled cells below an endorheic lake's water carry no river. Briefing item 15: "channels end at lake entries". The ledger's G-CON rows ("0 channels end mid-course") were taken before the pieces. **V** | A3 and the gate text still describe the other design. A literal G-CON on the produced pieces does not hold (a piece's receiver is a dropped cell). The segment id rule has no lake end |
| 12 | A3:94, A3:97 | burn-in lowers cells along an 8-connected line | GEN-165 (#1324, merged): a trench of `max(1 cell, w)`, `w = 7.2 Q_w^0.5`; design notes section 7 row 2. A3 has no width law | Contract text behind the code. The width law re-keys K7 and moves `L` |
| 13 | A3:398 against the sediment table A3:407 and briefing item 10 | thermal and pit spreading "run per tile with a halo, keyed per tile, after the last round (as K7 does today)" (A3:398, ADR 0112 point 4) | The same section's sediment table (A3:407): "Today it runs on a whole grid and has no tile or catchment borders"; briefing item 10: GEN-155's partition test is groups of whole catchments, "a spatial cut of the pass stays open" | The per-tile claim is design, and untested. ADR 0112 point 4 states it as decided |
| 14 | A3:376, ADR 0115 point 7, briefing item 9 | "12 GB", "the process's peak" | Not stated: decimal or binary (the briefing's 11,444 MiB means 12e9 bytes, decimal); commit charge or working set (ledger: peak commit 11,363 MiB, working set 10,861 MiB, a difference of 502 MiB; the gate's margin is 81 MiB; the spread between two runs is 232 MiB). Two runs, spread above the margin. The briefing already calls the margin thin | A pass defined by an unstated unit and metric, inside the noise |

## 2. A3 behind the work of 2026-11-10 (stale, not contradicted by anything but A3)

| A3 | Says | Now |
|---|---|---|
| A3:367-372, ADR 0115 consequences, ledger G-SCALE rows | the H1 pass "not run at 12,288 cells: 12.1e9 bytes extrapolated, at the budget" | measured: complete resident build on the phase plan, worst of two runs 11,363 MiB commit (GEN-158, #1287); the 7,858 MiB row is the rounds only. A3 has no complete-build row |
| A3:62 | "Not done: the stored `L`" | `L` stored as kind 13 and measured (GEN-154, GEN-156; #1315, #1316) |
| A3:546-558 | store rows "to measure" | the four A3 section 10 rows carry values (ledger G-STORE, 6b5bb0ac): field bake 7.00 GB, round records 2.7 GB (A3: about 4 GB) |
| A3:64 | a switch "waits for G-BASIN and the lake producer (K11)" | the producer exists (GEN-163) and the basin table is stored as kind 19 (STRM-080); nothing in `frame` or `render` reads either; the fill is still added back at tile decode (`layer_tiles.rs:594-599`). The lake water mask (design notes section 13) and kind 19 are in no A3 section |
| A3:37, A3:2 table (marks N and X) | K4, K6 graph and lakes, K7 marked N or "extended" | `bake/h0.rs`, `bake/hydro.rs`, `bake/lakes.rs`, `bake/k7.rs` exist; none is in the shipped bake (`generate` drives the old pass), the channel graph is not persisted. "Exists in generators, not yet in the shipped bake" |
| A3:117 | "`Grid` today has one `dx`" | `Grid` carries a metric (GEN-138); pass 1's default, tile and K7 paths still run on `dx` (`carve.rs:76` `cell_metric = false`, `tile.rs:295`, `k7.rs:260`) |
| A3:69 | pass 2's stride-4 window "is retired" | true of `k7_tile` only; `tile.rs:284` still calls `sample_window_coarse(..., BASE_STRIDE)` and the shipped path (`layer_tiles.rs:704`) uses it |
| A3:397 (GEN-162) | the breach "never cuts below its chain's exit ... on land and at sea alike" | GEN-162 applies only under the local clip of the round rules; a chain ending at or below the sea has no base (floor = sea level + epsilon); K7's breach, the sediment pass and the default rules keep the old breach |
| A3:4 | evidence "F1-F6" | the text cites F7, F8 and F9 |
| A3:626 | "The planner judges numeric gates" | `docs/routing.md`: gate verdicts are the architect's; opus-planner is retired |
| A3:641-649 | "Open items for the review" 1 to 8 | written for the external reviews; items 1 to 5 are G-gates, 7 and 8 are decided or ruled (toolchain pin: ADR 0114 point 6; tile-wise route: A3:378) |
| A3:628 | "numbers reserved" | the files exist, status proposed |
| A3:514 | the determinism manifest covers "tile hashes" | it covers the 64-cell global bake, terrain and erosion heights, meshes and keys (`generators/src/manifest.rs`, `app/src/manifest.rs`); not pass 1, rounds, H1, K7, lakes. The "open" is right |
| A3:508 | "no `std::arch` ... in `bake`, `erosion`" | true in those modules; `generators/src/horizon.rs:150-180` has a runtime-detected AVX2 path (outside the terrain path; no bit-match test was looked for) |
| A3:512 | HashMap "for lookups only (the memo)" | `bake/memo.rs:89`, `bake/hydro.rs:467-475`, `bake/spill.rs:126-140` iterate a HashMap and sort before use; deterministic, but not "memo only" |
| A3:539 | `bake/tile.rs:322` | line 324 |

## 3. Statements about code and specs that do not hold on main

| A3 | Claim | Fact |
|---|---|---|
| A3:34, A3:274, A3:489 | a `SimulationProfile` in K1 holds rounds, steps, `K dt` and the rest | no such type in the tree. The values are `CarveParams.k_dt = 0.05`, `reroute_every = 5`, `steps = 20` (`carve.rs:64-66`), `PlanParams.rounds = 4` (`build.rs:54`); `check()` asserts `steps == rounds * reroute_every` (`round.rs:213`). `WorldDefinition` has no target cell size or hydrology switch. "Round count in the profile" is design, not code |
| A3:112 | grid counts "are never constants of the code" | 51 matches of 12,288, 98,304, 3,072 or 16,384 in 18 `src` files, all in the shipped bake and its perf and acceptance tooling: `layer_tiles.rs:346` (`BakeShape::production`), `:362`, `tile.rs:24` (`BASE_STRIDE = 4`), `streamperf.rs:24`, `perf/src/accept*.rs`, `lithology_preview.rs:155`, `relief_channels.rs:17`. None in the new chain (`h0`, `hydro`, `lakes`, `round`, `partition`, `k7`) |
| A3:56, A3:75 | the C1 read is "clamped as today"; the clamp is in `base_id` | GEN-107 (c1-base-read) has no clamp: "rounded to f32 once", bounds widened by 0.28125 m; `read_elevation_c1` (`global.rs:960`) has none. The clamp is on the `L` read (GEN-112, four cells) |
| A3:57-58 | `k` only scales the crease amplitude | `erosion.rs:529-536`: with the relief channels on, the code computes `m + k*(e - m) + k*detail`, so `k` also blends `B` toward the ring mean `m`. The runtime `B` term is not the pure `read_elevation_c1` of A3 §3.1 step 1 |
| A3:97, A3:57 | creases are octaves "below the local grid" | GEN-109: the band drops crease octaves "above the erosion layer's grid" (at most 2 layer cells kept, 4 or more dropped, smoothstep between), with the relief channels on |
| A3:208 | the artefact limit `min_depth_m` is "in cells" | hydrology-h1 design: `min_cells` 4 and `min_depth_m` 2 m (metres). A3 gives no value for `min_cells` or `min_depth_m` and does not relate `min_depth_m` to `pit_min_m` (0.5 m, `hydro.rs:41`) |
| A3:548 | "5-8 GB (narrow channels, ADR 0045 point 4)" | 0045:25: encoding "decided by a spike", the gain "3-6x", unmeasured: 2-4 GB from 11.7 GB. Measured by the ledger: 7.00 GB with deep-ocean tiles, 1.18 GB narrow |
| A3:501 | "ARM and macOS are excluded until they are added to CI (CON-26)" | CON-26 says "Windows-native; Linux by CI; WSL not part"; ADR 0010:21 says aarch64 and macOS are "expected to match" but untested |
| A3:46, A3:583, A3:473, A3:37, A3:568, A3:589 | ADR 0014 and 0086 "never select or drive a simulation", "ADR 0086's lesson", ADR 0072 point 3 for commits, "the coupled run of ADR 0077 was a predecessor" | 0014 is CDLOD selection and says nothing on simulation; 0086 states no lesson and keeps the global coupled run (0086:14, 18); 0072 point 3 covers a re-keyed tile, not a commit (A3:472 says commits are new); the 0072 preview attribution is one line. Citations that read as support and are not |
| ADR 0112:11 | "smallest erosion unit today is a whole face (10 GB, ADR 0099)" | 0099 has no 10 GB figure (0099:21: about 1.6e8 cells) |
| A3:610 and the G-INC row | "E2 at most 10 % of a full routing" | the unit is work (tiles re-flooded); the measured wall time is 2.2 times a resident route (A3:347: 1.58 s against 0.71 s; ledger 6.5 % and 9.8 % of tile floods). A3:440 admits the memo "has not yet paid". The pass criterion can be met by an edit that is slower than a full re-route |
| A3:608 (G-SED) | pass = volume balance and partition identity | passes a thermal pass that moves a column a long way: briefing item 10 reports a thermal maximum of 609.7, 920 and 1,249.1 m (k=1, 2, 4), "evidence for the A/B gate, no fix ordered". Nothing in A3 §6.7 or G-SED bounds the move |
| A3:603 (G-LOCAL) | S1 criterion; the key "holds everything a round reads" (A3:349) | no ledger row (`not run`). G-PROFILE and G-RT also `not run`. The catchment-round memo and its key are not built (A3:438, "by design"). For the A/B gate this is the claim no number supports yet |
| A3:3 header vs ADR statuses | ADRs 0110-0115 | 0115:2 and 0115:22 say the owner "sets" or "decides" point 7's budget; point 7 already records "owner, 2026-11-08 and 2026-11-09". 0115:23 and 0112:2 and 0110:2 give measured figures and changed gate lists in ADR text: CON-23 treats an ADR's evidence as fixed; these ADRs are proposed, so editing is allowed, but the status lines list different gates from the consequences (0110: G-BASIN against G-CON; 0112: G-SCALE missing) |

## 4. Briefing and `docs/status.md`

| Where | State | Fact |
|---|---|---|
| status.md "Queue (2026-11-09)", rows 27 to 29 | prototype items 9, 10, 11, 14 "explorer"; "second region and seed" to spikes; "item 15 waits for item 11" | items 9, 10, 11, 13 done (briefing); item 13 was crusader's; item 11 is done and item 15 is under way. The queue is dated 2026-11-09 |
| briefing item 3 and header | "the stored-field read is open" | #1317 (h0-stored-read, GEN-164) merged |
| briefing item 14 and header | open sub-items "L on a second region, deep-ocean field tiles, temporary space, edit overrides" | #1294, #1315 and #1316 merged them before the 14:50 stamp (the stamp is also later than the last commit of the file, #1314) |
| briefing item 15 | cites none of GEN-163, 164, 165, STRM-080; "order lakes, sampler switch, rivers" | the lake producer (#1313, #1318), the basin table (#1321) and the width law (#1324) are merged |
| briefing item 8, item 16 | E1-E6 | the edit-cases change covers E1-E7 (E7: land bridge); A3:617 requires every gate after E1-E7 |
| briefing item 18 | "#1282 to #1289" | #1283 is k7-canonical (GEN-160), #1284 a docs status entry, #1287 phase-plan (GEN-158); item 18's PRs are #1282, #1285, #1286, #1288, #1289, #1299 |
| briefing header | "16 and 17 open" | item 16 is "in progress" in the table |
| briefing "Contract changes" | four lines of 2026-11-09 | the architect's decisions of 2026-11-10 ( lake producer at bake time, width law, `L` as kind 13 version 3, channels dropped inside kept lakes; kind 19, kind 13 v3; the lake water mask) live only in `openspec/changes/runtime-composition/design.md`. BUILD-040 checks only the 12.1 gate rows and the Owner decisions table, so nothing failed. PR 5b retires GEN-119, GEN-120, STRM-076, STRM-077 (the fill channel of accepted ADR 0101 point 3) on the strength of a proposed ADR (0110) |
| openspec | GEN-094 to 099, 107, 110 to 114, 121, 129, 138, 139, 141 to 152, 154, 155, 158 to 160, 162 to 165 | requirement text lives in open changes with Status "planned" or "active", not in `openspec/specs/`; the open changes still hold `design.md`; GEN-143's text still says runoff weights are not supported (they were added with GEN-146) |

## 5. Wording that can be read two ways

- **`k` means three things.** A3:79 and A3:112 (grid exponent), A3:58 and A3:82 (the relief driver, GEN-099), and the lane runs' "k=4" (rounds: briefing items 9 and 10, `runtime-composition` "segments at k=4"). A3:79 and A3:82 are two lines apart in the same list. Suggest `kgrid`, `krelief` and `rounds=4`.
- **Amends against extends against replaces.** A3 §13 and the ADRs' own lines differ for 0101 (A3 "amends point 3"; 0110 "replaces point 3"; 0111 "amends points 2 and 3"), 0041 (A3 "producers"; 0111 "extends"), 0010 and 0003 (A3 "amends"; 0114 "extends", "uses"; nothing in either changes), 0088 point 4 (A3; 0112 "point 4.1"), 0045 (A3 "amends"; 0115 "becomes normative"), and 0086 point 3 (0113 "settles", missing in A3). ADR 0110's "amends ADR 0099 point 3" does not say what changes (0099:15 is the clamped C1 read, which A3:57 and A3:94 keep); "amends ADR 0100" has no points.
- **CON-22** is cited for "draw the previous commit until the new one is ready" (ADR 0113:18, A3:471); CON-22 is "streaming never blocks a frame".
- **GEN-129** does not name `discharge_weight`; the spec gives the numbers (300 mm, 0.02 to 8) and the name is in code only. GEN-145 does not say that touching basins are one component (design item 8(b)); the exit filter "within 256 cells" of the lake balance (design item 9) is absent from A3:244.
- **Thresholds without values.** `min_cells`, `min_depth_m` (section 3), the initiation `T` "50-100 km^2" against the runs' 75 km^2; the probe of SPIKE-106 found channel density 6.7 to 10.5 % of land across regions and `T` uncalibrated.
- **Basin base level.** A3:353 puts "its base level (sea or basin spill)" in a catchment key; a catchment's outlet is a region outlet, so a basin spill is not a base level there. Meant for the window's fixed outlets?
- **`base_id` is per tile** (A3:78: items "whose footprint meets the tile") while A3:81 folds it into the key of every derived layer, including region-wide K5 and K6. Which tile's `base_id` a region layer folds is not said.
- **"K7 only lowers"** (A3:94) is the burn-in; the same K7 then runs thermal and deposition, which raise cells; G-PROFILE (`not run`) asks for no rise above 0.25 m along a segment after all of K7.
- **A3 §6.7 "K7 reads the channel levels as boundary conditions"** against §3.3's burn-in, which is an initial condition.
- **Store target.** A3:558 estimates 26-38 GB; the gate's target is 35 GB (A3:610). The upper end of the design's own range is above its target.

## 6. What the review did not cover

- The kernels' numerics (route, flood, carve) beyond the facts quoted; the numerics-reviewer's remit.
- ADRs cited only by SPIKE-105 and not by A3 or the briefing; ADR 0016 and ADR 0074 are cited by none of the three files (the brief named them; nothing to check).
- The external review texts themselves (`SeriousPlanetData/owner-refs`), and the SPIKE-105 verdict tables.
- The A3 §11 preview rows and §7.4 against any code: no level-2 job, commit or `commit_id` exists in the editor yet.
- BUDG-005's "terrain 4 GB" value was not re-read (the requirement exists, Status: planned).
- Items 16 and 17 (editor path, noise of the base bands): no contract text to check beyond A3 §11.

## 7. Suggested order for the architect

1. Items 1 to 6 of section 1: the Amends lists and CON-08 (an owner matter, per CLAUDE.md).
2. Items 11, 12 and 13: A3 §5.2, §5.4, §3.3, §12.1 G-CON, §6.7 brought to what GEN-163, GEN-165 and GEN-155 build, with a contract-change line (BUILD-040) for each gate row touched.
3. Items 8, 9, 10, 14: decide whether the f32 canonical form is part of the prototype's contract (then G-ROUND and G-PART are re-run on it) or moves to the A/B gate; fix the kernel-version sentence; fix the G-SURF row; state the unit and the metric of the 12 GB.
4. Sections 2 to 4: a text pass by the architect for A3 and the ADR status lines; the bulldozer for the briefing re-estimate and the status queue.
