# Detailed Execution Plan: Parking River Graphs (`Option C+`)

- **Date:** `2026-11-26`
- **Target Baseline:** `origin/main` @ `700f7f488` (`#1669`); `origin/spike/rg-prototype` @ `6d53e75d8`
- **Basis:** [carbon-rg-merged-feasability-2026-11-26.md](file:///c:/dev/ag_serious-planet-builder/review/carbon-rg-merged-feasability-2026-11-26.md), [carbon-rg-feasibility-2026-11-26.md](file:///c:/dev/ag_serious-planet-builder/review/carbon-rg-feasibility-2026-11-26.md), [fable-rg-feasibility-2026-11-26.md](file:///C:/DEV/SeriousPlanetData/lanes/digest/review/fable-rg-feasibility-2026-11-26.md), and [00-open-decisions-2026-11-26.md](file:///C:/DEV/SeriousPlanetData/lanes/digest/decisions/00-open-decisions-2026-11-26.md).

---

## 1. Objective & Hard Guardrails (Preventing "Park Drift")

1. **Full Stop on All Hydrology & Fluvial Base Work Now:**
   - Override the lead's preliminary sequence (*"C, then D, with E scoped first"*, [00-open-decisions-2026-11-26.md](file:///C:/DEV/SeriousPlanetData/lanes/digest/decisions/00-open-decisions-2026-11-26.md#L11)):
     - **Execute `C+` (Clean Park + Wave-1 `G3-6` Memory/Speed Harvest) ONLY.**
     - **Cancel `Option E` (`0` lane-days):** Do not run hybrid `RG`-into-`B` experiments.
     - **Park `Option D` (`S0` / `K5` / `K7` look & erosion work) in `docs/parking.md` too:** Do **not** roll lanes from RG into `K5` washboard or `B` hump-trench calibration now. Zero river/erosion R&D runs after close-out until the owner explicitly unparks it.
2. **Strict Time & Scope Cap:**
   - Close-out completes in **`<= 2` calendar days** across **`<= 3` small PRs** on `main` (`PR #1` Wave-1 harvest, `PR #2` Parking ADR + `status.md` + `parking.md`, `PR #3` `E6` non-functional housekeeping).
   - Required owner effort: **`~5 minutes`** (sending the directive in §4; **no flight required** to park).

---

## 2. Step-by-Step Parking Checklist

### Phase 1 — Immediate Lane Freeze & Git Preservation (`T + 0 to 2 h`, Lead + Lanes)

| Step | Owner / Actor | Exact Action | Verification |
|---|---|---|---|
| **1.1** | **Owner → Lead** | Send the park directive (§4) closing `P0 = C+ (full stop on D and E)`. | Lead updates lane order files immediately; zero new RG switch rows start. |
| **1.2** | **All 3 Lanes** (`crusader`, `explorer`, `spikes`) | Stop current RG runs (`explorer #203jb`, `spikes #184/#190`, `crusader` Wave-2/L `G3-6`). Commit any clean local work-in-progress exclusively to each lane's `spike/*` branch and push to `origin`. Never touch `main`. | `git status` clean in all `.claude/worktrees/*`. |
| **1.3** | **Lead / Crusader** | Create immutable annotated tags on `origin` preserving key milestones before any branch pruning: <br>• `archive/rg-g2-record-2026-11-22` → `faa923f0` (`g1f15` G2 candidate record)<br>• `archive/rg-g3-parked-2026-11-26` → `6d53e75d8` (`origin/spike/rg-prototype` tip with `G3-6` Wave-1 `9.26 GB` pass)<br>• Optional lane tips if ahead (`archive/rg-r23-2026-11-26`, `archive/rg-step1-2026-11-26`). | `git ls-remote --tags origin "refs/tags/archive/rg-*"` lists all tags. |

---

### Phase 2 — Harvest Only Candidate-Neutral Wave-1 Speed & Memory Wins (`PR #1`, `<= 1.5` Lane-Days, `crusader`)

> **Rule:** Only code that optimizes `main`'s existing production regional bake (`base1`, `K5`, `K6` in `crates/generators/src/bake/`) with **100% bit-identical outputs (`CON-14`)** may enter `PR #1`. Zero `rg_chain`, Wave-2 portal, or L-graph code enters `main`.

| Item from `origin/spike/rg-prototype` | Source Commits | Value to `main` (Non-RG) | Acceptance Gate |
|---|---|---|---|
| **Parallel `base1` depression decide** | `5e66aa104` | `~4x` faster regional `base1` bake on multi-core builds. | Tier-A test proving bit-identical `base1` output across `1`, `2`, and `N` threads (`CON-14`). |
| **Lean `K6` (`h1_from_owned` / `h1_body`) & capped `K5` buffer lifetimes** | `6c145ab95`, `ef569a5f7` | Cuts full-planet (`k=4`, `1.489e8` land cells) Wave-1 peak RSS from `72–89 GB` to **`9.26 GB`** (`9.78 GB` commit). | Existing `crates/generators` + `crates/cache` Tier-A/B hashes unchanged; `cargo xtask prepush` + Windows/Linux CI green. |
| **Time-box fallback** | — | Prevents scope creep. | Any piece not green and bit-identical within `1.5` lane-days is dropped from `PR #1` and left on tag `archive/rg-g3-parked-2026-11-26`. |

---

### Phase 3 — Record the Decision, Disposition Open Items & Lock the Unpark Charter (`PR #2`, Docs-Only, `<= 0.5` Day, `lead`)

1. **Create `docs/adr/0129-river-graph-prototype-parked.md`** *(exact ADR number reserved by lead per `ADR 0120`; `<= 1` page per `ADR 0055`)* containing four sections:
   - **Context:** `P3` (`E6`) tested multi-scale River Graphs (`spike/rg-prototype`, `537` commits). The sparse graph/basin authority and `10x–90x` crest preservation vs. `B` were proven, while `G3` surfaced three architectural blockers: (1) `88–93%` of `G3-9` look defects originate upstream in `S0` (`base1`/`K5`/`K7`), (2) `CON-29` landmass-wide below-erosion edits (`ADR 0112`/`0113`) exceed editor `BUDG-003`/`005` budgets (`9.3–14 GB` RAM, `35 min – 8 h` on `6` threads), and (3) grading a graph renderer with a dry-ground `101.8 m` D8 raster drainage oracle forced a 10-pass, `6,425`-line L repair chain.
   - **Component Ledger (`plan.md:162`):**
     - *Banked on `main` (`#1632`–`#1669` + `PR #1`):* Connected-sea gate (`#1644`, `#1648`, `#1651`), `TEST-155` metric-calibration gate (`#1665`, `BUILD-069`), `GlobalCell` / `SpillGraph::connect_seams` / `CubePatch` / `cell_area` (`#1654`, `#1655`, `#1658`, `#1661`), `REND-068` bilinear mesh normals (`#1662`), `VIEW-075`/`076` + `look-sheet` (`#1629`, `#1633`, `#1650`), Wave-1 memory/parallelism (`PR #1`).
     - *Preserved as reference only (`archive/rg-g3-parked-2026-11-26`):* `G0` graph/basin contracts, `GEN-191` basin merge, `TEST-151` cross-seam proofs, `#184w` `S0` attribution.
     - *Retired (not to be ported):* `rg_chain.rs` (`6,425` LOC, `96` switches, `48` readings), `KS0`/`KS1`/`8`-round `band6`/`KC` global L drains, dry-ground `bar_agree_1km2`, and uncalibrated look proxies (`Xdiag`, `M4`, `D8 sinuosity`, `straight-run`).
   - **Disposition of Proposed ADRs & Open Decisions:**
     - Proposed RG ADRs `0123`, `0125`, `0126`, `0127`: marked **`parked`** in `docs/adr/README.md` (closes `E4`, `E5`, `R19`, `R20`).
     - `B` (`origin/main`) remains the production pipeline as-is until a future milestone explicitly schedules terrain/hydrology.
   - **Mandatory 5-Point Unpark Charter (Binding Preconditions Before Unparking Hydrology):**
     1. **Editor (`AP1`):** Amend `ADR 0112`/`0113` so interactive editor sculpting updates `S0` + a bounded local river corridor (`< 5 s`, `<= 1 GB` RAM on `6` `BUDG-003` threads, quantized `0.25 m`/`0.5 m` stopping rule, progressive tile swaps); regional `Wave 1` erosion runs only as an explicit world/region bake.
     2. **Upstream `S0` + `WorldDefinition` First (`AP2` + `ADR 0108`):** Rework `S0` (`base1` breach width + channel-guarded `K5` hillslope/maturity + `K7` `C2` read) parameterized by `relief_scale`, `precip/runoff`, `erosion_maturity`, and `Option<SeaParams>` (`GEN-184`)—signed off visually *with rivers turned off* before writing any river stage.
     3. **Discharge Authority, No Dry-Ground Raster D8 Oracle:** Gate rivers/lakes strictly on physical discharge `Q_w >= Q_vis` (`0 ms` automatic no-op on dry worlds/moons, `R5b`); drop raster D8 drainage gates (`G-DRAIN` / `bar_agree_1km2`) on un-rendered hillslopes.
     4. **3-Stage Local Architecture (`Extend ADR 0119`):** Stage 1 sparse topological + declarative monotone-bed DAG solve → Stage 2 pure `CubePatch` (`halo = 128`) continuous segment-distance `C1` smooth-min valley carve (`~2 km` corridor A\* around `L` spurs) → Stage 3 optional local `CubePatch` pit polish (`<= 1,000` LOC, `<= 5` switches).
     5. **Single-Lane Governance (`AP4`):** Max `1` R&D lane during look/algorithm design; Gate-0 `BUDG-003`/`005` & `TEST-155` checks; auto-idle at `>= 2` open owner decisions; mandatory Tier-D owner flight at every look gate.

2. **Clean `docs/status.md` & `docs/parking.md` in the Same `PR #2`:**

| File & Section | Exact Change |
|---|---|
| `docs/status.md` → **Waiting for the owner** | **Delete** all 4 RG items: `RG's direction (P0)`, `G3 gate, open decisions`, `G3: the owner's flight`, and `G3: the breach bound (D1-3)`. |
| `docs/status.md` → **Queue (`P3`)** | **Delete** finished/parked rows `G3-A`, `G3-B`, `G3-C`, `G3-D`, `E4`, `E5`; leave only `E6` housekeeping until `PR #3` merges, then close `P3`. |
| `docs/parking.md` → **New Section: `Hydrology & S0 Base (parked 2026-11-26, ADR 0129)`** | Consolidate `RG` (`archive/rg-g3-parked-2026-11-26`), `O5` (`K5` washboard), `O12` (`K7` `C2` read), `O13`/`O14` (calibrated look bars), `ADR 0108` (`GEN-184` world parameter lift for `S0`/hydrology), and `ADR 0112`/`0113` bounded-edit revision under the 5-point Unpark Charter. |
| `docs/log.d/2026-11-26-rg-parked.md` | One-line milestone log entry citing `ADR 0129` and tag `archive/rg-g3-parked-2026-11-26`. |

---

### Phase 4 — Digest Freeze, Disk Reclamation & `E6` Housekeeping (`PR #3`, `<= 1` Day)

1. **Freeze `C:\DEV\SeriousPlanetData\lanes\digest\`:**
   - Move `decisions/O*.md` into `decisions/archive-g3-parked/` and replace `decisions/00-open-decisions-2026-11-26.md` with a 5-line `CLOSED: parked per ADR 0129` pointer.
   - Add `river-graph/00-PARKED-README.md` indexing the 6 canonical artifacts for future-you: `g0-contracts.md`, `g2-report-2026-11-22.md`, `g3-snapshot-2026-11-26.md`, `attribution-184w-2026-11-25.md`, `fable-rg-feasibility-2026-11-26.md`, and `ag_serious-planet-builder/review/carbon-rg-merged-feasability-2026-11-26.md`.
2. **Reclaim Disk Space Across Worktrees & Data Root:**
   - **Delete (`50–150+ GB` reclaimed):** All intermediate `tmp/g1f*`, `tmp/s184*`, `tmp/g36*`, raw `.f64` surface dumps, and obsolete `g1f14`/`g1f15` stores across `.claude/worktrees/*/target/` and `C:\DEV\SeriousPlanetData`.
   - **Keep (`< 500 MB`):** All `.md` notes, `.txt`/`.csv` gate tables, and rendered `.png` comparison/attribution sheets.
   - **Optional (`~6–8 GB`):** Keep *only* the 6 `fly1126` stores (`b8-r1h`, `rg-def-r1h`, `rg-s1-r1h`, `b8-f1`, `rg-def-f1`, `rg-s1-f1`) for 30 days in case you ever want a casual look—or delete them immediately if you want zero river artifacts on disk.
3. **Run `E6` Housekeeping (`PR #3`, `crusader` or `explorer`):**
   - Strip any stale comments referencing `rg_chain` readings/switches in `main` crates (`river_gate*`, `bake/*`), archive completed M8/M9/P3 OpenSpec changes (`docs/status.md` `E6`), and close milestone `P3`.
4. **Stand Down the 3-Lane Swarm:**
   - Set `crusader`, `explorer`, and `spikes` to **idle** (no background orders) once `PR #1`–`#3` merge, until the owner chooses the next **non-river** topic (e.g., instancing & ground detail, materials/atmosphere, editor UX, or a full project pause).

---

## 3. Decision Dispositions at a Glance (Zero Loose Ends)

| Item | Disposition Under This Plan |
|---|---|
| **`P0` (RG direction)** | **Decided: `Option C+`** (Park RG cleanly; harvest Wave-1 `G3-6` memory/speed only; do **not** start `D` or `E`). |
| **`O8`, `O10`, `O15`, `O16`, `Step 1 sign-off`, `D1-3` ADR** | **Closed as moot** (specific to `rg_chain` G3 port). |
| **`O5` (`K5` washboard), `O12` (`K7` `C2` read), `O13`, `O14`** | **Moved to `docs/parking.md`** under Unpark Precondition 2 (not worked on now). |
| **`fly1126` 30-min owner flight** | **Optional / Unblocking:** Not required to close `P3`; stores either kept 30 days on disk or deleted at owner's choice. |
| **Proposed ADRs `0123`, `0125`, `0126`, `0127`** | **Marked `parked`** in `docs/adr/README.md` by `ADR 0129`. |
| **External PR Reviewer (`proposals/external-pr-review-2026-11-24.md`)** | **Independent of rivers** — kept as the sole optional process item whenever you want it. |

---

## 4. Ready-to-Send Directive for the Lead (`opus-architect`)

Copy-paste this directly to the lead session to execute the entire park without further back-and-forth:

```markdown
Decision on P0: Execute Option C+ (clean park) now, with a full stop on all river, hydrology, and S0/K5/K7 erosion work afterwards (do NOT start Option E or Option D). Follow `c:/dev/ag_serious-planet-builder/review/rg-parking-plan-2026-11-26.md`:

1. Freeze all RG lane runs immediately; commit/push any clean WIP to `spike/*` branches only and tag `origin/spike/rg-prototype` (`archive/rg-g3-parked-2026-11-26`) and `faa923f0` (`archive/rg-g2-record-2026-11-22`).
2. PR #1 (crusader, <= 1.5 days, bit-identical CON-14 only): harvest ONLY the candidate-neutral Wave-1 (`base1` parallel decide + lean `K6`/`K5` memory, `9.26 GB` RSS) improvements into `main`—zero `rg_chain`, Wave-2, or L-graph code. Drop anything not bit-identical within the time-box.
3. PR #2 (lead, docs-only): write the Parking ADR (`ADR 0129` or next reserved number) with the component ledger, mark proposed ADRs 0123/0125/0126/0127 as `parked`, adopt the 5-point Unpark Charter (bounded <5 s editor corridor edits; S0 + ADR 0108 world-parameter rework with rivers off first; discharge-gated water with no dry-ground D8 raster oracle; 3-stage ADR 0119 CubePatch architecture; single-lane governance), clear RG items from `docs/status.md`, and move O5/O12/O13/O14 + S0/RG to `docs/parking.md`.
4. PR #3: finish `E6` OpenSpec/comment housekeeping, clean `tmp/*.f64` dumps across worktrees (keep `.md`/`.txt`/`.png` and keep `fly1126` stores untouched for now), freeze the `river-graph` digest with a short index README, and idle all three lanes when merged.
```
