# Spike register

Time-boxed experiments that answer a question before a decision. Opened and closed with `/spike`. Spike code lives on its spike branch and is never merged into the main crates as-is. **Each spike's report, numbers and renders live in `spikes/<name>/results/`** (`results.md` with a Visuals section; the `spike` skill has the rule). `docs/spikes/` holds only the frozen images of SPIKE-03 to 07.

Planned spikes from the background report still to open: city HLOD with 10^5 buildings (with Cities). Proposed after M8: game-layer exploration, a vehicle on chunk collision (`docs/roadmap.md`).

Per-spike register sections of M5 and M6: `docs/archive/M6/spikes.md`. Closed spikes up to M7 (SPIKE-08 to 32 and the explorations): `docs/archive/M7/spikes.md`. Closed spikes of M8 (SPIKE-33 to 57, 59 and 60): `docs/archive/M8/spikes.md`. Closed spikes of earlier milestones: `docs/archive/M0-M3/spikes.md` (SPIKE-01 to 04), `docs/archive/M4/spikes.md` (SPIKE-05 to 07). SPIKE-10 was never used.

## Index of open spikes
| Id | Title | Status | Results |
|---|---|---|---|
| SPIKE-64 | R3b: CPU thermal and deposition kernels on a region | closed 2026-10-31 (no effect on today's slopes, far under the critical angle; on x4 relief thermal 33 cuts p99 48 to 36 deg; deposition fills basins, 15-33 %; deposition about 0.3-3 s a tile, thermal 16-41 s, needs an active-cell kernel) | `spikes/m9-thermal-deposition/results/` |
| SPIKE-63 | B8: ground detail at 1.8 m (R6 as a spike) | closed 2026-10-31 (G1 shows at 3-80 m in low sun, laws a and b alike under G3, no GPU cost; G3's 3 cm octave a block grid at the feet; G3 full 0.81-1.47 ms) | `spikes/m9-ground-detail/results/` |
| SPIKE-62 | R4 routing cost and flow direction | closed 2026-10-30 (re-route every 5th step: 5.5x less, p95 0.23 m; Barnes flood 1.3x; jittered D8 ends straight runs; planet bake about 4.5-6 h, estimate) | `spikes/m9-route-cadence/results/` |
| SPIKE-61 | M9 R3: bake pre-run at 100 m | closed 2026-10-30 (10 steps: 9.2-13.6 h planet, routing 60 %; delta 7-10 GB; fits with re-routing every 5th step and replaced channels) | `spikes/m9-bake-prerun/results/` |
| SPIKE-58 | Perf windows that block less (B9) | closed 2026-10-30 (loaded readings never faster; loaded pass at 5 % margin; pinned CPU +27 %, GPU +14 %; ADR 0087) | `spikes/m9-perf-windows/results/` |

Every exploration gets a `SPIKE-NN` entry.
