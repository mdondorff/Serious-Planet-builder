# Spike register

Time-boxed experiments that answer a question before a decision. Opened and closed with `/spike`. Spike code lives on its spike branch and is never merged into the main crates as-is. **Each spike's report, numbers and renders live in `spikes/<name>/results/`** (`results.md` with a Visuals section; the `spike` skill has the rule). `docs/spikes/` holds only the frozen images of SPIKE-03 to 07.

Proposed after M8: game-layer exploration, a vehicle on chunk collision (`docs/roadmap.md`).

Per-spike register sections of M5 and M6: `docs/archive/M6/spikes.md`. Closed spikes up to M7 (SPIKE-08 to 32 and the explorations): `docs/archive/M7/spikes.md`. Closed spikes of M8 (SPIKE-33 to 57, 59 and 60) and of the M9 groundwork and Option B after the M8 acceptance (SPIKE-58, 61 to 84): `docs/archive/M8/spikes.md`. Closed spikes of the M9 groundwork from SPIKE-85 to 106 (SPIKE-96 never run, SPIKE-103 has no row): `docs/archive/M9/spikes.md`. Closed spikes of earlier milestones: `docs/archive/M0-M3/spikes.md` (SPIKE-01 to 04), `docs/archive/M4/spikes.md` (SPIKE-05 to 07). SPIKE-10 was never used.

## Index of open and planned spikes
| Id | Title | Status | Results |
|---|---|---|---|
| SPIKE-96 | Cheap edit-aware sky visibility factor for the sky ambient | planned only, not run (waits for the owner's ADR 0095 look decision and a perf slot; the horizon bake stays until it decides) | plan: `spikes/sky-visibility/results/plan.md`; no results folder |
| SPIKE-107 | MFD comparison: unclipped vs clipped vs boundary flux (A3 §12.4, ADR 0112 condition 1) | runs done 2026-11-11 (R1, R3a, R3b; the breach rule, not the area clip, makes most of C's difference in R3a and R3b); the architect judges; re-run after the window-cut fix (GEN-172): `results-rerun.md` | `spikes/mfd-comparison/results/results.md` |
| SPIKE-108 | 0/45/90-degree shapes check: channel orientation and straight runs against GLO-30, the flats, the mechanism | measured 2026-11-11 (diagonal chords 1.2 to 4.7x and straight runs 2.3 to 10x the DEM's; the straight runs trace to the start surface's chessboard-gradient flats, then carved); the architect judges; addendum: breach chains, jitter forms; `results-pits.md`: the lifted cells are basins, not noise pits | `spikes/shapes-check/results/results.md` |
| SPIKE-109 | Start-surface basins: where z0's closed basins come from (component sweep against the frozen DEM pool, breach depth per basin, the 5.001 m baseline) | measured 2026-11-12 (interior basins over 30 cells hold 32.3 / 22.0 / 22.5 % of the land in R1h / R3a / R3b against 3.7 / 0.6 % in the DEM pools; the class band switched off leaves 12.2 / 11.8 / 10.9 %; the 8 km hill octave, the mountain amplitude, the plateau bowls and the coast plain are named); the architect judges | `spikes/start-surface-basins/results/results.md` |
| SPIKE-109 follow-up | coast-plain tilt probe: a sea-ward tilt on the smooth land depth added to z0 (R3a, R3b, R1h) | measured 2026-11-12: R3b drains from 22.5 to 4.9 / 1.5 % at 2 / 4 m/km, R3a from 22.0 to 6.8 / 3.5 % at 4 / 8 m/km, at +0.8 to +3 km of added height at the medians; deep basins stay; the architect judges | `spikes/start-surface-basins/results/tilt.md` |

Every exploration gets a `SPIKE-NN` entry.
