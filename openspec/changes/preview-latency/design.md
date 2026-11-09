## The scene
`preview_latency [n] [--threads N] [--smoke]` (default n = 3,072 cells a face edge, 56.6e6 cells, 3.26 km at Earth; `--smoke` n = 96).

| Step | What | Timed as |
|---|---|---|
| base | the heights, rainfall and temperature of every cell from the default class source and the orographic climate, in parallel over rows; the baseline routing and discharge (`PreviewBase::new`) | once per session: `base_s`, not an edit latency |
| edit A | one hill of 300 m with a reach of 400 km on land | `edit_s` = stamps + route + discharge + overlay |
| edit B | one rainfall region (factor 2, reach 800 km) over a catchment, no height change | same |
| edit C | edit A and edit B together | same |

Each edit is run 3 times after one warm run; the report gives the median and the maximum. Phases: the stamp pass (`edited_heights`), the flood and routing (`GlobalGrid::route`), the discharge, the overlay (trunk set, changed cells). The process peak working set after the base and after each edit (PowerShell, as `landmass_scale`), and the resident bytes of the session (`z0`, `p`, `t`, the baseline routing and discharge) and of one preview.

## Why the phases
F6 measured 7 s and 2.2 GB for the route alone (A3 5.1). The preview adds the stamp pass (every cell's direction is computed to test the reach: 56.6e6 square roots), the discharge pass and the overlay, and clones `z0` and the baseline routing; the split tells which part is worth a row-parallel loop or an index of the cells inside a cap.

## Report and exit-list number
The report is a table per edit kind: median and maximum seconds, the phases, the peak and the resident bytes, with the machine's readiness state. The briefing's exit list asks for "the latency of the preview levels": the number is `edit_s` of edit A at n = 3,072 (a seconds-scale target; F6's 7 s is the reference, not a pass bound until the architect sets one).

## Gate
`cargo xtask perf preview` runs the example in release under the heavy-run lock like every xtask command that builds or runs. It has no readiness gate of its own (the gate lives in the `test-render` harness this example does not use): a timing read is taken in a perf window that sonnet-bulldozer authorises, on a quiet machine, after `cargo xtask perf preflight`. `--smoke` runs at n = 96 on any machine and prints the table labelled NOT A MEASUREMENT. A run before the window is labelled untimed.

## Phases in the library
`PreviewBase::preview_timed` returns the seconds of the stamp pass, the route, the discharge and the overlay beside the preview (`Phases`); `preview` calls it. The clock reads nothing that enters the result (a tier-A test compares both calls).
