# Terrain prototype (work block after SPIKE-105)

Status: active since 2026-11-08 (owner: D1 = option A, conditional). Replaces the M9.1 gate of `docs/briefings/m9-plan.md`; the M9 planet bake is frozen until this block closes. Contract: `docs/design/terrain-target.md` (A3), ADRs 0110-0115 (proposed). Plan: `spikes/architecture-replan/results/results.md` sections 13 and 17. Target date: mid-December 2026 (estimate).

Re-estimate: 2026-11-10 (remaining work: item 14 open for the explorer lane (G-STORE: L on a second region, deep-ocean field tiles, temporary space, edit overrides; item 3's stored-field read), 12 repeated at the gate, 15 in progress, 16 and 17 open; items 9, 10, 11, 13 and 18 done; target date as above, unchanged). An item that becomes done moves this date (BUILD-042).

Scope: the region of `spikes/prototype-region/results/results.md`. Everything stays behind switches, off the default bake path.

## Items
| # | Item | Lane | State |
|---|---|---|---|
| 1 | Target document and contracts, three external reviews | spikes, planner | done; a further ADR review pending |
| 2 | True cell metric (GEN-138) | explorer | done (#1235) |
| 3 | H0 across cube edges (GEN-139) | explorer | done (#1236); the stored-field read is open |
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
| 14 | Store: real serialized data (G-STORE) | explorer | in progress |
| 15 | Runtime: sampler composition, lakes, rivers; flight (G-RT) | crusader | in progress: design notes #1305 with the architect's answers (decisions D6: lake producer at bake time, width law `w = a · Q_w^b` as bake parameters, L as kind 13 with a version bump, G-RT on the prototype region, channels end at lake entries); order lakes, sampler switch, rivers and G-PROFILE, G-RT |
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

## Exit list (the A/B gate)
- Every gate of A3 section 12.1 passes on the fresh build and after each edit, or its failure is reported against S1-S5.
- Measured: landmass memory against 12 GB, store against 40 GB, latency of the preview levels.
- Measured by a lane other than the one that built the item (item 12).
- A re-estimate of the remaining work under A and under B.
- The owner reviews the prototype once and decides A or B.

## Close
At the gate: each item is marked done, carried or dropped in `docs/status.md`.
