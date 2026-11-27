# Hydrology downstream seams for `G3` and `P4`

Status: non-normative brief for `P3` `G3` and `P4` (owner and `ag2-antigravity`, 2026-11-22). Read with `docs/roadmap.md`, `docs/design/terrain-target.md`, `docs/archive/P3/P3.md` and `docs/milestones/P4.md`.

## 1. Placement in the schedule
- None of the items below blocks `P3` `G2` or `G3` from selecting the winning hydrology variant.
- Consumer interfaces (`M10` suitability curves and instancing, `M11` 3D water geometry and optics, `M12` road/city planning and authored river constraints) remain deferred until after `P4`, because runtime representations are disposable caches (`CON-05`) whereas stored commit records (`kind 13`, `kind 19`, `kind 20`) and terrain-bed invariants are costly to migrate after planet bakes (`G-STORE` 40 GB budget).
- Two landing points during the transition to production:
  1. **`G3` production ADRs:** persisted record fields, `G-STORE` raster discipline, and `soil_m` provenance.
  2. **`P4` move to production (`P4` `E1`):** unified CPU point/patch query under one `commit_id` and mouth `coast_fade` reconciliation.

## 2. Downstream milestone inputs from production hydrology
| Milestone | Required from production hydrology | Risk if dropped during `P3`/`P4` |
|---|---|---|
| `M10` Living surface | Sub-metre wet exclusion along `w = a Q_w^b` and lakes (finer than 32 m driver texels); valley-floor / `HAND` signal; explicit `soil_m` source if `K7` thermal/deposition stays off | Oversized planet wetness rasters in `G-STORE`, or plants spawning inside stream trenches and kept lakes |
| `M11` Water and sky | Upgrade path from burned-bed coverage (`REND-060`) to `water >= bed`; downstream tangent, `q_w`, `rise_over_budget` rapids markers; zero step at mouths (`coast_fade`) and lakes; dry valleys (`q_w = 0`) | Estuaries, waterfalls, and lake outlets tear once water gains non-zero surface depth |
| `M12` Settled world | Landmass-scale graph query without decoding render tiles; stable IDs and edit continuity maps; reserved reach-level meandering slot (`KP`) | Roads, bridges, and meandering require a `kind 20` format break |
| `M13` and Game layer | Bounded LRU coverage slots (`RIVER_SLOTS`); Seam 1 (`docs/roadmap.md`) single CPU query returning ground, normal, material and water (`CON-07`, `CON-14`) | Physics/placement water diverges from rendered water |

## 3. Four producer invariants for `G3` and `P4`
| # | Invariant | Target | Rule |
|---|---|---|---|
| 1 | Vector-first `kind 20` (`STRM-084`) and `kind 19`, disposable runtime masks (`CON-05`) | `G3` ADRs | Persist oriented polylines (`up` to `down`), `q_w`, Strahler order, `bed`, `H1` parent IDs, and node kinds (`lake_inlet`, `lake_spill`, `outlet_sea`, `exception`) plus a lightweight landmass trunk/basin index |
| 2 | No extra planet-wide riparian rasters in `G-STORE` | `G3` ADRs | Reconstruct tile-local height above nearest drainage (`HAND`) and floodplain proximity in `M10` CPU workers from `kind 13`, `kind 19`, and `kind 20` |
| 3 | Explicit `soil_m` (`STRM-072`) disposition | `G3` ADRs | Record whether `M10` soil depth comes from a post-valley sediment pass or an analytic equilibrium proxy (`slope`, curvature, `HAND`, lithology, runoff) when `K7` thermal/deposition stays off |
| 4 | Unified commit query and mouth `coast_fade` consistency | `P4` (`E1`) | One GPU-free CPU query (`generators`/`cache`, `CON-02`) keyed to `commit_id` (`STRM-083`) returning ground, bed, water level and body kind (`water >= ground` where wet), with `coast_fade` reconciled into the bake |

## 4. Kept strictly after `P4`
| Deferred scope | Owner milestone | Reason |
|---|---|---|
| Suitability trapezoid schema and GPU indirect instancing (`ADR 0076`, `SPIKE-28`) | `M10` | Unmeasured LOD popping, triangle budget, and shader packing (`LOOK-008`) belong after `P4` |
| Elevated river ribbons, flow UVs, rapids foam, orbit simplification | `M11` | Depends on production `R1h` look review (`P4` `E1`) first |
| Road/bridge valley routing, ports, meandering (`KP`), authored river constraints | `M12` | Consumes the stable `kind 20` graph and continuity map once `P4` lands |
