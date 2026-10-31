# Spike register

Time-boxed experiments that answer a question before a decision. Opened and closed with `/spike`. Spike code lives on its spike branch and is never merged into the main crates as-is. **Each spike's report, numbers and renders live in `spikes/<name>/results/`** (`results.md` with a Visuals section; the `spike` skill has the rule). `docs/spikes/` holds only the frozen images of SPIKE-03 to 07.

Planned spikes from the background report still to open: city HLOD with 10^5 buildings (with Cities). Proposed after M8: game-layer exploration, a vehicle on chunk collision (`docs/roadmap.md`).

Per-spike register sections of M5 and M6: `docs/archive/M6/spikes.md`. Closed spikes up to M7 (SPIKE-08 to 32 and the explorations): `docs/archive/M7/spikes.md`. Closed spikes of M8 (SPIKE-33 to 57, 59 and 60): `docs/archive/M8/spikes.md`. Closed spikes of earlier milestones: `docs/archive/M0-M3/spikes.md` (SPIKE-01 to 04), `docs/archive/M4/spikes.md` (SPIKE-05 to 07). SPIKE-10 was never used.

## Index of open spikes
| Id | Title | Status | Results |
|---|---|---|---|
| SPIKE-72 | The Highlands seam | closed 2026-10-31 (a 1,100 m basin across a core edge, filled by the local carve to window-dependent spill points, lakes 195 m apart; fix: the local carve only incises (no fill, no breach), seam-free at all four sites; flats fall to 1-2.5 %, for the deposition sweep and R7) | `spikes/m9-highlands-seam/results/` |
| SPIKE-71 | Bake time margin | closed 2026-10-31 (active-cell thermal exact and 1.4-13x faster; point samples at 814 m replace the block means at 1/47 of the cost, relief within 1-3 %; the 1 m land floor does not fix the Highlands seam; planet about 6-7.5 h, central 6.9 h) | `spikes/m9-bake-time/results/` |
| SPIKE-70 | Global routing for the carve | closed 2026-10-31 (the carve per face at 800 m keeps the joint's relief with no core-edge seam; a per-tile carve capped at 1 km^2 after it restores the steep slopes, seam-free at 3 of 4 sites, the 4th needs a 1 m land floor; forced rivers keep seams; thermal needs active cells; bake 7.5-10 h; SPIKE-61's inflow fix +8 %) | `spikes/m9-global-routing/results/` |
| SPIKE-69 | The relief-driver map: where k comes from | closed 2026-10-31 (the landform driver L through a land-quantile curve, k 1 at L 0, 4.5 by L 0.028, 5.6 at L 1, with the coast taper: Earth's shares within 3 points (63/12/25 %), sites 3.9-4.6 against 3.6-6.9; other sources need k 8-17 at the sites; pre-bake in seconds, paintable as an L override) | `spikes/m9-relief-driver/results/` |
| SPIKE-68 | Erosion-layer seams | closed 2026-10-31 (tiles with a 256-cell halo: edge step p99 134-172 m, max 400-480 m; the error follows river networks deep into the cores; inflow and an oracle edge ring do not fix it, held neighbours are worse, a 64-cell blend hides the step only; the carve's routing must be global) | `spikes/m9-layer-seams/results/` |
| SPIKE-67 | G3 block grid at the feet | closed 2026-10-31 (cause: value noise, its slope is 0 on cell edges, so low sun draws the lattice; gradient noise on the same lattice and hash removes it at no measurable cost, all octaves +0.14 ms; R6 builds it) | `spikes/m9-g3-grid/results/` |
| SPIKE-66 | Relief factor in the generator | closed 2026-10-31 (all-land factor x4-6 for hills, about x7.5 for the Alps' 0.8; the landform driver is the wrong weight; a 10 km stencil breaks batch reads, bounds and plan-check: bake the factor and mean; taper at the coast; breach cuts flats to 10-12 %) | `spikes/m9-relief-factor/results/` |
| SPIKE-65 | Where M9's relief comes from | closed 2026-10-31 (amplitude x4-6 is the lever, 0.68-1.26 of real at hill sites; steady-state stream power erases landforms; amp6 + transient carve + thermal 0.62-1.13 at hills, Alps 0.53-0.78; landform drivers own the amplitude, R4 the carve and thermal) | `spikes/m9-relief-source/results/` |
| SPIKE-64 | R3b: CPU thermal and deposition kernels on a region | closed 2026-10-31 (no effect on today's slopes, far under the critical angle; on x4 relief thermal 33 cuts p99 48 to 36 deg; deposition fills basins, 15-33 %; deposition about 0.3-3 s a tile, thermal 16-41 s, needs an active-cell kernel) | `spikes/m9-thermal-deposition/results/` |
| SPIKE-63 | B8: ground detail at 1.8 m (R6 as a spike) | closed 2026-10-31 (G1 shows at 3-80 m in low sun, laws a and b alike under G3, no GPU cost; G3's 3 cm octave a block grid at the feet; G3 full 0.81-1.47 ms) | `spikes/m9-ground-detail/results/` |
| SPIKE-62 | R4 routing cost and flow direction | closed 2026-10-30 (re-route every 5th step: 5.5x less, p95 0.23 m; Barnes flood 1.3x; jittered D8 ends straight runs; planet bake about 4.5-6 h, estimate) | `spikes/m9-route-cadence/results/` |
| SPIKE-61 | M9 R3: bake pre-run at 100 m | closed 2026-10-30 (10 steps: 9.2-13.6 h planet, routing 60 %; delta 7-10 GB; fits with re-routing every 5th step and replaced channels) | `spikes/m9-bake-prerun/results/` |
| SPIKE-58 | Perf windows that block less (B9) | closed 2026-10-30 (loaded readings never faster; loaded pass at 5 % margin; pinned CPU +27 %, GPU +14 %; ADR 0087) | `spikes/m9-perf-windows/results/` |

Every exploration gets a `SPIKE-NN` entry.
