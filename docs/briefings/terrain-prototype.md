# Terrain prototype (work block after SPIKE-105)

Status: active since 2026-11-08 (owner, 2026-11-08: option A, conditional). Replaces the M9.1 gate of `docs/archive/M9/briefings/m9-plan.md`; the M9 planet bake is frozen until this block closes. Contract: `docs/design/terrain-target.md` (A3), ADRs 0110-0115 (accepted for the prototype, 2026-11-11, with the conditions of A3 §12.4). Plan: `spikes/architecture-replan/results/results.md` sections 13 and 17. Target date: mid-December 2026 (estimate).

Re-estimate: 2026-11-11 00:30 (remaining work: P0 closure; item 15 in progress, its order changed for CP1 and CP2 (owner, 2026-11-10); f32 rounds (GEN-166) and the memo-key fix added; 12 repeated at the gate; 16 and 17 open; items 3 and 14 done; target date as above, unchanged, if P0 uses the lanes' waits on machine slots). An item that becomes done moves this date (BUILD-042).

Scope: the region of `spikes/prototype-region/results/results.md`. Everything stays behind switches, off the default bake path.

## Phases (owner, 2026-11-10)
The block runs in phases of about 1.5 weeks. A phase starts with an entry note here (items, exit list, risks, IDs reserved), which the owner approves in one line. It ends with a closure, which bulldozer runs and the architect verifies, and a one-page close to the owner: what passed, what deviated, go or no-go. Work in flight is never closed mid-way; it carries into the next phase.

| Phase | Ends (estimate) | Exit |
|---|---|---|
| P0 | about 2026-11-13 | closure of finished work only (list below); in-flight work (item 15, GEN-166, the memo-key fix) is not touched |
| P1 | CP1, about 2026-11-20 | G-LOCAL measured; one region flyable from the stored `L` (lakes as flat water, no rivers yet); f32 rounds and the G-SCALE k=4 pair; closure |
| P2 | CP2, about 2026-12-01 | rivers and lakes drawn; G-RT frame time on the prototype region; the thermal-dam options brief; closure with an independent review pass |
| Gate | mid-December | the A/B gate (exit list below); a full `housekeeping` skill run as a milestone close |

Closure checklist (every phase):
- Changes whose code is merged and complete: specs synced to `openspec/specs/`, change archived, `design.md` and `tasks.md` deleted (ADR 0055).
- A3, ADRs 0110-0115 and this briefing agree with the decisions and the code (architect); `docs/status.md` current (bulldozer).
- Worktrees of merged branches pruned (bulldozer; removal only with the owner's yes).
- Learnings recorded. Every lasting decision of the phase is in its home (the ledger, a change's design and spec deltas, A3, the ADRs, the briefing). The architect's working channel to bulldozer is not archived (owner, 2026-11-11).

P0 list:
- Archive the finished changes (each coding lane its own; during waits on machine slots).
- The A3 and ADR text pass for the ADR review pass (`spikes/prototype-gates/results/adr-review-pass.md`): Amends lists, k names, G-SURF and G-CON rows, lake pieces, width law, 12 GB unit (architect; done in this PR). CON-08: resolved by constitution v1.7 (ADR 0116, #1335).
- Repository audit, read-only (spikes): outdated, misleading, overly verbose or duplicated documents and folders, ranked by cost to readers, each with a proposed action (delete, merge, shorten, move). The owner browses the report; actions follow per owner's yes.
- Briefing and `docs/status.md` refreshed; worktree inventory.

## Items
| # | Item | Lane | State |
|---|---|---|---|
| 1 | Target document and contracts, three external reviews | spikes, planner | done; the ADR review pass done (#1326), its verdicts in P0 |
| 2 | True cell metric (GEN-138) | explorer | done (#1235) |
| 3 | H0 across cube edges (GEN-139) | explorer | done (#1236); the stored-field read done (GEN-164, #1317) |
| 4 | Round kernels and solver (GEN-141, GEN-142) | explorer | done (#1238, #1240) |
| 5 | Partitioned route (GEN-143) | explorer | done (#1241) |
| 6 | Landmass across cube edges (GEN-144) | explorer | done (#1243) |
| 7 | Hydrology H1: channels, basins (GEN-145) | explorer | done at 3.26 km (#1244), gates at 814 m (#1245) |
| 8 | Edit cases E1-E6, G-EQ, G-INC, G-BASIN (GEN-146) | explorer | done for the gates that exist |
| 9 | Complete resident build against 12 GB (G-SCALE, A3 §6.6 scenarios); cache eviction | explorer, crusader (eviction) | done for the complete resident build on the production phase plan (GEN-158, #1287): worst of two k=4 runs 11,363 MiB commit against 11,444 (margin 81 MiB, thin); a change that adds resident state in or before hydrology re-measures it (two k=4 runs), first lever rule (1) inside hydrology; eviction merged (GEN-153); other §6.6 scenarios not run; tile-wise route not triggered |
| 10 | Deposition, one-cell pit spreading and thermal on the cube grid; G-SED | explorer | done: kernels and balance (GEN-150 to GEN-152), the partition test (GEN-155, #1290: compact against masked cube, 1 3 8 threads, groups of whole catchments; a spatial cut of the pass stays open), canonical flood in the sediment pass (GEN-159) and the new-chain K7 (GEN-160). Pits after the pass 0.26 % (k=1), 0.35 % (k=2), 0.7355 % (k=4); thermal maximum 609.7, 920 and 1,249.1 m: evidence for the A/B gate (A3 §6.7), no fix ordered. Open: the dam count (next k=4 run, probe on); sediment inside the carve against rewording A3 §6.7 (owner, architect) |
| 11 | Local 102 m pass in the new chain on the base read (G-SURF) | explorer | done (GEN-148, GEN-154; ledger #1311 on ea6b1b2b, canonical K7 and GEN-162): G-SURF pass (5.7e-14 m at the centres before quantisation, 0.125 m after); G-SURF-I reported, A3's criteria met: 0.66 % of vertices over 0.25 m, p99 0.225 m, error grows with the bake step (0 of 3,333 over 0.25 m below 5 m, at most 0.098 of the step), spread (84 of 256 bins, at most 8 in one), error against the local base rise p99 0.057 m; the overshoot (max 2.63 m) is the base's own interpolant, not the stored change, and the C1 clamp stays. The look is checked on G-RT's flight (item 15) |
| 12 | Independent measurement of the gates by another lane | spikes | partly done (SPIKE-106); repeated at the gate |
| 13 | Hills and plains: the structural gates on a second region and seed | crusader | done on R3a and R3b (`spikes/prototype-gates/results/results.md` §6, corrected after GEN-162 in §6c): G-ROUND, G-DET, G-PART 0 differ; G-CON at most 3.0e-14; G-LAND 0.059 % and 0.005 % unclassified; G-SED closes. G-BASIN is not runnable on a window: read at the gate on a stored region. Findings: base stripes (17), fans over lake flats |
| 14 | Store: real serialized data (G-STORE) | explorer | done: G-STORE pass on steady state (ledger, 6b5bb0ac; worst sum 28.4 GB against 35); the compaction peak (2x steady, up to 57 GB) reported for the gate |
| 15 | Runtime: sampler composition, lakes, rivers; flight (G-RT) | crusader | in progress: lake producer (GEN-163), lake shores, basin table (kind 19), width law in K7 (GEN-165) merged; design notes #1322. Order for CP1: kind-13 v3 with the water-mask section, the bake writer, the L read behind `--erosion` (a flyable region); then for CP2: the water source, rivers and G-PROFILE, G-RT. The fill-channel deletion (PR 5b) waits for the owner's acceptance of ADR 0110 |
| 16 | Editor path and preview levels (E1-E6 from the editor) | crusader | in progress: level 1 first (height, stamp and climate rows of A3 §11.1), no dependency on item 15; coarse-carve rows and level 2's display wait for item 15 and GEN-153; a climate item kind in `world-def` comes to the architect first (CON-10) |
| 17 | Noise of the base bands: gradient noise, crease filter or turned lattice on eroded terrain | crusader, owner | open; decided at the gate |
| 18 | Stage previews: a read-only probe at each new-chain stage, sheets in `target/review/stages/` (BUILD-046, GEN-161); committed manifest of data hashes and numbers on a small fixture, required up to date by CI (BUILD-047, TEST-103, TEST-104); advisory reviewer agent and sheet size cap (BUILD-048) | crusader | done (#1282 to #1289; hillshade view #1299). The next k=4 run carries the probe on |

## Owner decisions, 2026-11-09 (A3 §3.1, §6.6, §6.7, §12.1)
| Topic | Decision |
|---|---|
| G-SURF | gate at the cell centres; vertices reported as G-SURF-I; the clamp stays |
| Memory | compact resident route is the normal path; cache eviction required; the complete build measured against 12 GB; tile-wise route only if needed |
| Sediment | post-round pass for the prototype, defined in A3 §6.7; G-SED kept, with a partition test; within-round transport revisited at the A/B gate on evidence |
| Stage previews | committed text manifest (data hash and key numbers per stage, small fixture); images in `target/` and as CI artifacts; sheets committed only at gates, under a size cap; the reviewer agent advises and does not gate |

## Contract changes
A change of a gate row of A3 12.1 or of the table above needs a line here (BUILD-040): `- YYYY-MM-DD | gate or section | what changed | who decided`.
- 2026-11-09 | G-SURF | gate at the cell centres; the lattice vertices reported as G-SURF-I; the clamp stays | owner
- 2026-11-09 | Memory | compact resident route is the normal path; cache eviction required; tile-wise route only if needed | owner
- 2026-11-09 | Sediment | post-round pass for the prototype (A3 6.7); G-SED kept with a partition test | owner
- 2026-11-09 | Stage previews | committed manifest; images in target/ and CI; sheets committed only at gates; item 18 added | owner
- 2026-11-11 | G-SURF | tightened to the spec: at most 1e-9 m before quantisation, half a step (0.125 m) after (GEN-154; the measured values pass) | architect
- 2026-11-11 | G-SURF-I | the vertex set is the tile's core store-lattice vertices (170 x 170 at 102 m) | architect
- 2026-11-11 | G-CON | read on H1's whole graph; the stored river pieces end at a lake entry and restart at the spill cell, checked on their own | architect (GEN-163)
- 2026-11-11 | G-SCALE | a working target of 11e9 B peak commit by the A/B gate below the 12e9 B hard limit (BUDG-016); additions inside the hydrology call bring an equal saving until it holds | owner
- 2026-11-11 | Constitution | v1.7: CON-08 allows regional, partitioned and iterative stages; CON-28 to CON-31 added | owner (ADR 0116)
- 2026-11-11 | ADRs 0110-0115 | accepted for the prototype; the external review's conditions become A3 §12.4 and each ADR's conditions | owner
- 2026-11-11 | SPIKE-105 §13.5 | decisions 2-10 confirmed at the recommended options (A3 §12.4's first P0 condition closed) | owner
- 2026-11-12 | G-MFD | new gate row for ADR 0112 condition 1 (the MFD comparison, SPIKE-107) | architect
- 2026-11-12 | G-SEAM | new gate row: K7 tile borders against interior; alternate layouts reported (ADR 0119) | owner

## Exit list (the A/B gate)
- The P0 and P1 conditions of A3 §12.4 are closed or reported.
- Every gate of A3 section 12.1 passes on the fresh build and after each edit, or its failure is reported against S1-S5.
- Measured: landmass memory against 12 GB, store against 40 GB, latency of the preview levels.
- Measured by a lane other than the one that built the item (item 12).
- G-RT on the in-memory base fixture (no base store reads) is provisional. It is re-run over a store of the approved world bake, and its row names the streaming measurements that stand in for the base read until then (architect, 2026-11-12).
- A re-estimate of the remaining work under A and under B.
- The owner reviews the prototype once and decides A or B.

## Close
At the gate: each item is marked done, carried or dropped in `docs/status.md`.
