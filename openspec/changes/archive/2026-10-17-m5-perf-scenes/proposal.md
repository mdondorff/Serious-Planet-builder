## Why
M5 item 8, second slice (status.md item 8; the owner's M5 budgets of 2026-10-15: atmosphere at most 1 ms, water at most 2 ms, frame median at most 16.6 ms and p99 at most 20 ms at 1440p in a valid window; ADR 0034 and ADR 0036 for what a GPU verdict needs). The harness measures the M4 scenes on the debug path and times only the terrain and (since item 7) the water pass. Window 6, the first M5 window, needs the lit path measured by pass, the M4 budgets measured again on the lit path, and the compact-against-vertex ratio the M4 report left open.

## What Changes
- `render::gpu_timing` (REND-028): one pair of timestamps for each of the lit frame's passes (terrain, water, sky view, aerial perspective, sky composite, exposure histogram, exposure adapt, tone map), the scene copy as the gap before the water pass, the frame's GPU span; `FrameTiming::passes`. The terrain and water queries keep their numbers, so `gpu_ms` and `water_gpu_ms` read as before.
- `frame::litscenes` (TEST-025): the views of the lit scenes and the coastal flights (the shoreline is found in the world by a walk along the parallel and a bisection).
- `planet_perf::lit` (TEST-025): metric names, the budget rows of a static view and of a flight, the proposed ADR 0036 point 3 floor as a recorded figure.
- `planet::litperf`: scenes `lit` (five held views, three runs each, the noise rule with one retry; the two flights frame by frame, with `water.shore_filter` at the look's default and at the other setting, report only) and `lit-formats` (four views in the vertex and the compact tile format back to back, the per-pass ratio); `enable_lit` and the water pass are warmed before any measured frame (REND-017) in these scenes and in the streaming scenes' new `--lit` variant (the same scripts, the same verdicts, own logs `<scene>-lit.json`, `protocol.field` naming the lit path).
- `cargo xtask perf lit | lit-formats | stream ... --lit | m5`: the command lines of the window.

## Impact
Specs: REND-028 and TEST-025 added. No golden: no pixel changes (a test pins that a timed lit frame draws the pixels of an untimed one). No MESH_VERSION or GLOBAL_VERSION change. The streaming scenes without `--lit` are unchanged; their M4 logs and acceptance read as before. M5 acceptance (`accept M5`) is the next slice.
