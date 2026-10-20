## Why
`gpu_edit_perf::the_edit_latency_scene_runs_end_to_end_in_smoke_mode_and_prints_its_table` timed out at 300 s on Windows CI tier C twice, after the regional, brush, line and region cases joined the scene (opus-planner, 2026-10-20). A smoke run proves the path runs end to end; it does not need a long calm per case.

## What Changes
- A smoke run keeps its cases (one per kind at 200 m: the 50 m stamp, the 5 km regional stamp, the brush, the ridge, the region) and waits for fewer frames: a case settles after 3 calm frames (20 in a measured run), the preview cost steps 1 frame after setting the preview (3), renders once (20 in a measured run) and settles after 2 calm frames (5).
- A measured run is unchanged. The timeout is not raised.
- Locally the smoke test takes about 20 s, down from about 32 s, while another job loads the CPU.

## Classes and bounds (ADR 0035)
No field, no shader value and no golden is touched.

## Out of scope
The scene's measured cases; the table; a smaller smoke set (every kind is still run once).
