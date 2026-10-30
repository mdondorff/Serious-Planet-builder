## 1. PR order (each green; nothing in the product changes)
| PR | Scope | Requirements | Checks (tier) |
|---|---|---|---|
| 1 | `generators::descriptors`: slope, curvatures, relief, TPI, geomorphons, fill, D8, density; synthetic tests | GEN-089 to GEN-092 | A: the synthetic table of design.md; 1 and 8 threads byte-identical |
| 2 | `testkit::glo30` reader on `miniz_oxide`; synthetic TIFF test; ignored real-tile check | TEST-078 | A: synthetic round trip; B (ignored, `PLANET_GLO30_DIR`): N47E011 p50 slope within 0.5 degrees of SPIKE-55's 19.3 |
| 3 | `examples/terrain_score.rs`, the sites file, distances, report; `cargo xtask terrain-score` | TEST-079, TEST-080, BUILD-029 | A: distances and report on synthetic results; xtask argument tests. Manual: one run on the store and the tiles, wall time per phase in `report.json`, at most 60 s |

## 2. Steps
- [ ] 2.1 PR 1: descriptors and their tier-A tests; `numerics-reviewer` (generators)
- [ ] 2.2 PR 2: GLO-30 reader; licence notice text as a constant beside it
- [ ] 2.3 PR 3: the run, the report and the command; the sites file from SPIKE-55's table
- [ ] 2.4 Check the run reproduces SPIKE-55's slope and relief p50 and p99 per tile and site within 2 % (blocks and slope are the same definitions; the real side now uses the central 0.5 degrees, so the real rows are re-stated, not compared)
- [ ] 2.5 Wall time at most 60 s on Rasierklinge (a correctness run, no window: ADR 0087 point 3 lists no such check); if over, design.md risk 1
- [ ] 2.6 `verifier` before each PR is declared done; `spec-guardian` before archiving

## 3. Sites file (from SPIKE-55; `crates/app/data/terrain-score-sites.toml`)
| Tile | Landform | Generator sites (lat, lon) |
|---|---|---|
| N52E012 | Brandenburg plain | (-38.5, 30.5), (41.5, 149.5) |
| N41W096 | Loess hills and plains | (-15.5, 150.5), (-10.5, 155.5) |
| N43W102 | Badlands | (13.5, 22.5), (-33.5, 4.5) |
| N31W004 | Erg Chebbi desert | (-23.5, -18.5), (9.5, 18.5) |
| N40W078 | Valley and Ridge | (16.5, -111.5), (-40.5, 1.5) |
| N48E008 | Black Forest | (2.5, -124.5), (17.5, -26.5) |
| N57W005 | Scottish Highlands | (10.5, -122.5), (-26.5, -118.5) |
| N47E011 | Alps, Tyrol | (11.5, -18.5), (-38.5, 3.5) |
| N27E086 | Himalaya, Everest | (-38.5, 1.5), (7.5, -19.5) |

## 4. After the change
- [ ] 4.1 Archive (sync GEN-089 to GEN-092, TEST-078 to TEST-080, BUILD-029); delete design.md and tasks.md (ADR 0055); log fragment
- [ ] 4.2 Baseline: the first report on main, committed as `docs/milestones/M9/terrain-score-baseline.md` with the Copernicus notice (derived statistics only; no tiles, no maps); the owner gets it through the planner's SendUserFile
- [ ] 4.3 Glossary (architect): `terrain scorecard`, `geomorphon`, `TPI` as accepted terms if M9 uses them in specs
