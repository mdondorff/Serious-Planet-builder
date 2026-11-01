# Spike register

Time-boxed experiments that answer a question before a decision. Opened and closed with `/spike`. Spike code lives on its spike branch and is never merged into the main crates as-is. **Each spike's report, numbers and renders live in `spikes/<name>/results/`** (`results.md` with a Visuals section; the `spike` skill has the rule). `docs/spikes/` holds only the frozen images of SPIKE-03 to 07.

Planned spikes from the background report still to open: city HLOD with 10^5 buildings (with Cities). Proposed after M8: game-layer exploration, a vehicle on chunk collision (`docs/roadmap.md`).

Per-spike register sections of M5 and M6: `docs/archive/M6/spikes.md`. Closed spikes up to M7 (SPIKE-08 to 32 and the explorations): `docs/archive/M7/spikes.md`. Closed spikes of M8 (SPIKE-33 to 57, 59 and 60) and of the M9 groundwork and Option B after the M8 acceptance (SPIKE-58, 61 to 84): `docs/archive/M8/spikes.md`. Closed spikes of earlier milestones: `docs/archive/M0-M3/spikes.md` (SPIKE-01 to 04), `docs/archive/M4/spikes.md` (SPIKE-05 to 07). SPIKE-10 was never used.

## Index of open spikes
| Id | Title | Status | Results |
|---|---|---|---|
| SPIKE-89 | Relative, topology-aware compute shares | closed 2026-11-01 (every thread share is an absolute count of the 20-thread reference machine and the job system detects only the logical count; proposal: shares as clamped fractions of C = P + 0.65 E less 2, detection through the existing `windows` crate and sysfs; Rasierklinge terrain 9, 8-core desktop 8, 4-core laptop 2; 4 PRs) | `spikes/relative-shares/results/` |
| SPIKE-88 | First-read stalls and the K view orbit stall | closed 2026-11-01 (the smoke's 1-2.2 s input frames are the scripted vsync toggle reconfiguring the swapchain on WARP, not first reads; the K stall is main-thread block decodes, 20 ms idle but 70-320 ms while K's refresh of all resident tiles saturates the CPU, with the camera's blocks evicted from the about 30-block shared cache; fix: the camera's ground from the warm-up thread, blocks pinned) | `spikes/first-read-stalls/results/` |
| SPIKE-87 | Editor CPU cost at cube edges (pick hypothesis) | closed 2026-11-01 (refuted, not reproduced: the marker is a cube corner, yet a warm hover pick costs 0.06-0.18 ms there, as at face centres; the editor update averages 0.17 ms in a smoke at the marker; only cold block decodes cost 17-43 ms, at any site, and run off the main thread since #858/#831 (VIEW-059); close the Tracked line) | `spikes/editor-edge-cost/results/` |
| SPIKE-86 | Base LOD over the real store (VIEW-032 item) | closed 2026-11-01 (a harness artefact: 0 refused or capped in 400 real-store and 1,000 generated poses at 20-31 km against 51.8 % capped at level 11 over the global-bound stub (ADR 0074 caps instead of refusing); new: near the ground over steep land 5.5 % of real-store poses cap at levels 16-21 from loose fine-level bounds, 5-37x the sampled range) | `spikes/base-lod-real-store/results/` |
| SPIKE-85 | The slope ceiling of the heightfield | closed 2026-11-01 (GLO-30 at 30 m: slopes above 45 degrees cover 4.3 % of the Alps, 8.7 % of Everest, under 0.2 % of the hill sites, and carry none of the 1 km relief (kept 0.997-1.000); the LOD engine has no slope limit (caps are bound-driven), the bake's thermal 33-41 degrees sets it; proposes a 45 degree ceiling for criterion 1 (Alps real p50 x0.95) and triplanar plus steep-face displacement, then instanced cliff meshes, for the rest) | `spikes/m9-slope-ceiling/results/` |

Every exploration gets a `SPIKE-NN` entry.
