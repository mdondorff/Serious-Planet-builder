## 0. Preconditions
- [ ] 0.1 B6 PR 5 (`rm-chunks-streaming`) merged: B6 PRs 3 to 5 touch `frame` and `streaming`, where the horizon jobs live (opus-planner, 2026-10-30). PR 1 below may start earlier only if it touches `frame::clipmap` alone.
- [ ] 0.2 Perf checks: ADR 0087 accepted, or a planner-authorized window (ADR 0020) for each check marked "window" below.

## 1. PR order (each PR leaves main green; nothing draws the new shadow before PR 5)
| PR | Change id | Adds or removes | Requirements | Checks (tier) |
|---|---|---|---|---|
| 1 | `shadow-clipmap-plan` | `frame::clipmap`: levels, anchor, snap, toroidal scroll, per-level tile lists from R, f64 per-tile mapping and drop coefficients, dirty regions incl. preview footprints | REND-046 | A: mapping error < 1/8 texel over face-corner tiles; drop fit against the f64 sphere; scroll dirties only the strips; the same R gives the same plan; plan time under 0.1 ms for R of 21k tiles |
| 2 | `shadow-clipmap-gather` | `render::shadow` gather and max mips, coarse max cube, `clipmap-gather` compute pass (not yet in the lit graph) | REND-047, REND-048 | C (software adapter, 256^2 levels): readback equals the tile heights minus the drop at sample texels; incremental update equals a full rebuild byte for byte; a preview stamp's delta appears exactly at flagged tiles (REND-033, REND-045); mips are the 3x3-dilated max; coarse cube is the max of the floor tiles |
| 3 | `shadow-march` | `shadow-march` compute pass at half resolution: DDA with max-mip skipping, coarse continuation, penumbra, analytic planet shadow | REND-049, REND-053 | A: spec-lint's shader check finds no planet-radius value; C: a synthetic wall shadows the receivers its geometry predicts (to one texel), a sun below the geometric horizon gives 0 without marching, two runs give identical bytes, no atomics in the WGSL (text check) |
| 4 | `shadow-resolve` | `shadow-resolve`: depth-aware upsample, temporal accumulation with rejection over dirty regions, the two-frame spread below 5 degrees | REND-050, REND-052 | C: a dragged stamp over 3 frames shows no history of the old position; a camera cut resets history; the spread fills every pixel within 2 frames. Perf (layer 1 total, river-r1 and hills-b, 300 m and 30 m, midday, sunset and 2 degrees): **window** for the 2-degree pose (a close call in SPIKE-56), loaded pass for the rest under ADR 0087 once accepted. A low-sun miss decides SPIKE-56's fallback here |
| 5 | `shadow-integration` | `sun-direct` target in `terrain`; `sky-composite` applies `shadow x sun-direct`; water reads `shadow`; the horizon factor leaves the fragment stage; J's shadow view shows `shadow` | REND-051; BUDG-002, REND-044 reworded | C: every lit golden moves once, per adapter (ADR 0061), through `golden-review`; first lit goldens of the new shadow escalate to the owner's `/bless` (ADR 0017); debug-view goldens unchanged. Gate (`cargo xtask gate`): no frame over 100 ms added. Owner package (TEST-074) for the look review |
| 6 | `rm-horizon-render` | `render::{horizon, horizon_wgsl}`, `frame::horizonview`, the horizon textures and their bind group, J's horizon view, `ShadeView::Horizon` | REND-038, REND-041, REND-043 removed; VIEW-054 reworded | A, C: suites pass with the horizon tests removed together with their requirements; no golden moves (PR 5 moved them). **Starts only after the owner's look review of PR 5** |
| 7 | `rm-horizon-jobs` | `streaming::horizonjobs` (`HorizonJobs`, `HorizonBaker`, `HorizonDisk`, `ChunkSource`), `app::horizonworld`, `cache::horizon_store`, the horizon parts of `stills`, `editperf`, `gate`, `envelopeperf`, `litperf`, `--horizon-threads`, `--no-horizon-jobs` | STRM-048, STRM-049, STRM-050, STRM-057, STRM-059, STRM-061, STRM-064, TEST-065, TEST-066 removed; STRM-060, TEST-054 reworded | A, B: suites pass; the edit-latency and envelope scenes run without horizon rows |
| 8 | `rm-horizon-generators` | `generators::{horizon, horizon_lookup, coarsehorizon}`, the coarse horizon's make and read paths, `examples/coarse_probe.rs` | GEN-044, GEN-045, GEN-046, GEN-085, GEN-086, STRM-045 removed | A: `C:/DEV/SeriousPlanetData/global16384` still opens without a rebuild with its stored coarse horizon channel untouched on disk (a tier-A check on the definition hash and the store's channel list; opus-planner condition 1); GEN-046's helpers have no caller outside the horizon maps (checked at main 10db3b63: only `coarse_probe`) |
| 9 | `shadow-clipmap-march` archive | perf acceptance row, spec sync, archive, glossary rows | none new | **window**: layer 1 at most 1.5 ms at every pose (REND-052), the acceptance number of record; `spec-lint`; log fragment |

## 2. Steps per PR
- [x] 2.1 PR 1 `shadow-clipmap-plan`
- [x] 2.2 PR 2 `shadow-clipmap-gather` (2a gather and mips; 2b the coarse cube, REND-048)
- [x] 2.3 PR 3 `shadow-march`
- [x] 2.4 PR 4 `shadow-resolve` (perf window for the 2-degree pose: asked of opus-planner, not yet measured)
- [ ] 2.5 PR 5 `shadow-integration` (goldens per adapter; owner package)
- [ ] 2.6 Owner: look review of PR 5 (stills old against new, a drag video) and `/bless` of the first lit goldens
- [ ] 2.7 PR 6 `rm-horizon-render`
- [ ] 2.8 PR 7 `rm-horizon-jobs`
- [ ] 2.9 PR 8 `rm-horizon-generators`
- [ ] 2.10 PR 9 archive: perf window, sync, archive, log fragment
- [ ] 2.11 Reviews: `numerics-reviewer` on PRs 1 to 4 (frame, shaders); `spec-guardian` before PR 9; `verifier` before each PR is declared done

## 3. Owner items
- [ ] 3.1 Look review (after PR 5): river-r1, hills-b and the Plateau 6 drag at 300 m, 30 m and 1.8 m; midday, sunset and a 2-degree sun; old against new; a 5-10 s drag video of a plateau stamp (`cargo xtask package`)
- [ ] 3.2 `/bless` of the first lit goldens of the new shadow, per adapter

## 4. Glossary (architect, with PR 9)
- [ ] 4.1 `height clipmap level`: accepted to built
- [ ] 4.2 `shadow march`: accepted, not built, to built
- [ ] 4.3 `horizon map`: retiring to retired (section 8, non-normative); identifiers, flags and comments that use it are gone after PRs 6 to 8
- [ ] 4.4 `chunk`: drop "keys horizon maps" (design.md open point 3)
