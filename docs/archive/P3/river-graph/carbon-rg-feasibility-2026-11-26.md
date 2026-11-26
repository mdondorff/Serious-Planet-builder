# Adversarial Feasibility, Architecture & Sunk-Cost Review: River Graphs (Carbon, 2026-11-26)

- **Baseline:** `origin/main` @ `700f7f488` (`#1669`, `docs/constitution.md` v1.7 `CON-01`..`CON-31`); `origin/spike/rg-prototype` @ `5e66aa104` (`G3-6` `k=4` run @ `d9c320dd7`; `537` commits, `+38,149` lines across `129` files, `rg_chain.rs` at `6,425` lines / `96` CLI switches, `48` contract readings).
- **Primary Inputs:** [g3-snapshot-2026-11-26.md](file:///C:/DEV/SeriousPlanetData/lanes/digest/river-graph/g3-snapshot-2026-11-26.md), [00-open-decisions-2026-11-26.md](file:///C:/DEV/SeriousPlanetData/lanes/digest/decisions/00-open-decisions-2026-11-26.md), [g3-gate-2026-11-23.md](file:///C:/DEV/SeriousPlanetData/lanes/digest/river-graph/g3-gate-2026-11-23.md), [g2-report-2026-11-22.md](file:///C:/DEV/SeriousPlanetData/lanes/digest/river-graph/g2-report-2026-11-22.md), [g0-contracts.md](file:///C:/DEV/SeriousPlanetData/lanes/digest/river-graph/g0-contracts.md), [attribution-184w-2026-11-25.md](file:///C:/DEV/SeriousPlanetData/lanes/digest/river-graph/attribution-184w-2026-11-25.md), [o16-base-fixes-scoping-2026-11-25.md](file:///C:/DEV/SeriousPlanetData/lanes/digest/river-graph/o16-base-fixes-scoping-2026-11-25.md), [g3-6-full-count-plan-2026-11-26.md](file:///C:/DEV/SeriousPlanetData/lanes/digest/proposals/g3-6-full-count-plan-2026-11-26.md), and `origin/main` ADRs `0110`–`0119` + `compute-budgets` / `generator-pipeline` specs.
- **Independence:** Written strictly without reading `fable-rg-feasibility-2026-11-26.md`.

---

## 1. Executive Verdict: Stop `Option A` Now — Separate a Sound Graph Thesis From an Impossible 3-Way Over-Constraint

### 1.1 Direct Sunk-Cost Verdict on `P0` ([00-open-decisions-2026-11-26.md](file:///C:/DEV/SeriousPlanetData/lanes/digest/decisions/00-open-decisions-2026-11-26.md#L7-L16))

- **Reject `Option A` (continue G3 as planned): Sunk-cost trap.**
  - Zero of nine G3 gate rows have passed in full after `537` spike commits (`168` since G2; [g3-snapshot-2026-11-26.md](file:///C:/DEV/SeriousPlanetData/lanes/digest/river-graph/g3-snapshot-2026-11-26.md#L30)).
  - The lead's remaining `26–43` lane-day estimate ([g3-snapshot-2026-11-26.md](file:///C:/DEV/SeriousPlanetData/lanes/digest/river-graph/g3-snapshot-2026-11-26.md#L67)) explicitly assumes *"if no row loops back"* (every complex row so far looped 3–5 times), excludes `6–14+` open-ended lane-days for `S0`/`K5` look fixes (`88–93%` of rejected look defects), and excludes the multi-PR production port itself—putting real completion at **6–10+ calendar weeks of heavy owner supervision**.
  - Decisive stopper: **Even if `G3-1..6, 8, 9` all passed, the current architecture fails interactive editing (`G3-7`) by `2x–5x` in RAM (`BUDG-005` `4.0 GB` terrain share) and `15 min to 4+ hours` in settle latency (`BUDG-003` `6` editor threads + `ADR 0112`/`0113` `CON-29`).**
- **Reject open-ended `Option B` (grafting partial L-graph stages onto `B` now):**
  - Almost every candidate-neutral win is **already on `origin/main`** (`GlobalCell` `#1654`, `SpillGraph::connect_seams` `#1655`, `CubePatch` `#1658`, `cell_area` `#1661`, `REND-068` bilinear normals `#1662`, connected-sea gate `#1644`, `TEST-155` metric calibration `#1665`, `look-sheet` `#1650`, `VIEW-075`/`076` `#1629`/`#1633`).
  - Grafting half of RG's L-chain onto `B` builds a third hybrid pipeline that still inherits `S0`'s washboard and still hits the `G3-7` edit wall. The **only** spike code worth a fast harvest (`<= 2` lane-days) is `crusader`'s Wave-1 `G3-6` memory/parallelism work (`Grid::compact_cube`, `4x` parallel `base1`, capped `K5`, `61 B/cell` `K6`), which serves `B`'s regional bake directly.
- **Recommend `Option C+` (Clean Park + Optional 2-Day Wave-1 Memory Harvest + One 30-Minute `fly1126` Flight):**
  - Spend `30 minutes` flying the 6 already-built `fly1126` stores ([explorer-outbox.md](file:///C:/DEV/SeriousPlanetData/lanes/explorer-outbox.md#L2434-L2445)) and saving `F9` bundles so you see `B` vs. `RG` first-hand under `#1662` lighting, then **freeze `spike/rg-prototype`**.
  - Record hard restart preconditions (`AP1`–`AP4`, §5): **`rg_chain.rs` is never resumed as a 9-stage L repair chain under landmass-wide `CON-29` edit rules.**

### 1.2 Is the River-Graph Network Approach Still Feasible in Principle?

- **Yes—the topological river/lake graph thesis is sound and proven superior to `B`; what failed is trying to satisfy three mutually incompatible constraints simultaneously:**
  1. Treat corrupted `814 m` `base1 + K5 + K7(C1)` output (`S0`) as immutable while expecting `L` (`101.8 m`) river carving to look clean (`AB3`);
  2. Enforce unpartitioned landmass bit-identity (`G3-4`) on multi-round iterative `L` raster drains instead of `ADR 0119` canonical tile domains (`AB2`); and
  3. Require interactive below-erosion edits (`ADR 0113` + `CON-29`) to re-solve continent-scale erosion/hydrology bit-for-bit inside a `4.0 GB` / `6`-thread editor (`AB1`).

| Capability / Metric | RG (`g1f15` / `G3`) | `B` (Production on `main`) | Source |
|---|---|---|---|
| **Crest preservation (`p95` lowering vs `S0`)** | `7.2–20.9 m` (`0.09–0.44 m` vs `S_KH`) | `15.6–316.1 m` (`3x–15x` worse) | [g2-report-2026-11-22.md](file:///C:/DEV/SeriousPlanetData/lanes/digest/river-graph/g2-report-2026-11-22.md#L28-L35) |
| **`10 km` relief lost (`R1h` / `M1`)** | `-1.18 m` / `+0.11 m` | `-9.69 m` / **`-29.80 m`** | [g2-report-2026-11-22.md](file:///C:/DEV/SeriousPlanetData/lanes/digest/river-graph/g2-report-2026-11-22.md#L30-L34) |
| **Cells cut `> 50 m` (`R1h` / `M1`)** | `26.6k` / `227k` | **`2.48M`** / **`18.4M`** (`80x–93x` worse) | [g2-report-2026-11-22.md](file:///C:/DEV/SeriousPlanetData/lanes/digest/river-graph/g2-report-2026-11-22.md#L35) |
| **Honest connected-sea gate on `R3a`** | `11/11` bars (`0.00%` uphill, `0.00%` ponds) | **`57.68%` undrained inland pits**, `12.85%` uphill | [g2-report-2026-11-22.md](file:///C:/DEV/SeriousPlanetData/lanes/digest/river-graph/g2-report-2026-11-22.md#L82-L83) |
| **Persistent river/lake graph objects (`R4a`/`R4b`)** | Stable reach/basin IDs, monotone beds, spill levels | None in `fly1126` `b8-*` stores (no ribbons) | [explorer-outbox.md](file:///C:/DEV/SeriousPlanetData/lanes/explorer-outbox.md#L2437-L2454) |
| **Net per-cell stage budget (`O3` / `R3`)** | **`0` net overruns** across all 5 windows | Unbounded valley excavation | [g3-gate-2026-11-23.md](file:///C:/DEV/SeriousPlanetData/lanes/digest/river-graph/g3-gate-2026-11-23.md#L55) |

---

## 2. Deep Dive on `G3-7` (`AB1`): Why Interactive Editing Fails Under Current Rules

### AB1 [CRITICAL BLOCKER] — Below-Erosion Landmass `CON-29` Re-Solves Violate Both Editor RAM (`BUDG-005`) and Interactive Settle Latency (`15 min` to `4+ hours`)

- **Governing rules currently in force on `origin/main`:**
  - `ADR 0113` pt 1 (`origin/main:docs/adr/0113-edit-ordering-and-dependencies.md:14`): Landform height edits (sculpt, raise, lower, stamps) and driver edits sit **below erosion** (`B(d)` at `K2`/`K3`, upstream of `Wave 1` and `Wave 2/3`).
  - `ADR 0112` pts 1, 6 (`origin/main:docs/adr/0112-erosion-solver-regions-rounds-catchments.md:14, 19`) + `ADR 0113` pt 4 (`CON-29`): Routing domain is the **entire connected landmass (`region`)**; propagation stops **only at bit-equality of stored inputs, never at a tolerance** (*"`quantised to zero` is not a stopping rule"*, `origin/main:docs/design/terrain-target.md:470`).
  - `ADR 0113` pt 6 (`origin/main:docs/design/terrain-target.md:476-479`): All-or-nothing commit swap—the viewer draws the **previous** commit until all dirty stages and drawn tiles of the new commit finish (`Level 0/1` previews show only raw stamps + `3.26 km` `H0` lines with zero `L` valleys, lakes, or rivers; `:568-585`).
  - `BUDG-003` & `BUDG-005` (`origin/main:openspec/specs/compute-budgets/spec.md:41-80`): In the editor on Rasierklinge (`6 P-cores / 12 threads`), terrain background work is capped at **`6` threads** (`~2.2x` slower than `16`-thread `heavy --exclusive` runs, shared with live LOD tile streaming) and **`4.0 GB` total terrain RAM** inside `8.0 GB` (`CON-21`)—leaving **`<= 2.0–2.5 GB`** once resident tile/mesh pools are loaded.

- **Measured `2026-11-26` compute & memory vs. Editor `BUDG-003` / `BUDG-005` limits:**

| Stage / Domain | Measured `16`-Thread Offline Cost | Editor Reality (`6` Threads, `<= 2.5 GB` Free Terrain RAM) | Why `CON-29` Keys Fail to Localize Edits |
|---|---|---|---|
| **Wave 1 `base1` (`k=4`, `1.489e8` H1 cells)** | **`9,424 s` (`2.62 h`)**, `6.53 GB` RSS ([crusader-outbox.md](file:///C:/DEV/SeriousPlanetData/lanes/crusader-outbox.md#L2689)) | **`~5–6 h`**, **OOMs `BUDG-005` (`2.6x` over)** | `11` global depression-join rounds; superlinear (`6.8x` time per `4x` cells, `:2659`); zero catchment memoization. |
| **Wave 1 `K5` (`4` rounds, `k=4`)** | **`4,541 s` (`1.26 h`)**, `8.45 GB` RSS ([crusader-outbox.md](file:///C:/DEV/SeriousPlanetData/lanes/crusader-outbox.md#L2690)) | **`~2.5–3 h`** (`~25–40 min` routing floor even if `99%` catchments hit), **OOMs `BUDG-005`** | `85–90%` of each round is landmass routing (`ADR 0112` Context); `terrain-target.md:441` measured `GEN-143` incremental route `2.2x` *slower* than resident route (`1.58 s` vs `0.71 s`). |
| **Wave 1 `K6` (`h1_from_owned`, `k=4`)** | **`1,079 s` (`18 min`)**, **`9.26 GB` RSS / `9.78 GB` commit** ([crusader-outbox.md](file:///C:/DEV/SeriousPlanetData/lanes/crusader-outbox.md#L2691)) | **`~38–45 min`**, **OOMs `BUDG-005` (`3.7x` over)** | Single region-wide `K6` key (`terrain-target.md:459`); full landmass weighted flood + route + hydrology on every regional change. |
| **Wave 2 Portal Trunks (`8.4e7 km²` landmass)** | `5.2–8.9 GB` exact (`11.6–19.2 GB` parsed `Vec`s) ([g3-gate-2026-11-23.md](file:///C:/DEV/SeriousPlanetData/lanes/digest/river-graph/g3-gate-2026-11-23.md#L66)) | **OOMs `BUDG-005` (`2x–7x` over)** | Landmass-wide trunk accumulation (`reading 18`) + global spill join (`GEN-191`) before `L-B` tiles can start. |
| **L Stages (`1` `8,160²` window = `66.6M` L cells ≈ `830 km` box)** | **`958–1,847 s` (`16–31 min`)**, **`12–14 GB` RSS** in `basins` ([g2-report-2026-11-22.md](file:///C:/DEV/SeriousPlanetData/lanes/digest/river-graph/g2-report-2026-11-22.md#L23), [explorer-outbox.md](file:///C:/DEV/SeriousPlanetData/lanes/explorer-outbox.md#L2505)) | **`~35–68 min`** per window-equivalent, **OOMs `BUDG-005` (`5x` over)** | (1) Every height edit dirties `relief10` across a **`25 km` radius (`50 x 50 km` = `4–9` `1,024²` tiles; [g0-contracts.md](file:///C:/DEV/SeriousPlanetData/lanes/digest/river-graph/g0-contracts.md#L83))**; (2) any ridge capture or main-stem dam alters `q_w`, `W_floor`, `bed`, or `spill` for **`500–4,000 km` downstream (`30–200+` tiles)** across `8` `band6` barrier rounds + `2x` `KG-trib`/`basins-2` passes. |

- **(a) Concrete failure scenario:** An author lowers a mountain saddle by `25 m` or raises a valley dam by `30 m` on a continental landmass in `editor` mode. Under `ADR 0112` + `ADR 0113` + `CON-29`, background re-solve either crashes/thrashing-swaps past `BUDG-005` (`9.3–14 GB` vs. `4.0 GB` terrain cap) or runs for **`30 minutes` (L-only shortcut) to `4+ hours` (full below-erosion `Wave 1 + Wave 2/3`)** on `6` threads while `ADR 0113` pt 6 holds the rendered terrain frozen on the pre-edit commit.
- **(b) Cost to reverse if built:** Very High (`3–5 weeks` of wasted cache-key, wave-runner, and commit-swap plumbing across `generators`, `cache`, `streaming`, and `editor`).
- **(c) Cheapest falsification test (`CON-17`):** Run one `-25 m` ridge-capture edit on `k=2` (`1/4` landmass) + one `R1h` window pinned to `6` threads (`BUDG-003`) with `mem_stress`'s counting allocator hard-capped at `2.5 GB` (`BUDG-005` net of tile pools). Cost: `< 1` hour, zero new infrastructure; guaranteed to fail both RAM and `< 60 s` usability.

---

## 3. Three Additional Unquestioned Architectural Blocks (`AB2`–`AB4`)

### AB2 [HIGH] — Refusing `ADR 0119` Canonical Tile Domains for L-Grid Conditioning (`G3-4` Over-Constraint)

- **Fact:** On `2026-11-12`, the owner accepted `ADR 0119` (`origin/main:docs/adr/0119-k7-canonical-tiling.md:20-35`, `GEN-182`), ruling that `tile + 128-cell halo` is **part of the local stage's canonical definition (`CON-06`)**, waiving unpartitioned landmass bit-identity for `K7`.
- **What went wrong in G3:** Instead of applying `ADR 0119` to RG's local terrain-carving/drain stages once the sparse river/basin graph is fixed, `G3-4` (`D1-3`) demanded bit-identity against an unpartitioned landmass L grid—requiring `8` global-barrier local/join/apply rounds for `band6` and cross-seam joins for `KC` just to resolve **`30` `band6` breaches (`0.014%` of `209,014`) and `7` `KC` breaches (`0.07%` of `10,718`)** ([cross-seam-breaches-2026-11-25.md](file:///C:/DEV/SeriousPlanetData/lanes/digest/proposals/cross-seam-breaches-2026-11-25.md#L5-L10)).
- **(a) Concrete failure scenario / broken invariant:** Multi-round global barriers couple every tile in a region at `L`, turning independent `O(1)` tile bakes into synchronized landmass waves (`8–12` lane-days still unbuilt in `G3-4`/`G3-6`, [g3-snapshot-2026-11-26.md](file:///C:/DEV/SeriousPlanetData/lanes/digest/river-graph/g3-snapshot-2026-11-26.md#L62)) and preventing per-tile progressive edit commits.
- **(b) Cost to reverse if built:** High (`8–12` lane-days of 3-wave L-stage extraction and global join machinery).
- **(c) Cheapest falsification test (`CON-17`):** Measure seam visual/gate delta (`G-SEAM` protocol of `ADR 0119` pt 6) when `band6`/`KC` run purely inside `CubePatch` (`halo = 128`) after global sparse graph decide (`< 1` lane-day on `R1h`).

---

### AB3 [HIGH] — Treating Corrupted `S0` (`base1` + `K5` + `K7` C1 Read) as Immutable While Stacking 9 L-Stage Repair Passes

- **Evidence:** `S0` enters the river chain with four severe upstream defects:
  1. `C1` interpolation of `814 m` `z_s` dams `13.3%` of land in pits along diagonals (`origin/main:docs/adr/0110-the-authoritative-surface.md:9`);
  2. `K5` stream power without hillslope coupling creates `3.5–16 m` `814 m` checkerboard washboard (`origin/main:spikes/k5-washboard/results/results.md:75`), driving **`88–93%`** of Step 1's visual failures ([attribution-184w-2026-11-25.md](file:///C:/DEV/SeriousPlanetData/lanes/digest/river-graph/attribution-184w-2026-11-25.md#L19-L33));
  3. `base1`'s `1`-cell diagonal H1 breaches seed `56%` of `F1`'s diagonal comb (`0.034 -> 0.121` of `0.189`) before `K5` runs ([o16-base-fixes-scoping-2026-11-25.md](file:///C:/DEV/SeriousPlanetData/lanes/digest/river-graph/o16-base-fixes-scoping-2026-11-25.md#L72));
  4. `H1` trunks routed at `814 m` cross **`3,771` (`R1h`) to `7,087` (`F1`) `L`-scale ridges ("cols")** up to `150+ m` high (`O15`), while naive static-sill `K5'` failed `8/18` ([o16-base-fixes-scoping-2026-11-25.md](file:///C:/DEV/SeriousPlanetData/lanes/digest/river-graph/o16-base-fixes-scoping-2026-11-25.md#L122-L123)).
- **(a) Concrete failure scenario / broken invariant:** Holding `S0` fixed forces `RG` into an impossible dilemma: either carve `150–380 m` mountain trenches and pit-chains across washboard crests, flood `+3,000` to `+11,600 km²` of valleys into artificial lake chains, or overwrite `S0` only inside an `8`-cell river corridor while leaving washboard right beside it. Meanwhile, **falling back to `B` does not escape `S0`**—`B` shares the same `S0` plus `15x` worse crest destruction and `57.7%` `R3a` pits.
- **(b) Cost to reverse if built:** `6–14+` lane-days of open-ended upstream recalibration *plus* full re-tuning and 15-window rebaselining of every downstream RG parameter tuned to corrupted `S0` ([o16-base-fixes-scoping-2026-11-25.md](file:///C:/DEV/SeriousPlanetData/lanes/digest/river-graph/o16-base-fixes-scoping-2026-11-25.md#L98-L100)).
- **(c) Cheapest falsification test (`CON-17`):** Already confirmed by `#184w` (`88–93%` `S0` attribution) and `#184v` (`K5'` `8/18` failure).

---

### AB4 [HIGH] — Four Competing Hydrological Authorities & Imperative Post-Hoc Bed Mutation Instead of One Declarative DAG Solve

- **Evidence:** Channel paths, kept lakes, and bed elevations are mutated across **four separate authorities** (`K5`, `K6`, `KG-trunk`+`band6`, and `KG-trib`+`basins-2` run twice via `--trib-rerun`) plus imperative fix-up loops (`lift`, `floor_inlet_beds`, `carry`, `4` `connect 4` passes).
- **Live proof tonight (`2026-11-26 21:15`, [explorer-outbox.md](file:///C:/DEV/SeriousPlanetData/lanes/explorer-outbox.md#L2482-L2540)):** Hours after `R23` was declared "frozen as reading 48", `c1` failed `bar_violations` (`bed_rise` + `bed_not_shared`); fixing `bed_rise` in `#203j` left `bed_not_shared` because `floor_inlet_beds` lifted a trunk vertex (`162.0 -> 173.5 m`) above a `trib_junction` (`324`), requiring yet another patch (`junction_stops`, `#203j b`).
- **(a) Concrete failure scenario / broken invariant:** Imperative post-hoc bed carries create non-terminating corner-case interactions whenever two basins, junctions, or budget clamps meet on one reach (`readings 32, 35, 41, 42, 44, 48`).
- **(b) Cost to reverse if built:** High—every new feature (`R23` intended basins, `O8` Row 3 col spills, `R4` coastal rule) triggers multi-day violation whack-a-mole across `6,425` lines of `rg_chain.rs`.
- **(c) Cheapest falsification test (`CON-17`):** Count coupled bed/inlet readings (`>= 11` of `48` readings in [g0-contracts.md](file:///C:/DEV/SeriousPlanetData/lanes/digest/river-graph/g0-contracts.md#L138-L201)) and `c1`'s `#203h -> #203j -> #203jb` cascade on a single calibration window.

---

## 4. Five Systematic Process Failures (`SF1`–`SF5`)

| ID | Systematic Failure | Concrete Evidence | Cost Incurred | Cheapest Prevention Rule (`CON-17`) |
|---|---|---|---|---|
| **SF1** | **Closing gates without Tier-D human flight** | `G2` closed `2026-11-22` with `0` owner flights and 5 open misses ([g2-report-2026-11-22.md](file:///C:/DEV/SeriousPlanetData/lanes/digest/river-graph/g2-report-2026-11-22.md#L58-L62)); `fly1126` stores ready since `07:45` today remain unflown. | `4+` calendar days (`~12` lane-days) spent optimizing before seeing `S0` washboard or `135°` mesh sawtooth (`#1662`). | Never close a candidate/look gate without a `20`-min owner flight + `F9` repros first. |
| **SF2** | **Registering uncalibrated scalar look proxies as hard gates** | `#1665` (`TEST-155`, [spikes-outbox.md](file:///C:/DEV/SeriousPlanetData/lanes/spikes-outbox.md#L3395-L3409)) proved **0 of 5** geometric look metrics valid: `Xdiag` failed `23/30` smooth valleys, `M4` `17/30`, `D8 sinuosity` `12/30`, `straight-run` `18/30` + missed `48/54` defects. | ~50% of post-G2 lane compute ([g3-snapshot-2026-11-26.md](file:///C:/DEV/SeriousPlanetData/lanes/digest/river-graph/g3-snapshot-2026-11-26.md#L52)) chased lattice-aliased numbers (`A5` `10 m` cap, `Xdiag`, `M4`). | `BUILD-069` (`TEST-155` synthetic multi-angle + `S0` baseline check) mandatory *before* any metric enters a gate. |
| **SF3** | **Accretive complexity via "readings" (`48`) & switches (`96`)** | `spike/rg-prototype` grew to `+38,149` lines across `129` files and `6,425` lines in `rg_chain.rs` ([g3-snapshot-2026-11-26.md](file:///C:/DEV/SeriousPlanetData/lanes/digest/river-graph/g3-snapshot-2026-11-26.md#L46-L49)). | Manual edit mutexes (`rg-chain-claims.md`), `35-min` prepushes, multi-stage bug cascades (`--trib-rerun`, `4x` `connect 4`). | Hard cap: `<= 1,000` LOC per file, `<= 5` active feature flags; conflict -> simplify stage contract, never add a repair pass. |
| **SF4** | **3 autonomous 24/7 lanes vs. part-time owner bandwidth** | Lanes cannot amend `S0`, `ADR 0112`/`0113`, or `CON-*`; during 24–48 h owner gaps they iterate furiously inside `rg_chain.rs`, generating `8+` queued owner briefs/day. | Rapid accumulation of sunk code and invalidated baselines whenever an upstream boundary (`O15`, `O16`) shifts. | Run **1** exploratory lane during R&D; scale to 3 lanes only for decoupled production PRs (`P1` primitives); hard auto-idle when `>= 2` owner forks are open. |
| **SF5** | **Sequencing kill-gates (`G3-7` edit latency, `S0` look) last** | `G3-7` (`CON-29` edit latency / `BUDG-005` RAM) sits `"not started"` ([g3-snapshot-2026-11-26.md](file:///C:/DEV/SeriousPlanetData/lanes/digest/river-graph/g3-snapshot-2026-11-26.md#L26)) after `537` commits, despite `Wave 1` taking `4.4 h` / `9.26 GB` and windows taking `16–31 min` / `12–14 GB`. | Risk of building an entire hydrology pipeline that is unusable in `editor` mode. | Back-of-the-envelope RAM + thread-scaled latency arithmetic (`BUDG-003`/`005`) is Gate **0**, before writing stage code. |

---

## 5. Strategic Options (`P0`) & Four Mandatory Preconditions (`AP1`–`AP4`) for Any Future Restart

### 5.1 Option Comparison Against Owner Effort & Sunk-Cost Risk

| Option | Owner Effort Now | Calendar / Compute Cost | What Lands on `main` | Verdict |
|---|---|---|---|---|
| **`A`: Continue G3 as planned** | Very High (`8+` decisions + multiple flights + base trade-offs) | `6–10+` real weeks (`26–43` d + `6–14` d base + port + loopbacks) | Full `rg_chain` port—**but fails `G3-7` editor RAM/latency (`AB1`)** | **REJECT** (Sunk-cost trap) |
| **`B`: Partial L-graph hybrid onto `B` now** | Medium-High (new hybrid design + look/gate arbitration) | `2–4` weeks | Hybrid pipeline inheriting `S0` washboard + `G3-7` edit bottleneck | **REJECT** (Second tar pit) |
| **`C+`: Clean Park + Wave-1 `G3-6` Harvest + 30-Min Flight** | **Minimal (`~30–45 min` total: 1 flight + 1 freeze sign-off)** | **`0–2` lane-days** | Keeps all 36 merged `main` PRs (`#1632`–`#1669`) + optional Wave-1 `72 GB -> 7.9 GB` / `4x` `base1` win; freezes `spike/rg-prototype` | **RECOMMENDED** |

### 5.2 Four Fundamental Changes (`AP1`–`AP4`) Required Before Unparking Hydrology

1. **`AP1` — Reframe Interactive Editing (`Amend ADR 0112`, `ADR 0113`, and `CON-29` Scope for Edits):**
   - Decouple **Offline/Modal World Bake** (`Wave 1` `base1 + K5 + K6` under `12 GB` / `16` threads) from **Interactive Editor Sculpting** (`BUDG-005` `4.0 GB`, `BUDG-003` `6` threads).
   - Default editor terrain sculpting acts **above `K5` (on `S0`)** with a **bounded local watershed/corridor re-grade** (`<= 1 GB` RAM, **`< 2–10 s` settle time**), stopping downstream propagation once quantized `0.25 m` heights and `0.5 m` bed codes stop changing, and swapping tiles progressively (`ADR 0072`) rather than freezing the world behind a continent-wide commit barrier.
2. **`AP2` — Fix `S0` Upstream First (`K7` C2 Read + `base1` Breach Width + `K5` Channel-Guarded Hillslope):**
   - Never resume L-stage river work on today's `S0`. First pass an upstream `S0` milestone (benefiting `B` immediately) that eliminates `base1`'s `1`-cell diagonal breach beads, couples `K5` diagonal hillslope parities without filling `1`-cell channels, and replaces `RegionField::at` with a smooth `C2` quasi-interpolant (`O12-A`).
3. **`AP3` — Replace the 9-Stage `rg_chain.rs` Repair Pipeline With a 3-Stage Architecture (`Extend ADR 0119`):**
   - **Stage 1 (Sparse Topological Decide):** Single pass over `S0` + `H1` solving kept basins (`GEN-191`), smooth `4`-connected centrelines, and **one declarative monotone bed DAG solve** (eliminating `--trib-rerun`, `floor_inlet_beds`, and `4x` `connect 4` repair loops).
   - **Stage 2 (Canonical `ADR 0119` Tile Valley Carve):** Single `CubePatch` (`halo = 128`) smooth-minimum valley/thalweg carve (`C1` smooth-min, smoothed wall slope)—zero global barriers.
   - **Stage 3 (Canonical `ADR 0119` Local Tile Pit/Spill Cleanup):** Pure `CubePatch`-domain local pit cleanup (`< 5 s`/tile instead of `8`-round landmass `band6`/`KC` joins).
4. **`AP4` — Enforce Lean Single-Lane R&D Governance:**
   - Gate 0 = `BUDG-003`/`005` latency/RAM math + `TEST-155` metric calibration; single R&D lane (`<= 1,000` LOC, `<= 5` switches) until owner flight passes; automatic lane idle whenever `>= 2` owner architecture decisions are pending.
