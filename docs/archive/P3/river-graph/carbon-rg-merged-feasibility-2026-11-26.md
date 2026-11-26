# Merged Feasibility, Process & Pipeline-Complexity Synthesis: Carbon vs. Fable (2026-11-26)

- **Compared Reviews:**
  - **Carbon:** [carbon-rg-feasibility-2026-11-26.md](file:///c:/dev/ag_serious-planet-builder/review/carbon-rg-feasibility-2026-11-26.md)
  - **Fable:** [fable-rg-feasibility-2026-11-26.md](file:///C:/DEV/SeriousPlanetData/lanes/digest/review/fable-rg-feasibility-2026-11-26.md)
- **Additional Stage-Ablation Evidence:** [g2-report-2026-11-22.md](file:///C:/DEV/SeriousPlanetData/lanes/digest/river-graph/g2-report-2026-11-22.md), [review-response-2026-11-21.md](file:///C:/DEV/SeriousPlanetData/lanes/digest/river-graph/review-response-2026-11-21.md), [g0-contracts.md](file:///C:/DEV/SeriousPlanetData/lanes/digest/river-graph/g0-contracts.md), [spikes-outbox.md](file:///C:/DEV/SeriousPlanetData/lanes/spikes-outbox.md#L1741-L1872), and `origin/main:openspec/specs/compute-budgets/spec.md`.
- **Independence:** Written after both standalone reviews were finalized, strictly without reading any merged comparison file from Fable.

---

## 1. Cross-Model Consensus: Independent Blind Convergence (`>95%` Combined Confidence to Park Now)

Written independently without shared notes, Carbon and Fable reached the **same strategic verdict**, the **same look attribution**, and the **same process diagnosis**:

| Dimension | Carbon (`carbon-rg-feasibility`) | Fable (`fable-rg-feasibility`) | Merged Verdict |
|---|---|---|---|
| **Strategic call (`P0`)** | **Reject `A` (continue G3) and `B` (hybrid).** Recommend **`C+`**: 30-min `fly1126` flight, harvest Wave-1 `G3-6` memory wins (`<= 2` d), freeze `spike/rg-prototype`. | **Reject `A` ("not reachable") and `B` ("amended gate" w/ `1,250` trenches).** Recommend **`C` now (`~70%` conf.), then `D` (`S0` base work)**; 30-min `fly1126` flight. | **UNANIMOUS: Park `spike/rg-prototype` now (`C+`).** Combined confidence rises from Fable's `70%` to **`>95%`** once Carbon's `G3-7` editor RAM/latency blocker (§2.1) is included. |
| **Is the River-Graph idea sound?** | **Yes** (authority, `0` net budget overruns, `GEN-191` basins, `80x–93x` fewer `>50 m` cuts than `B`, `0%` vs `57.7%` `R3a` pits). | **Yes** (`94.5–99.5%` `75 km²` agreement, stable basin IDs/spills, `Wave 1` `9.26 GB` full-count pass). | **Sound core thesis; broken 9-stage L repair chain and incompatible boundary constraints.** |
| **Can `rg_chain.rs` pass G3 honestly?** | **No** (`0/9` rows passed in full; `6–10+` real weeks left; fails `G3-7` editor RAM/latency outright). | **No**—only reachable by shrinking/withdrawing bars (`O10, O13, O14, O15, O16`), violating `CON-31` (`8–12` lane-weeks + port). | **Unreachable without gutting `CON-31` and breaking `editor` usability.** |
| **Root cause of `G3-9` look failure** | **`88–93%` upstream `S0`** (`base1` `56%` `F1` comb + `K5` `3.5–16 m` washboard + `K7` `C1` dams + `3.8k–7.1k` H1-vs-L cols). | **`88–93%` upstream `S0`/`K5`** (`4/5` flagged defects; `0.96–0.97` washboard corr.; `3,771–7,087` cols; `K5'` failed `8/18`). | **Self-contradictory plan:** gating RG on `S0`'s look while parking `S0` post-port (`O5`/`O16`), when fixing `S0` rebaselines all of RG. |
| **Process & metric failures** | `G2` closed unflown (`SF1`); `0/5` `TEST-155` look bars valid (`SF2`); `48` readings / `96` switches (`SF3`); `3` 24/7 lanes outrunning owner (`SF4`); `G3-7` sequenced last (`SF5`). | `10` defect-moving fix rows in `5` days (`P1`); `5/7` look bars failed `TEST-155` + circular candidate floors (`P2, P3`); decisions reopened in `48 h` (`P4`); `G2` closed unmet (`P5`); `~440k` words / `95` orders (`P6–P8`). | **Identical root diagnosis:** autonomous 3-lane local patching inside a frozen upstream box generated `440k` words and `+38k` LOC of repair loops. |

> **Why Fable's single caveat (`"What would change the verdict: a 30-min flight where step 1's look is acceptable"`, `fable:67`) no longer rescues `Option B`:** Fable missed the `BUDG-003`/`BUDG-005` editor constraints on `G3-7` (§2.1). Even if you flew `fly1126` today and accepted `1,250` mountain trenches and `K5` washboard, porting the current architecture still fails interactive editing (`35 min` to `8+ hours` settle time and `9.3–14 GB` RAM vs. `4.0 GB` cap). Fly `fly1126` for learning and `F9` repros, not to revive `Option B`.

---

## 2. Key Divergences & Blind-Spot Adjudication (What Each Review Caught That the Other Missed)

### 2.1 [CRITICAL — Carbon Catch / Fable Blind Spot] `G3-7` Interactive Editor Viability (`BUDG-003`, `BUDG-005`, `ADR 0112`/`0113`)

- **Fable's assessment ([fable-rg-feasibility-2026-11-26.md](file:///C:/DEV/SeriousPlanetData/lanes/digest/review/fable-rg-feasibility-2026-11-26.md#L53)):** Classified `CON-29` incremental edits as *"Real but designed for... Not a reason to park on its own; a reason the port is large"*, citing only `N20` (`18,615` graph items moving `215–252 km`). Fable did not check `openspec/specs/compute-budgets/spec.md` or `ADR 0113`'s commit-swap and layer-ordering rules.
- **Carbon's falsification ([carbon-rg-feasibility-2026-11-26.md](file:///c:/dev/ag_serious-planet-builder/review/carbon-rg-feasibility-2026-11-26.md#L44-L62)):**
  - `BUDG-003` caps editor background terrain on Rasierklinge to **`6` threads** (`~2.2x` slower than `16`-thread `heavy --exclusive` runs); `BUDG-005` caps total terrain RAM to **`4.0 GB`** inside `8.0 GB` (`CON-21`), leaving **`<= 2.0–2.5 GB`** after streaming/mesh pools.
  - `ADR 0113` pt 1 places height/sculpt edits **below erosion**, `ADR 0112` pts 1, 6 + `CON-29` force **exact bit-equal landmass re-routing** (no quantization stopping rule), and `ADR 0113` pt 6 freezes visible terrain on the old commit until all dirty tiles finish.
  - Against today's (`2026-11-26 21:00`) measured runs: full-count `Wave 1` (`15,812 s` / `9.26 GB` on `16`t) takes **`~8–9 hours` on `6` threads and `3.7x` over-allocates `BUDG-005`** (`K6` alone = `~40 min` / `9.26 GB`); even an L-only shortcut takes **`35–68 min` per `8,160²` window on `6` threads** and **`11.8–14 GB` RSS** in `basins`, while `relief10` dirties a `25 km` radius (`4–9` tiles) on every edit and ridge/main-stem edits cascade `500–4,000 km` (`30–200+` tiles).
- **Adjudication:** **Carbon's finding is a hard architectural stopper.** `G3-7` *is* a reason to park on its own, and **amending `ADR 0112`/`0113` (`AP1`) must precede any future hydrology restart**—including after Fable's `Option D`.

---

### 2.2 [HIGH — Carbon Catch / Fable Blind Spot] `G3-4` Multi-Round Landmass L Joins vs. `ADR 0119` Canonical Tile Domains

- **Fable's assessment ([fable-rg-feasibility-2026-11-26.md](file:///C:/DEV/SeriousPlanetData/lanes/digest/review/fable-rg-feasibility-2026-11-26.md#L37)):** Treated the `12.5–16.5` lane-day 3-wave runner (`8`-round `band6` + cross-seam `KC` landmass joins) as required build work for `CON-30`.
- **Carbon's falsification ([carbon-rg-feasibility-2026-11-26.md](file:///c:/dev/ag_serious-planet-builder/review/carbon-rg-feasibility-2026-11-26.md#L68-L75)):** `ADR 0119` (`GEN-182`, accepted `2026-11-12`) already established that `CubePatch` (`tile + 128-cell halo`) is a local stage's **canonical `CON-06` domain**, waiving unpartitioned landmass bit-identity for `K7`. Building `12.5–16.5` lane-days of global L barriers just to synchronize **`30` `band6` breaches (`0.014%` of `209,014`) and `7` `KC` breaches (`0.07%` of `10,718`)** destroys `O(1)` tile locality and blocks progressive per-tile editor updates.
- **Adjudication:** **Adopt Carbon's `AB2`/`AP3`.** Global authority belongs *only* on the sparse topological graph (`Stage 1`); all L raster carving and pit polish (`Stages 2–3`) should be pure `ADR 0119` `CubePatch` tile functions with zero global barriers.

---

### 2.3 [HIGH — Fable Catch / Carbon Refinement] Hidden Unbudgeted Lowering in `KH` (`vs S_KH` vs. `vs S0`)

- **Fable's catch ([fable-rg-feasibility-2026-11-26.md](file:///C:/DEV/SeriousPlanetData/lanes/digest/review/fable-rg-feasibility-2026-11-26.md#L29)):** Highlighted that RG's `0.09–0.44 m` crest `p95` bar is measured against `S_KH`, whereas `KH` (`--kh-lower-only`, `reading 40`) lowers **`10–18%` of all land** outside the stage budget (`7.2–20.9 m` crest `p95` against `S0`, and `R3b` saddles `753` vs. `B`'s `723`, [g2-report-2026-11-22.md](file:///C:/DEV/SeriousPlanetData/lanes/digest/river-graph/g2-report-2026-11-22.md#L30-L36)).
- **Carbon's context:** Noted that `> 50 m` deep excavations against `S0` are still `80x–93x` lower in RG than `B` (`26.6k` vs. `2.48M` on `R1h`; `227k` vs. `18.4M` on `M1`), because `B` carves unbounded gorges while `KH` shaves `814 m` quad interiors uniformly.
- **Adjudication:** **Fable's catch strengthens pipeline simplification (§3):** `KH` is an expensive, relief-shaving workaround for `K7`'s `C1` diagonal crease dams. Replacing `RegionField::at` with a smooth `C2` read (`O12-A` in `Option D` / `AP2`) **eliminates `KH` completely**, recovering that lost `7–21 m` of `S0` crest height.

---

### 2.4 [MEDIUM — Tactical Fork] Fable's `Option E` (Partial RG-into-`B` Spike) vs. Carbon's `Option C+` (Wave-1 `G3-6` Harvest)

- **Fable's proposal ([fable-rg-feasibility-2026-11-26.md](file:///C:/DEV/SeriousPlanetData/lanes/digest/review/fable-rg-feasibility-2026-11-26.md#L63-L65)):** Spend `1` lane-day testing `Option E` (`KH` + per-cell budget + basin objects grafted onto `B`, without `KG-trib` or `KV`) before starting `Option D` (`S0` base work).
- **Carbon's counter-proof:**
  1. **Why `Option E` fails mechanically:** `B` has no graph bed solve; it lowers valleys by iterative stream-power/breach-and-burn on `L`. Clamping `B` to `budget(x) <= 10–50 m` where `S0` has `3,771–7,087` ridges/cols up to `150+ m` leaves every capped ridge as an **undrained inland pit** (reproducing `B`'s `57.68%` pit failure on `R3a`), while adding `KH` imports `KH`'s `10–18%` unbudgeted land shave right before `Option D` replaces `K7`'s read anyway.
  2. **What *should* be harvested (`<= 2` lane-days):** Fable listed `Wave 1`'s `9.26 GB` full-count pass as a reason *against* parking (`fable:65`), overlooking that **Wave 1 (`base1 + K5 + K6`) is `main`'s existing regional bake for `B`**. Harvesting `crusader`'s self-contained Wave-1 memory/parallelism work (`Grid::compact_cube`, `4x` parallel `base1`, capped `K5`, `61 B/cell` `K6`: `72 GB -> 9.26 GB`) directly upgrades `main` with zero RG coupling.
- **Adjudication:** **Reject `Option E`; adopt Carbon's Wave-1 `G3-6` harvest (`<= 2` lane-days) if clean on `cargo xtask test`.**

---

### 2.5 [MEDIUM — Complementary Catches on `Option D` (`S0` Base Fixes)]

- **Fable's catch ([fable-rg-feasibility-2026-11-26.md](file:///C:/DEV/SeriousPlanetData/lanes/digest/review/fable-rg-feasibility-2026-11-26.md#L62)):** Warned that `SPIKE-139`'s hillslope term in `K5` **lost `14–22%` of terrain relief**, so `K5` calibration may not converge quickly (`2–4` lane-weeks, open-ended).
- **Carbon's catch ([o16-base-fixes-scoping-2026-11-25.md](file:///C:/DEV/SeriousPlanetData/lanes/digest/river-graph/o16-base-fixes-scoping-2026-11-25.md#L72)):** Caught that **`base1`'s `1`-cell diagonal H1 breaches seed `56%` of `F1`'s diagonal comb (`0.034 -> 0.121` of `0.189`) before `K5` even runs**—missing from Fable's `K5 + K7` framing.
- **Adjudication:** Both reinforce **not** rushing `Option D` under `G3` pressure. When resumed, `Option D` (`AP2`) must cover **`base1` breach geometry + channel-guarded `K5` hillslope + `K7` `C2` read** on a **single R&D lane** with a relief-preservation gate.

---

## 3. Quantitative Proof of Your Pipeline-Sheet Observation: Stage Risk / Benefit Audit

Your observation from the review sheets—*that many separate stages in `rg_chain.rs` barely change the resulting terrain or river, yet carry massive complexity*—is **quantitatively confirmed** by the solo timings (`#162a`), depression-drain tables (`#163` `deps163.txt`, [spikes-outbox.md](file:///C:/DEV/SeriousPlanetData/lanes/spikes-outbox.md#L1747-L1809)), and gross-vs-net budget audit (`#199`, [g0-contracts.md](file:///C:/DEV/SeriousPlanetData/lanes/digest/river-graph/g0-contracts.md#L90)):

- **Headline Waste Ratio:** On `R1h` (`1,118 s` solo total), **`~82%` of runtime (`~927 s`) and `100%` of the `14 GB` RAM spike** occur in stages that either **change zero terrain pixels (`KG-trunk`, `KG-trib x2`, `budget`: `~310 s`)**, mark flat water on `0.6–3.4%` of cells (`basins x2`: `229 s`, `14 GB`), scratch `1`-cell pilot hairlines that clear `< 2%` of pits (`KS0`, `KS1`: `~40 s`), shave quads to hide `C1` creases (`KH`), or spend `271 s` (`8` global rounds) draining `S0`'s `10k–25k` checkerboard pits (`band6`) and refilling pits that `KV` itself just dug (`KC`)!
- **Self-Cancelling Work (`#199`):** On `M1`, `1,298` cells lowered by `KS0`/`KS1` are **filled right back up by `band6` and then cut a second time by `KV`/`KC`** ([g0-contracts.md](file:///C:/DEV/SeriousPlanetData/lanes/digest/river-graph/g0-contracts.md#L90)), while `KV` creates **`+1,727` brand-new pits** (`2,514 -> 4,241`, [spikes-outbox.md](file:///C:/DEV/SeriousPlanetData/lanes/spikes-outbox.md#L1800)) that `KC` then has to patch!

| # | Current Pass in `rg_chain.rs` | `R1h` Solo Time / Peak RAM | L Depressions Left (`R1h` / `F1` / `M1`) | Actual Visible Effect on Review Sheets | Defect / Complexity Tax (`6,425` LOC, `96` flags) | Benefit / Risk Ratio & Fate in Simplified 3-Stage Pipeline (`AP3`) |
|---|---|---|---|---|---|---|
| **1** | **`KH` (`h4c` + `h5` + `connect-4` #1)** | `10 s` (`83 s` pre-B3) / `2.8 GB` | `10,556` / `14,740` / `23,876` | Smooths `814 m` `C1` quad creases, **but shaves `7–21 m` off `10–18%` of land** outside budget. | High (`readings 19, 20, 40`; `kh-guard` x25 ponds; `r1h-kw` 4x `>150 m` cuts; cube-edge gap). | **LOW (Upstream Workaround) → DELETE** once `K7` uses a smooth `C2` read (`AP2`). |
| **2** | **`budget_field` (`relief10` `25 km`)** | `8–10 s` | unchanged | **None** (computes `10–50 m` cap). | High edit tax: **`25 km` L window dirties `50 x 50 km` (`4–9` tiles) per edit (`AB1`)**. | **POOR → FOLD INTO `K6` (`H1`)** as a cheap parent-cell lookup. |
| **3** | **`KS0` (1-cell trunk pilot cut + `connect-4` #2)** | ~`25 s` | `10,588` (**+32**) / `14,515` (`-1.5%`) / `25,324` (**+1,448**) | **Barely visible (`102 m` hairline);** clears `0–1.5%` of pits (adds `+1,448` pits on `M1`). | Medium: cuts cells that `band6` refills and `KV` cuts again (`#199`). | **VERY LOW → DELETE** (fold thalweg into `Stage 2` valley carve). |
| **4** | **`KG-trunk` (L trunk snap + beds)** | ~`15 s` | unchanged | **None on height** (graph polyline + beds). | High: routes blindly across `3.8k–7.1k` L cols (`O15`); imperative bed carries (`AB4`). | **ESSENTIAL AUTHORITY → MERGE INTO `Stage 1`** (sparse topological + declarative DAG bed solve). |
| **5** | **`basins` pass 1 & pass 2 (`--trib-rerun`)** | **`229 s` (`20.5%`)** / **`11.8–14 GB`!** | keeps `1,162` (`R1h`) / `508` (`F1`) lakes | Flat water mask on **`0.6–3.4%`** of area; **zero** ground height edit. | **EXTREME:** #1 OOM culprit (`14 GB`), run **2x**, causes `c1` `bed_not_shared` loops (`#203h/j/jb`). | **BAD COST/BENEFIT → REPLACE IN `Stage 1`** with `H1` kept basins + lightweight local L sill check (`< 1 GB`, run once). |
| **6** | **`KG-trib` pass 1 & pass 2 (`--trib-rerun`)** | **`287 s` (`25.7%`)** | unchanged | **None on height** (tributary graph + `q_w` + beds). | **HIGH:** Run **2x** (`46%` of total time w/ `basins`!), even after `--trib-once` passed `11/11` bars in `#162a`. | **ESSENTIAL AUTHORITY → MERGE INTO `Stage 1`** (run **once**, emit smooth `4`-connected chains directly). |
| **7** | **`KS1` (1-cell trib pilot cut + `connect-4` #3)** | ~`15 s` | `10,514` (`-0.7%`) / `14,241` (`-1.9%`) / `25,840` (**+516**) | **Nearly invisible (`1`-cell scratch);** leaves `98–100%` of pits and `39,871` residuals (`R1h`). | Medium: exists solely so `band6` sees tributary scratches. | **VERY LOW → DELETE** (`Stage 2` carves trunks + tributaries together). |
| **8** | **`band6` (`8`-round selective breach/fill)** | **`271 s` (`24.2%`)** | **`524` (`-95%`)** / **`268` (`-98%`)** / **`2,514` (`-90%`)** | Micro-breaches over hills + **`894` (`R1h`) to `4,495` (`M1`) ugly `10–50 m` hump trenches!** | **EXTREME:** `8` global seam-join rounds (`AB2`) + sole source of hump trenches (`#163`: `0` trenches when skipped). | **PURE `S0` DEFECT TAX → ELIMINATE GLOBAL PASS.** Clean `S0` (`AP2`) + local `Stage 3` `CubePatch` drain removes `8` global barriers. |
| **9** | **`KV` (Voronoi vertex valley carve + `connect-4` #4)** | ~`130 s` (`11.6%`) | `591` (**+67**) / `290` (**+22**) / `4,241` (**+1,727**!) | **THE ONLY HIGH-IMPACT LANDFORM STAGE:** shapes all multi-cell valley floors & walls. | High in current form: per-vertex Voronoi slices create `135°` ribs, corner links, and `+1,727` new pits on `M1`! | **CORE STAGE (`Stage 2`) → REDESIGN KERNEL:** `CubePatch` (`halo = 128`) continuous segment distance + `C1` smooth-min (`smin`) envelope (zero self-pits, zero `connect-4` patches). |
| **10** | **`KC` (post-KV breach/fill polish)** | ~`60 s` (`5.4%`) | `447` / `260` / `1,979` | **Barely visible** (`2.4k–6.4k` micro-patches refilling pits `KV` just dug). | Medium: cross-seam join (`7` breaches = `0.07%`) + fights `KV` (`Fable P1`). | **SHRINK TO `Stage 3` (`< 5 s`/tile):** pure tile-local `ADR 0119` `CubePatch` pit/flat cleanup, zero cross-tile joins. |

---

## 4. Unified Owner Action Plan (Synthesizing Carbon `C+` + Fable `C → D`)

### Immediate Close-Out (`<= 45 min` Owner Time, `0–2` Lane-Days, Zero Sunk-Cost Drag)
1. **Optional 30-Minute `fly1126` Flight (Both Reviews Agree):** Fly the 6 ready `fly1126` stores ([explorer-outbox.md](file:///C:/DEV/SeriousPlanetData/lanes/explorer-outbox.md#L2434-L2445)) once, capture `F9` repro bundles of what you like vs. reject under `#1662` normals, and archive them as ground-truth Tier-D fixtures.
2. **Bank Only Clean `main` Wins (`Option C+`, Skip `Option E`):** Keep the `36` already-merged `main` PRs (`#1632`–`#1669`). Optionally give `crusader` `<= 2` lane-days to land only the candidate-neutral Wave-1 `G3-6` memory/parallelism reduction (`72 GB -> 9.26 GB`, `4x` `base1`) if green on `cargo xtask test`; do **not** run `Option E` hybrid `B` experiments.
3. **Park & Tag Cleanly (`plan.md:162`):** Tag `spike/rg-prototype`, write a 1-page **Component Decision Record + `ADR: River-Graph Prototype Parked`** recording what is proven (sparse graph authority, `GEN-191` basin identity, `ADR 0119` `CubePatch` primitives, `TEST-155` metric gate) vs. what is retired (`6,425`-line 10-pass `rg_chain.rs`), and **idle the RG lanes**.

### Strict Order of Operations If/When You Resume Terrain & Hydrology Later
1. **Gate 0 — Editor Latency & RAM Architecture (`AP1`, Owner + Architect Only, `1–2` days):** Amend `ADR 0112`/`0113` so interactive editor sculpting operates **above `K5` (on `S0`)** inside a bounded local watershed/corridor (`<= 1 GB` RAM on `BUDG-005`, **`< 2–10 s`** on `6` `BUDG-003` threads, quantized `0.25 m`/`0.5 m` stopping rule, progressive tile swaps), keeping `Wave 1` as an explicit offline/modal world bake.
2. **Phase 1 — Standalone `S0` Base Quality Milestone (`AP2` / Fable `Option D`, `1` R&D Lane):** Fix `base1` `1`-cell diagonal breaches + channel-guarded `K5` hillslope/planform (with a hard relief-loss bar addressing Fable's `SPIKE-139` `14–22%` warning) + `K7` `C2` read (`O12-A`, deleting `KH`). Benefits `B` on `main` immediately even if RG stayed parked forever.
3. **Phase 2 — Clean-Sheet 3-Stage RG v2 (`AP3` + `AP4`, `1` R&D Lane, `<= 1,000` LOC, `<= 5` switches):**
   - **Stage 1 (Sparse Decide):** Single pass on clean `S0` + sill-consistent `H1` -> kept basins + smooth `4`-connected centrelines + **one declarative monotone bed DAG solve**.
   - **Stage 2 (Tile Valley Carve):** Pure `ADR 0119` `CubePatch` (`halo = 128`) continuous segment-distance `C1` smooth-minimum (`smin`) valley/thalweg carve (replaces `KH`, `KS0`, `KS1`, `KV`, and all `4` `connect-4` passes).
   - **Stage 3 (Tile Pit Cleanup):** Pure `ADR 0119` `CubePatch` local pit/flat polish (`< 5 s`/tile, zero global `band6`/`KC` barrier waves).
