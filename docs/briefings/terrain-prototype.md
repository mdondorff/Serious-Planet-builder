# Terrain prototype (work block after SPIKE-105)

Status: active since 2026-11-08 (owner: D1 = option A, conditional). Replaces the M9.1 gate of `docs/briefings/m9-plan.md`; the M9 planet bake is frozen until this block closes. Contract: `docs/design/terrain-target.md` (A3), ADRs 0110-0115 (proposed). Plan: `spikes/architecture-replan/results/results.md` sections 13 and 17. Target date: mid-December 2026 (estimate).

Re-estimate: 2026-11-09 (remaining work: items 9 to 18 open; target date as above). An item that becomes done moves this date (BUILD-042).

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
| 9 | Complete resident build against 12 GB (G-SCALE, A3 §6.6 scenarios); cache eviction | explorer, crusader (eviction) | open; partitioned rounds measured (GEN-149); eviction merged (GEN-153). Complete build in the example, with rule (1): commit peaks 11,136, 11,329 and 11,135 MiB against 11,444 MiB (2026-11-09, runs 2 to 4). Row closes on the production phase plan, worst of two runs; tile-wise route not triggered; other §6.6 scenarios not run |
| 10 | Deposition, one-cell pit spreading and thermal on the cube grid; G-SED | explorer | kernels and balance done (GEN-150 to GEN-152); partition test open. Sediment pass moves to the canonical flood (GEN-159, in review): the plain FIFO flood had inflated the pit share to 18.5 %, void. Pits after the pass 0.26 % (k=1), 0.35 % (k=2); k=4 comes with the phase-plan runs. New-chain K7 follows, before GEN-154's gate reading. Evidence for the A/B gate (A3 §6.7): thermal leaves dams in one-cell canyons, up to 344.5 m after deposition at k=2 |
| 11 | Local 102 m pass in the new chain on the base read (G-SURF) | explorer | pass done (GEN-148); gate and G-SURF-I report open |
| 12 | Independent measurement of the gates by another lane | spikes | partly done (SPIKE-106); repeated at the gate |
| 13 | Hills and plains: the structural gates on a second region and seed | spikes | open |
| 14 | Store: real serialized data (G-STORE) | explorer | in progress |
| 15 | Runtime: sampler composition, lakes, rivers; flight (G-RT) | crusader | open; starts when items 7 and 11 are on main |
| 16 | Editor path and preview levels (E1-E6 from the editor) | crusader | in progress: level 1 first (height, stamp and climate rows of A3 §11.1), no dependency on item 15; coarse-carve rows and level 2's display wait for item 15 and GEN-153; a climate item kind in `world-def` comes to the architect first (CON-10) |
| 17 | Noise of the base bands: gradient noise, crease filter or turned lattice on eroded terrain | crusader, owner | open; decided at the gate |
| 18 | Stage previews: a read-only probe at each new-chain stage, sheets in `target/review/stages/` (BUILD-046, GEN-161); committed manifest of data hashes and numbers on a small fixture, required up to date by CI (BUILD-047, TEST-103, TEST-104); advisory reviewer agent and sheet size cap (BUILD-048) | crusader | open; the probe lands before the k=4 runs of the phase-plan item; probe on and off give identical hashes; it must not raise H1's peak |

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
