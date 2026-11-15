# CP2 independent review pass (P3 E5)

Status: report (spikes, 2026-11-14; lead order #120 item 2). Read only: it states and proposes, it decides nothing.
Reader: the spikes lane built none of the production river code. It did measure some gate rows itself (G-ROUND, G-DET, G-PART, G-CON, G-LOCAL, G-SURF, G-SURF-I, G-PROFILE, G-MFD on windows): for those rows this pass is not an independent reading.
Sources: `docs/briefings/terrain-prototype.md` (Exit list, Phases), `docs/design/terrain-target.md` (A3) §12.1 and §12.4, `docs/gates/ledger.md` at main 72256aa4, `docs/milestones/P1.md` to `P3.md`, `docs/milestones/P1-P2-closure.md`, `spikes/region-edit/results/results.md` (SPIKE-112).
States: **pass** (a ledger verdict on main), **measured** (numbers exist, partial or outside the ledger), **open** (failed or work under way), **not started**.

## 1. The A/B gate exit list
| Exit | State | Evidence | Gap |
|---|---|---|---|
| A3 §12.4 P0 and P1 conditions closed or reported | measured | §2 below | 2 of 4 P0 rows partial; G-RT (P1) not run |
| Every §12.1 gate passes, fresh and after each edit E1-E7 | open | §3 below | G-PROFILE fails; G-SEAM, G-RT not run; after-edit readings exist for G-EQ, G-INC, G-BASIN only |
| Measured: landmass memory against 12 GB | pass | G-SCALE ee8347b7: 11,387 MiB worst of two k = 4 runs | margin 57 MiB to the 11,444 MiB gate; 897 MiB over the 11e9 B target (BUDG-016) |
| Measured: store against 40 GB | pass | G-STORE 6b5bb0ac: upper bound 28.4 GB steady | compaction peak (up to 57 GB) reported, not gated; no desert or glacier L |
| Measured: latency of the preview levels | measured | SPIKE-112: level 2 of an edit on one region 29-42 s against 178-184 s rebuilt, bit-equal; landmass: the area sweep alone 159 s a round (about 11 min a 4-round solve) | level 1 latency not found; landmass level 2 not measured end to end; spike code, not the production path |
| Measured by a lane other than the builder (item 12) | measured | spikes re-measured G-ROUND, G-DET, G-PART, G-CON, G-LOCAL, G-SURF, G-PROFILE, G-MFD | G-SCALE, G-STORE, G-EQ, G-INC, G-BASIN, G-SED: explorer only |
| G-RT on the in-memory base, provisional | not started | ledger: not run | waits on the owner's flights (P3 E3) |
| Re-estimate of the remaining work under A and B | not started | briefing re-estimate 2026-11-11, A only | B never estimated |
| The owner's review and A-or-B decision | not started | - | after the above |

No switch criterion S1-S5 is reported as triggered: G-LOCAL and G-EQ, the S1 and S2 gates, pass where measured.

## 2. A3 §12.4 conditions
| Condition | Priority | State | Evidence | Gap |
|---|---|---|---|---|
| Owner decisions 2-10 | P0 | pass | owner, 2026-11-11 | - |
| MFD: clipped against unclipped and boundary flux | P0 | pass | G-MFD bc34aafd; rule C kept (architect, 2026-11-12) | border slope +20 % (R3a) and +7.8 % (R3b) against a 1 % floor: reported, not gated |
| f32 rounds: gates re-run on the f32 form, near-degenerate fixtures | P0 | measured | G-LOCAL 0a8c65a7 on f32 rounds (P1 E2); k = 4 hashes equal (#1447) | G-EQ's f32 against f64 comparison (relief 2 %, topology 1 %) not run; no near-degenerate fixture named |
| Determinism: pins, Windows and Linux, thread counts, repeats, incremental equals fresh | P0 | measured | 1 / 2 / 5 / 8 threads 0 differ (5cc69697); k = 4 runs hash-equal; incremental = fresh in 5 edits (SPIKE-112) | Windows against Linux and the CI manifest fixture of the round solver and H1 not measured; toolchain pin not found in the ledger |
| Thermal: pits and dams, fixtures, sweeps | P1 | measured | brief #1514 (options A and C wait for ADR 0123) | fixtures and sweeps not run |
| Resources: headroom, compaction, crash-safe replacement, other bodies | P1 | measured | G-SCALE ee8347b7; G-STORE 6b5bb0ac | crash-safe replacement not tested; smaller and larger body not measured |
| Editor: readiness, supersession, failure; undo and replay; latencies | P1 | measured | SPIKE-112: undo of a basin merge bit-exact in 10.8 s | commit readiness, supersession and failure not tested; spike code |
| Runtime: frame time (G-RT) | P1 | not started | - | P3 E3 |
| Hydrology: connectivity across faces and after landmass-changing edits, channel levels, units | P1 | measured | G-CON pass on windows and the k = 4 landmass | across cube faces (REND-059's dry notch is open) and after E7 not measured; lake pieces not measured |
| Plausibility fixtures; convergence indicators per round | P1 | measured | R1h, R3a, R3b windows; SPIKE-114 to 118 river numbers | mountain divides, endorheic basins and coastal mountains fixtures absent; no per-round convergence indicator reported |

## 3. A3 §12.1 gates
| Gate | Fresh build | After edits | Latest evidence | Gap |
|---|---|---|---|---|
| G-SURF | pass | not measured | ea6b1b2b; 5cc69697 (spikes) | re-read on the ADR 0123 surface |
| G-SURF-I | reported | not measured | ea6b1b2b; 55ae7016 | as above |
| G-CON | pass | not measured | 1094d82a (k = 4), 5cc69697 | lake pieces; cube faces; E7 |
| G-PROFILE | **open (fail)** | not measured | 7d504a6f: rises up to 26.5 m on three tiles | cause: H1 lines crossing L ridges; P3 E2 after ADR 0123 |
| G-SEAM | not started | not started | - | ADR 0119's gate has no reading |
| G-LOCAL | pass | - | 0a8c65a7 (f32); the memo across a rule flip equals the fresh solve | two windows only; not read after an edit |
| G-ROUND | pass | - | 5cc69697 | - |
| G-EQ | measured | measured | f6adbdf1 (rounds, E1-E7, f64); SPIKE-112 (store bit-equal, 5 edits) | the canonical f32 forms, K7 and the sampled surface not in the ledger's comparison |
| G-INC | measured | measured | f6adbdf1: tile floods 6.5 / 9.8 % (bound 10 %) | no tier-A test of the bound; SPIKE-112: the memo covers 3-5 % of round time |
| G-BASIN | measured | measured | f6adbdf1: E3 fixtures | 1e-9 level rows and the high and zero `E_open` fixtures; no kept inland basin to split (SPIKE-112) |
| G-SED | measured | not measured | 86c8422b | no spatial cut of the pass; round-internal sediment |
| G-LAND | measured | not measured | e4c03141 | relief and hypsometry against the whole-region carve |
| G-STORE | pass | - | 6b5bb0ac | see §1 |
| G-SCALE | pass (complete build) | - | ee8347b7 | thin margin; partitioned route, H0 split, 2x radius, other bodies |
| G-RT | not started | - | - | P3 E3 |
| G-MFD | pass | - | bc34aafd | - |
| G-DET | measured | measured (memo = fresh) | 5cc69697 | Windows against Linux; CI manifest |
| G-PART | pass | - | 5cc69697 | - |

## 4. What looks overbuilt
- **The catchment memo of the rounds.** It saves 3-5 % of a round's time; memoised rounds run twice as slow as resident ones (8.9-9.8 s against 4.4-4.6 s); the whole edit gain is K7 on the dirty tiles (SPIKE-112). At landmass size the area sweep, recomputed whole on one thread, dominates (159 s a round). A per-catchment parallel sweep (69 s, bit-equal; learnings) does more for the edit path than the memo does.
- **Readings on a surface that is about to change.** G-SURF, G-SURF-I, G-PROFILE, G-SEAM and G-LAND read K7 on `B + regional`; ADR 0123 changes K7's start surface. Readings before E2 are stale at the gate; further readings on the old surface add little.
- **The gate count.** 18 gate rows of §12.1 for a prototype whose visible product (rivers at L, P3 E2) still fails G-PROFILE. The structural gates (G-ROUND, G-PART, G-LOCAL, G-DET, G-CON) pass and are repeated; the gates that show what the owner sees (G-PROFILE, G-SEAM, G-RT) have no pass.

## 5. What is missing
- **A river gate in §12.1.** The owner's bars (basins at most 10 % of river length, canyons over 50 m at most 1 %, lakes named, relief kept, axis share) live in P3 and in `cargo xtask river-gate` (BUILD-055), not in A3. Two canyon definitions disagree (burn below the surface against ring p50 minus level: R1h after one L round 0 % against 16.6 %); the gate row needs one definition before the gate.
- **A solver path that keeps an inland lake.** The breach cuts every basin (learnings; SPIKE-112): G-BASIN's split and merge cannot be exercised by a height edit, and P2 E5's lakes ADR is open.
- **The landmass edit path.** Level 2 is measured on one region (29-42 s); at landmass scope the routing recomputed whole sets the latency (about 11 min a 4-round solve), with no measurement end to end.
- **G-RT and G-SEAM readings,** and Windows against Linux for G-DET.
- **The re-estimate under B.** The exit list asks for both; only A has dates.

## 6. Readiness
- CP2's exits (briefing, P2 row): the thermal brief is done (#1514); this review is done; rivers and lakes drawn (P3 E2) and G-RT (P3 E3) are open.
- The A/B gate is not ready. In order: ADR 0123 decided; E2 (rivers at L, R3a re-baked); G-PROFILE, G-SEAM, G-SURF and G-LAND re-read on that surface; G-RT from the owner's flights; G-EQ's f32 comparison and Windows against Linux; the re-estimate under A and B.
