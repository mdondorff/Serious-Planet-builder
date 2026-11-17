# SPIKE-137: the G1f chain runner (plumbing)

Opened: 2026-11-17 (lead #166)
Question: can the RG chain run on a window from the lanes' spike code, each stage switchable, and write what `river-gate --graph --stages --partition` reads (BUILD-060), at what time and memory per stage?
Why it matters: river graphs G2 (`digest/river-graph/plan.md` draft 4): RG and B through one gate build. No judgement here: the G2 run waits for the owner's decisions 1-4 and for SPIKE-136.
Time box: about 1 day (lead #166). Used: about 0.3 day.
Success criteria: R1h end to end with the defaults (C1, KS1, every stage) and with two regions; the outputs read by `river-gate`; time and peak memory per stage.
Fallback if it fails: report the stage that breaks. Not needed.
Code location: spike branch `spike/rg-chain` (2334f9a3; `crates/cache/examples/rg_chain.rs`, never merged).

## 1. The runner
| Stage | Code | Switch |
|---|---|---|
| H1 base | today's K6 or K6' on the sills of `S0` (SPIKE-127), as crusader's `kg_trib.rs` | `--base today` (default K6') |
| KH, the hand-over | C1 (`S0` itself); h1 and h2 refused until SPIKE-136 lands | `--kh c1` |
| Budget | BUILD-060's field (`river_gate_graph::budget_field`, the one `river-gate` reads), per L cell; the H1 vector KG-trunk reads is the same field at each H1 block | - |
| KS0 | `S0 - min(max(0, r - C1(HAND)), budget)`, `HAND` over today's or K6''s channel cells; reading 17: never to the sea level + 0.5 m or below; H1's lake water held; a clamp a residual | `--ks off \| ks0 \| ks1` |
| KG-trunk | spikes' G1c (`build_graph`, as carried on crusader's `spike/g1d` 82aa5939) on KS0 | `--skip kg-trunk` |
| Basins | crusader's G1e (`basins_rg::produce`) on KS0 at L, the L trunk reaches as spill receivers; kept basins and H1's lake water are the protected cells | `--skip basins` |
| KG-trib | crusader's G1d extractor-1 (`kg_trib::extract`, D8 initiation, `A_crit` 1 km2) on KS0 | `--skip kg-trib` |
| KS1 | the same as KS0 with `H_f = KS0 - bed` of the first graph cell on KG-trib's receiver chain (the rasterised reach's bed code; KS0 where no reach owns the cell) | `--ks ks1` (default) |
| KV, KC | crusader's G1f (`kv_kc::run`, the plan's parameters) on the KS output, within the budget KS left; the L reaches passed in window cells (the module reads vertices as grid cells) | `--skip kv`, `--skip kc` |
| Regions | `--regions 2`: the pointwise fields (the two HANDs read at L) evaluated over two regions cut at a tile column and stitched; the graph stages run on the stitched input | `--regions 1 \| 2` |

Outputs (`--out DIR --tag T`): `T-graph.txt` (GEN-189 canonical), `T-stages/` in BUILD-060's layout (`ks-lower.u16`, `kv-lower.u16`, `kv-fill.u16`, `kc-lower.u16`, `kc-fill.u16`, `residuals.csv` with `cell,stage,needed_m,available_m,feature`), `T-final-<w>x<h>.f64` (the last stage's surface), `T-timing.csv`.

