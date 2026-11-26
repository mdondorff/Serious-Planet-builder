# Parking the river-graph work: plan (Fable, 2026-11-26)

Read-only review. Sources: `origin/main` ebb01b7a0 (ADRs 0112, 0113, 0118, 0119, 0120, 0121; `docs/learnings.md` 19,979 B; `docs/routing.md` rule 10; `docs/parking.md`; specs GEN-143, 147, 189, 191; BUDG-003/005), `origin/spike/rg-prototype` 5e66aa104 (+38,170 lines, 142 files; 287 non-merge commits off main), the lanes digest, lane outboxes of 11-26, Carbon's `rg-parking-plan-2026-11-26.md` (read; where it differs, I say so). "inferred" marks my reasoning.

## 1. What to persist and merge to main
Rules applied: a result-changing stage lands switched off (ADR 0118 pt 2); no production PR for a look the owner has not flown; ADRs about one page (ADR 0055); numbers in specs; no "D<n>" channel numbers in the repo; `docs/learnings.md` is at its 20 KB cap, so a new entry needs an archive move first (ADR 0075).

| # | Item | Where it lives now | Form on main | Size (lane-days) | Helps B now? |
|---|---|---|---|---|---|
| 1 | **Wave-1 memory levers**: compact grid (GEN-147, already on main), K5's in-flight catchment cap (spike 5211557c0), K6 levers (a)-(h) (e8954dd12, 6c145ab95, ef569a5f7, 5e66aa104), `mem_stress` and `landmass_scale wave1` with the counting allocator | spike; proven bit-identical on five layouts and on 1, 2 and all threads; k=4 full count 9.26 GB (crusader 11-26 21:00) | one code PR to `generators` behind tier-A bit-identity tests; the cap and the lean K6 are result-neutral so they may land on (ADR 0118 does not bind); a GEN-143/147 detail line and a BUDG row with the measured 61 B a cell | 1.5-2 | **yes**: B's regional bake runs base1, K5 and K6; the whole-cube form was 72-89 GB |
| 2 | **base1's parallel decide** (825 s -> 206 s at k=1, same hash) | spike 5e66aa104 | with item 1; tier-A test on 1, 2 and N threads (CON-14) | in item 1 | yes, if the planet bake keeps base1 |
| 3 | **Cross-seam breach join** (owner per depression; local, join, apply; TEST-151 layouts) for base1, band6, KC | `drain.rs` (+1,190 lines), tests `cross_seam_kc/band6/k5`, design note `proposals/cross-seam-breaches-2026-11-25.md`; TEST-151 is on main | **tag plus archive doc now**; a code PR only when B's planet bake needs a partitioned H1 drain (the design note names the per-face single-piece form as the simpler option) | 0.5 doc now; 2-3 later | later |
| 4 | **Gate integrity**: connected sea (#1644), D10 bars and the 10 seeded windows (#1648), relief cap (#1651), width columns (#1659), look sheets (#1650), `prepush --declared` (#1663) | main | keep; nothing to do | 0 | **yes**: it showed B's R3a at 57.68 % undrained pits |
| 5 | **Metric calibration harness** TEST-155 / BUILD-069 | main (#1665) | keep; add the rule to `docs/routing.md` beside rule 10: no geometric bar is registered without passing it | 0.2 | yes |
| 6 | **Attribution method** (#184w: along-valley residual, H1-locked share over 64 phases, per-term counterfactual) | spike `spikes/look-attribution/results/raw/*.py`, digest `attribution-184w-2026-11-25.md`, `o16-base-fixes-scoping-2026-11-25.md` | a numbered SPIKE results folder on main (`spikes/look-attribution/` with README, scripts, the two notes); docs-only PR | 0.5-1 | **yes**: rule 10 needs the method, and B's look defects are S0's |
| 7 | **G0 contracts and readings 1-48**, the G2 record, the G3 gate and risk register, the snapshot | digest `g0-contracts.md`, `g2-report`, `g3-gate`, `g3-snapshot` | `docs/archive/P3/river-graph/` as frozen reference (contracts, G2 record, gate, snapshot, both feasibility reviews); the canonical graph GEN-189 and TEST-140 are already on main and stay | 0.5 | reference |
| 8 | **Net budget validator** (`validate_budget_net`, reading 47) and contract §6's net rule | spike `river_graph.rs`; main's BUILD-060 reads `budget-violations.csv` | tag; the rule text goes with item 7 | 0 | no (B has no per-stage budget) |
| 9 | **Intended basins** R23 (tectonic mark, aridity ceiling, two arid calibration draws, `calib_draw`) | spike; `r23-intended-basins-design-2026-11-24.md` | tag plus the design note in item 7; the calibration-draw method is reusable for any lakes work | 0 | later |
| 10 | **Trunk centreline** (minimax lowest path, 4-connected raster, channel-scale meanders), `rg_parts/curve.rs` | spike | tag only; its findings go to the decision record (§2) | 0 | no |
| 11 | **Partition proofs** for KC, band6, base1 and wave 1 (`wave_runner.rs`, TEST-151) | spike tests | tag; TEST-151's layouts are on main | 0 | reference |
| 12 | **Flight-store tooling** (`flight_store.rs`), the six fly1126 stores | spike; `tmp/fly1126` | tag; keep the six stores (about 180 MB) until the owner has flown once | 0 | the B stores show B's look |
| 13 | **Spike reports** SPIKE-127 to 144 | main | keep; SPIKE-143 and 144 get a closing line that points at the tag | 0.2 | - |
| 14 | **Lead rules of 11-25** (2 unreviewed switch-row generations per look item; Tier-A calibration before a bar; lanes idle on infrastructure when 3 or more owner decisions block) | gate §6 only | `docs/routing.md` rules 11-13; docs-only PR | 0.2 | yes (process) |

Stays only on the tagged branch: `rg_chain.rs` and its 96 switches, KH, KS0, KS1, band6, KG-trunk, KG-trib, basins-2, KV, KC, `kg_trunk.rs`, `kg_trib.rs`, `kh_*.rs`, `repair_debt.rs`, `rg_h1.rs`, the `rg_stages` moves of #208 step 4, R23's switch, K5' (`--k5 k5p`), the h1 pass dry tests. Tags (annotated, on `origin`): `rg/g2-record-2026-11-22` at faa923f04 (the g1f15 record), `rg/parked-2026-11-26` at the spike tip after the lanes' last pushes (explorer's b8bc10e1b and crusader's d9c320dd7 window instrumentation are local today), `rg/k5p-sills-2026-11-17` at `spike/k5p-sills` a5a4ecc0. Total main-bound work: about 4-5 lane-days, three PRs (code: item 1-2; docs: items 3, 5, 6, 7, 13, 14; the parking ADR and status in §2).

Difference from Carbon's plan: Carbon parks the base work (my option D) as well and needs no flight. I keep the flight (30 minutes, stores ready since 07:45) because it is the only Tier-D evidence the owner will have of RG against B, and because every later base decision (O5, O12) is a look decision. I do not schedule D now; §5 says when.

## 2. The parking record
### 2.1 ADR "River graphs parked" (next number reserved through the lead; one page)
Context (up to 5 bullets): P3 E6 built RG on `spike/rg-prototype` (plan draft 4, 2026-11-16); G2 chose RG over B on 2026-11-22 with five plan criteria unmet; G3 (2026-11-23) opened nine rows; on 2026-11-26 none passes in full; two independent reviews recommend parking.
Decision:
1. RG work stops on 2026-11-26. `spike/rg-prototype` is frozen at tag `rg/parked-2026-11-26`; no RG stage is ported.
2. B stays the production chain. Its known defects (crest p95 15-316 m, saddles 40-27,108, R3a 57.68 % undrained pits under the connected sea) are recorded as B's, not as reasons to resume RG.
3. The component record (§2.2) is the evidence; it is kept in `docs/archive/P3/river-graph/`.
4. Restart conditions: §5's triggers, written as the ADR's point 4 (base signed off with rivers off; the editor edit model decided; one lane; calibrated gates set before any row; an owner flight at every look gate).
5. What lands on main from the spike: §1 items 1-2 (code), 3, 5-7, 13-14 (docs). Everything else is reference on the tag.
6. The proposed ADRs: §2.3.
Consequences: P3 E6 closes as "parked with record"; E4 and E5 close with their ADR dispositions; the E6 lane orders end; `docs/parking.md` gets one line per parked item (RG; K5's washboard O5; K7's read O12; the sinuosity and chainsaw bars O13, O14; the editor edit model).

### 2.2 Component decision record (what passed, what failed)
| Component | Result | Figures | Source |
|---|---|---|---|
| Graph and basin contracts (G0), canonical graph, validators | passed | injected failures caught; two layouts; GEN-189 on main | `plan.md:212`, #1564 |
| Graph authority on five windows | passed | violations 0 on R3a, R3b, F1; agreement at 75 km2 94.5-99.5 %; corridor 98.5-100 % | `g2-report:13-23` |
| Preservation against S_KH | passed | crest p95 0.09-0.44 m vs B 15.6-316 m; saddles 0-3,423 vs 40-27,108 | `g2-report:28-34` |
| Preservation against S0 | mixed | crest p95 7.2-20.9 m; R3b saddles 753 vs B 723; KH lowers 10-18 % of land outside the bars | `g2-report:32`, `review-fable-rgv0:96` |
| Basin topology (G1e) | passed | ids, nesting, spills stable under every partition and thread count | SPIKE-130 |
| Net budget rule | passed | 0 net overruns on five windows (gross R1h 100, M1 1,298) | gate R3 |
| Partition bit-identity | passed for KC, band6, base1 (tiled), wave 1 on TEST-151's five layouts | the fixture only; R1h one vs two regions not run | gate:190, :107 |
| Wave-1 memory at full count | passed | 9.26 GB peak working set at 1.489e8 cells against 10.8 GB; 4.4 h on 16 threads | crusader 11-26 21:00 |
| Wave 2 and L buffers at scale | not measured | wave 2 up to 8.9 GB; 16 L buffers 31.4 GB at today's bytes | `g3-6-memory-model:247-248` |
| Cost | failed | 2.4-3.5x B per window; one window 11.79 GB and 12-14 GB in basins | `roadmap:16`, explorer 11-26 14:58 |
| Look (G3-9) | failed | step 1 not passed; 88-93 % of the residual pattern is S0's; 3,771 / 7,087 cols; A5 would add +7,433 / +11,622 km2 of lakes | `step1-look-signoff:45`, `attribution-184w:19`, `row3-cols-design:32` |
| Look bars | failed calibration | Xdiag 23/30, M4 17/30, D8 sinuosity 12/30, straight-run share both rules, corner links miss 42/54 | `O14:138-145`, `O13:101-105` |
| Incremental edits (G3-7) | not started | wave 1 alone 4.4 h and 9.26 GB against BUDG-005's 4.0 GB terrain share and BUDG-003's 6 threads (inferred: about 2x the time at 6 threads) | Carbon AB1; ADRs 0112, 0113 |
| Cube faces (G3-5) | primitives only | GlobalCell, CubePatch, spill engine, cell area on main; KH on edges, base1 on the cube, B1-B7 not run | #1654-#1661 |
| Generalisation (G3-8) | not run | 10 windows drawn, never run | #1648 |

### 2.3 Dispositions
| Item | Disposition |
|---|---|
| ADR 0123 (draft PR #1499, the first river hand-over proposal) | close the PR as superseded by the record; no file on main |
| ADR 0124 (drainage at L in one scaled round; B's D-1 form) | stays "proposed" (it is B's path, `plan.md:38`); the parking ADR says so |
| ADR 0125 (river hierarchy and data contract), 0126 (regions are storage and execution domains), 0127 (lakes and basins are objects) | drafts in `digest/review-v4/09-11`; mark "parked, 2026-11-26" in `docs/adr/README.md` with a pointer to the archive; 0126 is noted as holding under B too (`plan.md:39`) |
| `docs/status.md` | delete the four RG "Waiting" items and the G3-A..D queue rows; one line "E6 parked (ADR <n>, tag `rg/parked-2026-11-26`)"; E4 and E5 rows close with the ADR dispositions |
| P3 E6 exit item | closed as "parked with record" in `docs/milestones/P3.md`; P3's acceptance runs without an RG verdict; a log fragment `docs/log.d/2026-11-26-rg-parked.md` |
| Open owner decisions O8, O10 rest, O15, O16, step-1 sign-off, D1-3's ADR | moot; recorded in the archive as open at parking |
| O5, O12, O13, O14 | `docs/parking.md`, one line each (they are base and look items) |

### 2.4 Lanes' wind-down
- Orders: the lead ends #203j (explorer), #208 and G3-6 steps (crusader), the spikes freeze; lanes push their clean tips to `spike/*` and report; then idle (`docs/routing.md`: idle lanes send heartbeats only).
- Claims: `lanes/rg-chain-claims.md` emptied and closed with one line.
- Worktrees (14 today): prune `spikes-gate` (merged), `start-879e4b`, `sonnet-spikes-36fc88`, `crusader-spike`, `spikes-161c`, `spikes-bake` after their tips are pushed and tagged (`cargo xtask worktrees --prune`); keep `crusader-main`, `opus-architect-*`, `sonnet-crusader-*` for the three PRs.
- Data (`C:/DEV/SeriousPlanetData/tmp` 114 GB): delete the field dumps (`g1f6` 24 GB, `g1f15` 29 GB, `g203h/j` 12 GB, `g196`, `g1f14*`, `g19x`, `g20x`, `rd`, `ks-g1b2`; about 100 GB); keep every `.md`, `.txt`, `.csv`, `.png` sheet (under 500 MB) and the six fly1126 stores (about 180 MB) in `tmp/fly1126/stores`. `worlds/` (14 GB) is unaffected.
- The digest: a `river-graph/00-PARKED.md` index naming the archive files; the decisions folder gets a closing line in `00-open-decisions-2026-11-26.md`.

## 3. Learnings to collect
Already on main (do not duplicate): 2026-11-26 "fixes ran only downstream; S0, the base and the climate were treated as fixed" and routing rule 10; 2026-11-14 "later-stage start-surface fixes failed" and "a default-on stage after one measurement made its defect worse". The file is at 19,979 B of 20 KB, so the 2026-11-14 block moves to `docs/archive/P3/learnings.md` first.

| Learning (one entry each, three lines) | Evidence |
|---|---|
| A geometric bar is calibrated on synthetic valleys at five angles and on the input surface before it is registered; five bars registered on 11-24 failed on 11-26 and about half the lane time between chased them | TEST-155 (#1665); `g3-snapshot §5`; spikes outbox 11-26 01:05 |
| A gate closed with its own criteria unmet turns every later gate into a renegotiation; G2 closed with cost, tile-locality, the edit, widths and the flight unmet, and G3 added 7 bars and withdrew 5 | `plan.md:130-147`, `g2-report:59`, `O13`, `O14` |
| A decision adopted before its row runs is reopened within two days: A5 (11-23 -> O15 11-25), O8 step 1 (11-23 -> not passed 11-25), O10's bars (11-24 -> fail 11-26); adopt after the row | gate §7 |
| An authority defined at 814 m and drawn at 101.8 m crosses ridges only the fine grid sees (3,771 and 7,087 cols); routing the coarse grid on fine passes needs the erosion re-solved on the same passes, and that is a base change | `row3-cols-design:25`, `O15:38-66` |
| Kernels that allocate for the whole cube hold 7.2x the landmass; the compact grid is the precondition of any per-land-cell budget (72-89 GB -> 9.26 GB) | `g3-6-memory-model:259-262`; crusader 11-26 21:00 |
| Window-edge semantics (edges as outlets, exits, seeds) inside a stage make the result depend on where the window is cut: moving R1h by 200 cells turned a 691 km2 lake into 191 km2 | `review-fable-rgv0:94`; spikes #153 |
| The editor's budgets (BUDG-003 6 threads, BUDG-005 4.0 GB terrain) are checked by arithmetic before a below-erosion stage is designed; wave 1 is 4.4 h and 9.26 GB at 16 threads | Carbon AB1; crusader 11-26 21:00 |
| Three autonomous lanes outrun one part-time owner: 95 orders in four days, eight open decisions, three unflown flights; cap the queue, not the lanes' speed | order files; `00-open-decisions-2026-11-24.md`; 11-25 digest Process |

The longer material (the ten displacement rows, the P1-P8 and SF1-SF5 tables) goes to the archive, not to `learnings.md`.

## 4. What must not be redone
| Dead end | Why it failed | Evidence |
|---|---|---|
| KH-only chainsaw forms (`line`, `line8`, `h4`, the quad-split variants) | `line8` removed the triangles but failed §3: corner links +71, saddles against S0 +1,652, 75 km2 agreement -58 %; the root is the trunk on H1 D8 segments, not KH's read | gate R12 (:65) |
| The S0 keep mask in step 1 (`--kh-keep-s0`) | 12,609 depressions, about 8x the final kept area; basins-2 drains most, leaving ponds 12.82 %; a mask is consistent only with the kept decision taken before the lines | gate:146-149 |
| `--kh-guard` (cap KH's curve lowering) | the held cols dam the trunks: ponds x25, uphill x19, corner links 16.74, 13/18 | gate:175 |
| `--band6-keep-rims` alone | moves the cuts: band6 928 -> 98 but KV 535 -> 1,046, corner links 260 -> 1,114 | gate R8 (:60) |
| K5' static sills as built (SPIKE-131 form (a)) | cols -52 % against -70 % needed, 1,126 bed rises against 100, trunk exceptions 22.10, 8/18; kept H1 basins 69 -> 337; it changes the whole erosion (mean 101 -> 78 m). A re-test is valid only with a K5 that has a hillslope term, since the straightness and cols are carved by the stream-power feedback | `O15:58-66`, `k5-sills-scoping:43-51`, `o16:122-125` |
| K6' (H1 on L's passes) with today's K5 | beds rise where K5 carved no channel: reaches with a rise over 10 m 44 -> 902, residuals 6,765 -> 472,614 | `O15:38-44` |
| Per-cell runoff as built (`--runoff-cell`) | 40,866 conservation breaks | gate R6 (:58) |
| `--bed-sea-floor` and `--sea17` as single switches | reading 17 held but relief lost fails (+0.24) and agreement falls; `--sea17` uphill 2.03 %. Reading 17 needs one coupled coastal rule (decision 5), never a switch | `g2-report:72`; 11-22 digest |
| `--band6-hump 10`, a lower budget floor | violations 113 -> 185, corner links +142; the floor had no effect | 11-23 digest |
| A5's 10 m col cap, and any per-col lake rule on today's H1 | +7,433 km2 of lakes on R1h (6.9x), +11,622 on F1 (44x), 274 / 593 lake chains; at 50 m still +5,708 km2 on R1h | `row3-cols-design:44-51` |
| The uncalibrated look bars (M4 <= 13.90, in-zone Xdiag <= 0.010, D8 sinuosity >= 1.20, straight runs <= 15 %, corner links as a defect count) | each reads a smooth straight valley as defective at some lattice angle; S0 itself reads M4 18.95 | `O14:138-145`, `O13:101-105` |
| `--kv-wall-cap`, `--curve-profile round`, `--curve-bed fit`, `--meander-runmin` as look fixes | none reached the in-zone target; the facets were the zone's height gate, fixed by the distance weight (`--kh-weight dist`) | gate:160-189 |
| A length cap on breaches (112 L cells; a tile cap on base1) | hump cuts +2.25 % (fails §3); the base1 cap moved S0 by up to 432 m and basins by -31 % | 11-25 digest item 3 |
| `sel-post` off | agree_75km2 -1.8 %, kept lakes -28 % | gate:195 |
| `--order carbon` (kept set before the lines) and `--trib-on pre` | spill_missing 0 -> 12, violations 1 -> 40; agreement fails on three windows | Carbon 11-23 S1; crusader #168 |
| A pit-reduced S0 proxy as a GEN-098 substitute | bounds the pits' effect only; relief10 falls 368 -> 130 m | gate O7 (:208) |

## 5. Ways to un-park
Triggers (all, not any; inferred from the record):
1. **The base is signed off with rivers off**: K7's C2 read (O12-A), K5's hillslope coupling and planform on the five windows, judged by the owner on look sheets and one flight; relief loss bounded by a bar set before the rows (SPIKE-139's term cost 14-22 %).
2. **The editor's edit model is decided** (Carbon's AP1): an ADR amending 0112/0113 so that interactive sculpting is a bounded local re-grade under BUDG-003/005, and the landmass re-solve is an explicit bake. Without it any below-erosion hydrology, B's included, cannot be edited interactively.
3. **Capacity**: the owner can give about an hour a day for 4-6 weeks, and one lane runs, not three.
4. **One pre-set test passes before any L stage is written**: H1 routed on sills of the new K5 removes at least 70 % of the deep cols and leaves at most 100 bed rises over 10 m on R1h (the test K5' failed), measured lock-free on dumps.
Restart forms, in outline:
- **F1, graph-only overlay**: the sparse graph (trunks, tributaries, basins with spill levels, GEN-189) computed on the new S0 and stored for the runtime's ribbons and lakes, with no L carving. Cheapest (inferred 2-3 lane-weeks); gives rivers and lakes as objects for gameplay; leaves B's valleys as they are.
- **F2, the simplified RG** (§6): three stages on a clean S0, canonical tiles for the raster stages.
- **F3, basins only into B**: G1e's basin objects and spill levels as B's lake stage. Small; depends on F1's graph.
- **F4, resume the tagged chain**: not recommended under any trigger; the record says why.

## 6. The owner's "last hope" option, judged
The proposal: a simplified RG pipeline, with the freedom to change S0's definition and the world parameters that feed hydrology, with gates redesigned on the learnings so that they are achievable, and the look weighted over technical correctness.

**Assessment.** As a basis for the next attempt it is sound in three parts and unsafe in one. Sound: (a) freeing S0 and the world parameters removes the contradiction that sank G3 (the look was gated on an input the plan held fixed; routing rule 10 now says so); (b) a shorter stage list removes the repair stack that produced ten displacements in five days; (c) the gates must change, because five of seven look bars could not measure what they were meant to. Unsafe: "look over technical correctness" is only sound if it means the Tier-D judgement replaces uncalibrated proxies, not that correctness rules are traded for appearance.

**What it would look like** (inferred; it coincides with Carbon's AP2-AP3 and my restart form F2):
| Stage | Content | Which RG parts survive |
|---|---|---|
| 0, the base (B's too) | base1 with wider breaches; K5 with a channel-guarded hillslope term and a km-scale planform bias, calibrated against GLO-30 relief and spectrum; K7's C2 read; K6 trunks on sills consistent with K5's routing; kept basins by water balance (ADR 0111 pt 5) and the tectonic mark (R23) | R23's mark and calibration draws; the attribution method |
| 1, sparse decide (global, bit-identical) | trunks and tributaries extracted once on the clean S0, minimax centreline, 4-connected emission at the source, one monotone bed solve over the graph (no post-hoc carries), basins with spill levels, the net budget per cell | G0 contracts, GEN-189, GEN-191, the net budget rule, the curve's minimax routing, basin objects |
| 2, tile valley carve (ADR 0119 canonical tile plus halo, own ADR with G-SEAM) | one smooth-minimum envelope around the stored beds, replacing KH, KS0, KS1, KV and the four connect passes | KV's envelope idea, width form 1 |
| 3, tile pit polish (canonical tile) | local fill-or-breach under the remaining budget, replacing band6 and KC; no global rounds | KC's bounded rule |
Gates in what form: graph validity, conservation, uphill and ponds against the graph, net budget 0: kept as hard bars (they are correctness, not look). Geomorphology: against GLO-30 with metrics that pass TEST-155 before registration; where none passes, report only. Look: the owner's flight and pre-registered crops at a fixed sun (O14's form) as the gate. Partition: bit-identity for stage 1 and for wave 1; canonical-tile reproducibility plus G-SEAM for stages 2-3 by their own ADR (ADR 0119 pt 4 requires one). Memory: BUDG-004's bake budget measured at the landmass count (G3-6's form, which worked). Edits: the AP1 model, measured at level 2 of terrain-target §11.1.

**Cost** (inferred from the lead's and Carbon's figures): base work 2-4 lane-weeks, open-ended calibration; the AP1 ADR 1-2 lead-days; stages 1-3 at 1,000 lines and 5 switches 3-5 lane-weeks on one lane; owner time about an hour a day at the look gates. About one quarter of calendar time, if it holds.

**Where it fails again:** (1) the 814 m to 101.8 m gap remains unless trigger 4 passes; a finer H1 (407 m) doubles wave 1's memory and time; (2) K5's calibration may not converge without relief loss (the only measured term cost 14-22 %); (3) the owner's capacity, not the lanes', sets the pace, and the last attempt consumed it in ten days; (4) switch growth: 13 -> 96 in nine days last time, so the cap must be a rule, not an intention; (5) the editor's edit cost is unsolved for B too.

**The CON-31 line.** Redesigning a gate on evidence means: the metric passes TEST-155 on synthetic valleys at five angles and on S0 before it is registered; its reference is outside the candidate (GLO-30 at matched relief, B's measured value, or S0's own density as a null); its value is set before the rows run and recorded in the spec with a date; a later change is an owner-stated exception with its reason, also dated. Fitting a bar to the candidate means: a floor taken from the candidate's own row (O10's proposal from f1-mm's 105.1), a tolerance raised after a fail, a bar moved to "report only" after a fail without a calibration finding, cells excluded from a bar because the candidate fails on them, or a gate judged on one window's row and generalised. The last attempt did some of each and corrected most of them; the restart writes them as rules before the first row.

**"Look over technical correctness."** Should mean: the owner's eye on fixed crops decides appearance (chainsaw, straightness, washboard, valley form), and no scalar proxy outranks it; metrics are reports; a correct chain the owner dislikes does not pass. Should not mean: water that flows uphill, ponds the graph does not know, a bed that rises without a recorded exception, cuts past the budget (the owner's fault class of 2026-11-13), results that depend on thread count, partition or window cut (CON-14, CON-28 to CON-30; canonical tiles are a legitimate re-definition by ADR, not a waiver), or a stage that cannot run inside BUDG-003/005 in the editor. Those are not "technical" against "look": uphill water and dry lakes are look defects too, and the editor budgets are the product.

**Recommendation.** Take the owner's option as the restart form (F2 in §5), with its four triggers as preconditions, and with form F1 (graph-only overlay) as the first deliverable inside it, because it needs no L carving and gives rivers and lakes as objects early. Do not start it now; the base work (trigger 1) comes first and helps B regardless. Confidence that this is the right form for a restart: about 65 %. Confidence that it lands within one part-time winter: about 40 %, with the base calibration and the owner's capacity as the two reasons it may not.
