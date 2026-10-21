# Spike register

Time-boxed experiments that answer a question before a decision. Opened and closed with `/spike`. Spike code lives on its spike branch and is never merged into the main crates as-is. **Each spike's report, numbers and renders live in `spikes/<name>/results/`** (`results.md` with a Visuals section; the `spike` skill has the rule). `docs/spikes/` holds only the frozen images of SPIKE-03 to 07.

Planned spikes from the background report still to open: city HLOD with 10^5 buildings (M7).

Per-spike register sections of M5 and M6: `docs/archive/M6/spikes.md`. Closed spikes of earlier milestones: `docs/archive/M0-M3/spikes.md` (SPIKE-01 to 04), `docs/archive/M4/spikes.md` (SPIKE-05 to 07). SPIKE-10 was never used.

## Index of M5 spikes and explorations
| Id | Title | Status | Results |
|---|---|---|---|
| SPIKE-08 | Ocean connectivity at 16384 | closed 2026-10-16 | `spikes/ocean-connectivity/results/` |
| SPIKE-09 | Atmosphere f32 forms | closed | `spikes/atmosphere-f32/results/` |
| SPIKE-21 | The wet floor of ADR 0071: cost and memory of the three passes | closed 2026-10-20 (recommendation: implement option C) | `spikes/wet-floor/results/` |
| SPIKE-17 | Two bugs of the owner's descent video: the atmosphere top, the sea edge (diagnosis) | closed 2026-10-20 (causes: far terrain hazed differently inside and outside the top, fixed in #404; the 100 km reach of the coastal band, see SPIKE-20 results) | `spikes/descent-video-bugs/results/`, `spikes/owner-captures-2026-10-20/results/` |
| SPIKE-18 | The drawn tiles go coarser for a moment during a slow approach (diagnosis) | closed 2026-10-20 (cause: the cover splits a tile only if every in-view child is resident) | `spikes/tile-coarsening/results/` |
| SPIKE-19 | The window aborts on a refused morph interval (diagnosis) | closed 2026-10-20 (cause: the coarse LOD factor sits at the validate limit, the plan adds the height extent) | `spikes/morph-refusal/results/` |
| SPIKE-20 | The atmosphere still banded after the AP range fix (diagnosis) | closed 2026-10-20 (cause: the 32 x 32 x 32 volume is 21 to 23 % off between texels at 60 to 97.5 km; fixed in `atmosphere-camera-march`) | `spikes/owner-captures-2026-10-20/results/` |
| SPIKE-22 | Is the base LOD refused in the real eroded world? (diagnosis) | closed 2026-10-20 (no: 0 refusals in 45,800 poses; the finding holds for fields with only a global bound) | `spikes/base-lod-refusal/results/` |
| SPIKE-27 | Ground detail at the lowest levels, with the near-plane check (M7 G1, G0; prototype) | closed 2026-10-21 (the smear is the shading; a procedural per-material term costs +0.45 to +0.85 ms at 1440p and works at 1.8 m and 0.5 m; near 5 cm shows no z-fighting; recommendation: procedural first, CC0 sets from a fetched cache) | `spikes/ground-detail/results/` |
| SPIKE-26 | The item sweep: the owner's 77 items visited in the real editor window at 2 km, 200 m and 30 m (automated P1 retest) | closed 2026-10-21 (45 of 231 stops refused a base-LOD plan, 22 items, mostly brushes; Plateau 7 clean; no crash or panic) | `spikes/owner-items-sweep/results/` |
| SPIKE-25 | Tiles flash to a coarser level while the camera moves (P1-4; diagnosis) | closed 2026-10-20 (not reproduced: 0 level drops in 14 scripted flights and the editor window loop; SPIKE-18's fix holds; the video's bursts are overlay brightness, unchecked) | `spikes/lod-flashing-moving/results/` |
| SPIKE-23 | Why the edit-latency case at 200 m with a 150 m stamp takes 508 ms to drawn-exact (diagnosis) | closed 2026-10-20 (cause: tile generation throughput, 40 ms a tile of which the stamp is under 1 %) | `spikes/edit-refresh/results/` |
| SPIKE-24 | Normals from the tile height lattice: errors, seams, pictures (evidence of ADR 0073) | closed 2026-10-20 | `spikes/lattice-normals/results/` |
| SPIKE-16 | Dark patches over the sea while the camera moves (water-motion-gap; diagnosis) | closed 2026-10-19 (cause: the shore ramp depends on the drawn node's level) | `spikes/water-motion-gap/results/` |
| SPIKE-11 | Mouse look plumbing | closed 2026-10-18 (recommendation: raw events, Locked then Confined) | `spikes/mouse-look/results/` |
| SPIKE-15 | Edit-to-test build time | closed 2026-10-18 | `spikes/build-speed/results/` |
| SPIKE-14 | egui in the frame | closed 2026-10-18 | `spikes/egui/results/` |
| SPIKE-13 | Edit-to-visible latency | closed 2026-10-18 | `spikes/edit-latency/results/` |
| SPIKE-12 | Retiring the vertex tile format | closed 2026-10-17 (recommendation: retire, early M6; owner decides at the M5 gate) | `spikes/vertex-retirement/results/` |
| exploration | WARP parallel-device crash: inside WARP, test-only (PRs #195, #196) | closed | `spikes/warp-crash/results/` |
| exploration | WARP device removal with compact tiles and the atmosphere: test-only (PRs #212, #213) | closed | `spikes/warp-compact-atmosphere/results/` |
| exploration | Tile-format duality: cost of retiring the vertex format (PR #218) | closed; SPIKE-12 follows | `spikes/tile-format-duality/results/` |
| exploration | Compact tiles in storage buffers (PR #225; ADR 0054) | closed | `spikes/compact-storage-buffer/results/` |

The explorations before 2026-10-17 had no register entry; from now on every exploration gets a `SPIKE-NN` entry.
