# Perf window 24 (the ground detail, G2a and G2b), 2026-10-22

Rasierklinge, RTX 3080 Ti Laptop (Vulkan), release build of main at 6d20ef62 (#530, #531) plus this change's scenes (the two ground views of `perf lit --views` and `PLANET_GROUND_DETAIL`). Coordinator opus-planner, run by sonnet-explorer. Preflight OK on the first run (`window-24/preflight.md`: single 46.3 to 47.7 ms against 52, parallel median 90.4 to 94.8 ms against 112). Readiness gate ok in every run; "SESSION VALID for budgets" (GPU verdicts against the 1400 MHz clock floor).

## Verdict

- **The ground detail is over its budget: +2.3 to +2.9 ms of terrain-pass GPU time at 1440p against the budget of +1.0 ms for G2a plus G2b.** At 1.8 m +2.35 ms (1.37 to 3.75), at 0.5 m +2.9 ms (1.36 to 4.26). The frame (waited) median goes from 3.2 to 5.8 ms (1.8 m) and 6.2 ms (0.5 m), far under the 20 ms frame budget, so the window is not slow; the term is 2.4 to 2.9 times its allowance.
- **Natural (0.6) and full (1.0) cost the same** (the strength only scales the terms; every octave, the weights, the roughness and the highlight run whenever the strength is above 0). A cheaper natural mode needs a cheaper shader, not a lower strength.
- The warm flight past Ridge 3 costs 408 to 663 ms per plan job against 926 to 2,673 ms for the still camera (cold plans), see below.

## Ground views, GPU ms (median of two rounds; `perf lit --views ground-1.8m,ground-0.5m`, 2560x1440, 2,424 and 2,317 nodes)

| View | Strength | Terrain pass | Delta | GPU span of the lit frame | Frame (waited) median |
|---|---|---|---|---|---|
| ground-1.8m | off | 1.367 | | 1.76 | 3.19 |
| | natural 0.6 | 3.751 | +2.38 | 4.15 | 5.77 |
| | full 1.0 | 3.702 | +2.33 | 4.10 | 5.77 |
| ground-0.5m | off | 1.363 | | 1.74 | 3.19 |
| | natural 0.6 | 4.258 | +2.90 | 4.64 | 6.17 |
| | full 1.0 | 4.185 | +2.82 | 4.56 | 6.16 |

The two rounds agree within 0.15 ms per cell (raw: `window-24/lit-<strength>-r<round>.txt` and `.json`). The atmosphere rows (0.2 ms) and the exposure passes are unchanged. At the ground the whole picture is ground, so these are the worst case: pixels of sky and water cost nothing here.

Where the time goes is not measured (no per-term toggles). The candidates, by the shader's structure: six octaves of 3D value noise with analytic gradients at every pixel (8 hashes and the gradient mixes each; the 32 m, 8 m and 2 m octaves are never faded at the ground), the weights loop (4 materials x 8 inputs), the material mix, and the GGX term (small).

## Item sweep, plan job ms (p50 of the plans the worker delivered, still camera and a drift of 0.5 m a frame; `PLANET_SWEEP_ONLY="Platonmax,Hill 7,Ridge 3"`)

| Item and altitude | Still (1 cold plan) | Drift 0.5 m/frame (warm plans) | Plans in the drift run |
|---|---|---|---|
| Platonmax 2 km | 281 | 51 | 85 |
| Platonmax 200 m | 1,557 | 555 | 15 |
| Platonmax 30 m | 607 | 576 | 9 |
| Ridge 3 2 km | 2,673 | 408 | 68 |
| Ridge 3 200 m | 1,703 | 648 | 79 |
| Ridge 3 30 m | 926 | 663 | 52 |
| Hill 7 2 km (first) | 137 | 89 | 89 |
| Hill 7 200 m | 1,170 | 602 | 62 |
| Hill 7 30 m | 1,188 | 684 | 62 |

- The still-camera plans of this window are 1.3 to 1.75 times those of window 23 (C: Platonmax 161, Hill 7 95, Ridge 3 1,998 ms at 2 km). Not explained: the plan jobs run on the CPU and the build differs from window 23's C only by the ground detail and the viewer's F6, but the editor window now draws with the detail on (natural) while the worker plans. Not checked: a rerun with `F6` off.
- Warm, Ridge 3 at 2 km is 408 ms against 2,673 cold (6.5 times less): the first-plan cost is cold, as tracked. At 200 m and 30 m the warm plan is 648 and 663 ms, the stamp stage's own cost over the ridge item (the editor chain with the stamps), which the dive and the unedited ground do not pay.
- No plan refused (0 in every stop).

## Not measured

- The detail's cost per term, and in the viewer's atmosphere path at orbit or aerial views (the ground views only).
- The sweep with the detail off.