## 2. R1h end to end
| Stage | Seconds (1 region / 2 regions) | Peak working set after the stage, MiB | Working set after the stage, MiB |
|---|---|---|---|
| Setup (B, r, `S0`, ocean, K6') | 51.8 / 40.1 | 1,629 | 1,335 |
| Budget field | 27.4 / 21.7 | 1,859 | 1,851 |
| KS0 | 19.4 / 11.6 | 3,384 | 2,368 |
| KG-trunk | 2.6 / 0.9 | 3,384 | 2,382 |
| Basins | 180.7 / 155.8 | 13,186 | 2,650 |
| KG-trib | 116.4 / 80.2 | 13,186 | 3,305 |
| KS1 | 3.8 / 4.2 | 13,186 | 3,425 |
| KV and KC | 166.1 / 164.5 | 13,186 | 5,146 |
| Write (5 fields of 133 MB, a 316 MB graph, a 105 MB residual list, the surface) | 120.1 / 119.1 | 13,186 | 5,147 |
| Total | 688 / 598 | | |
- The peak (13,186 MiB, 12.9 GiB) is the basins stage, above the 12 GB world-build budget (memory note of 2026-11-09).
- Times differ between the two runs (KG-trib 116 against 80 s, KS0 19 against 12 s): other lanes' jobs ran beside them in the heavy lock's second slot. The peaks agree within 2 MiB from the basins stage on (13,186 against 13,188 MiB); before it they differ (KS0 3,384 against 2,906 MiB).
- The stage counts above are over the whole window, margin included; the gate's counts below are over the region's box.

| Stage output | R1h |
|---|---|
| KS0 | 991,555 cells lowered, 103,006 residuals |
| KG-trunk | 16,881 nodes, 10,088 reaches |
| Basins | 55,502 L basins (45,438 kept), 2,541 unresolved spills |
| KG-trib | 199,124 reaches, 249,067 nodes, 301 open ends |
| KS1 | 1,498,950 cells lowered, 54,475 residuals |
| KV and KC | 683,748 KV and 1,634,032 KC residual records, 32,344 KC patches |

## 3. What `river-gate` reads
`cargo xtask river-gate r1h --graph r1h-graph.txt --stages r1h-stages --partition r1h-graph.txt,r1h-2r-graph.txt` (main's BUILD-060; `raw/gate-r1h.txt`):
| Part | R1h |
|---|---|
| Graph | 310,763 nodes, 5,044 H1 and 204,168 L reaches, 55,502 basins; 8 violations (1 duplicate id, 7 two nodes on one anchor); 2,541 spill nodes not in the graph |
| Connectivity | 121,281 of 121,699 sources reach an outlet; 0 trunk breaks; 0 conservation breaks; 199,124 of 199,124 tributaries attached |
| Bed | 0 rises outside exceptions; 704 `rise_over_budget` reaches |
| Hierarchy | density 0.489 km/km2, Strahler 51,906 / 11,217 / 2,276 / 216 / 83 / 15, `R_b` 5.38 |
| Agreement (against `S0`'s routing) | 1 km2: 65.9 % of routed cells, 84.1 % of graph cells; 75 km2: 47.7 % and 8.3 % |
| Budget (diagnostic) | KS lowers 479,161 cells (p95 10.5 m), KV 440,949 (p95 50 m), KC fills 73,889; 0 over the budget, 172,198 at 95 % of it or more; 2,372,255 residual records, 0 reaches flagged `budget_residual` |
| Partition | one region against two: identical |
These numbers are plumbing checks, not results: no stage is tuned and the G2 run is not started.

## 4. Gaps for the G2 run (ruled by the lead, #166a: gaps 1 and 2 explorer's, gap 3 and the basins peak crusader's)
- `river-gate` measures a window's `S0`, not the chain's final surface: the agreement and every raster column above read `S0`. G2 needs a surface input (for example `--surface FILE` on the window's L grid), a BUILD-060 follow-up.
- The two-region run splits only the pointwise fields; KG-trunk, basins, KG-trib, KV and KC run on the stitched window, so "identical" is the expected answer here. A partition test of the graph stages needs per-region extraction (crusader's G1d halo cases) in the runner.
- No reach is flagged `budget_residual`: KS's residuals name no feature (`-`), and the runner does not flag KV/KC's residual features in the graph. GEN-189's f64 budget validator would report each one.
- The runner sits on crusader's `spike/g1d` at 82aa5939; that branch has moved on (reading 18's landmass pass, the G1e producer on one flat array): re-merge before the G2 run; the basins stage's memory may fall with it.
- `--kh` h1 and h2 wait for SPIKE-136.

## Visuals
None: a plumbing run with no judgement; the outputs are files for `river-gate`.

## 5. Addendum (#166a, 2026-11-17)
Runner `spike/rg-chain` at 000a0ea5: crusader's `spike/g1d` merged to be4a383f, spikes' `spike/kh-stage` merged (KH `c1 | h2 | h4 | h4c`, default `h4c`); runs below with `--kh c1`, so they compare with sections 2 and 3.

### 5.1 What changed in the runner
| Item | Change |
|---|---|
| Gap 1 | `river-gate --surface` merged (#1587): the gate reads the chain's final surface; every run below is gated on it |
| Gap 2 | the L stages (basins, KG-trib, KS1, KV, KC) run per domain: with `--regions 2` each region on its own window, its core plus 16 H1 cells (13 km), the window's border its exits except the entry cells, seeded with the outside H1 cell's `A_d` (crusader's G1d rule); a region keeps its core's cells and the items its core's tiles own; landmass items from every domain, different contents for one id counted; KG-trunk once (the landmass) |
| The cut | on a global tile boundary (the window's origin is not on one) |
| Junction markers | a `trib_junction` with no reach ending at it and no basin spilling to it is dropped after the stitch (one a domain made for a tributary in its halo) |
| Gap 3 | crusader's `kv_kc::flag_residuals`: every KV/KC residual's reach flagged `budget_residual` |
| Basins | crusader's `produce_ids` (reach vertices read and anchors written as global ids); two passes as in G1d: on the trunks for the kept set, then with every L reach as spill receivers; the graph takes the second pass's nodes only |
| KH | `--kh h2 \| h4 \| h4c` give KS the base `KH(z_s) + r`; `S0` stays the reference of the budget, the sills, the ridges and the gate (agreed with spikes) |

### 5.2 Correction to sections 2 and 3
The basins of the first runs (crusader's producer at 82aa5939) carried window-local anchors and ids: the producer wrote grid indices, which equal global ids on the G0 fixture only. Their counts stand; their ids and the gate's basin columns there do not. `produce_ids` writes global ids.

### 5.3 R1h, one region against two
| Measure | One region | Two regions |
|---|---|---|
| Final surface | - | bit-identical (`raw/surface-compare-addendum.txt`) |
| Canonical graph | 316,142 nodes, 209,212 reaches, 55,501 basins | byte-identical; river-gate's partition: identical |
| Landmass items with different contents in two domains | - | 0 |
| Junction markers dropped after the stitch | 0 | 15 |
| Seconds: basins / KG-trib (with the second basin pass) / KV and KC / the run | 136 / 222 / 164 / 722 | 138 / 229 / 419 / 988 |
| Peak working set, MiB (the basins stage) | 13,802 | 13,429 |
- Before crusader's be4a383f and the second-pass nodes fix the graphs differed in one marker's owner, and 99 `lake_inlet`s of the first basin pass collided with confluences, sources and tile crossings.
- KV and KC run on the whole window per domain (the module has no active mask): 2.6 times the one-region time.

### 5.4 The gate on the final surface (one region; `raw/gate-r1h-addendum.txt`)
| Part | R1h |
|---|---|
| Graph | 316,142 nodes; 211 violations, all two nodes on one anchor; no two non-marker nodes share an anchor; 93 anchors hold two or more `exception` markers and 1 two `trib_junction` markers |
| Before crusader's d72ee86b | 452 invalid markers (`trib_junction` on its reach's last vertex) and 992 two nodes on one anchor |
| Spill nodes not in the graph | 1,182, the producer's unresolved spills |
| Connectivity | 121,281 of 121,699 sources reach an outlet; 0 trunk breaks; 0 conservation breaks |
| Exceptions | 704 `rise_over_budget`, 27,980 `budget_residual` (29,495 KV/KC residuals name no reach within KV's reach) |
| Partition | identical |

### 5.5 Open
- The basins stage's peak, 13.8 GB on R1h, above the 12 GB budget; crusader's window example with one pass peaks at 7.9-8.1 GB (crusader's #159 item 1; the chain holds its other fields beside it).
- `exception` markers sharing an anchor (93): G1c's trunk graph (spikes).
- KV and KC per domain on the whole window: a cost, not a partition fault.
