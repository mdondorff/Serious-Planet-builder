## Why

The M2 gate needs a terrain frame time on Rasierklinge, and the harness only timed a hello-triangle round trip. The one-shot executor also re-created everything per call, which would have dominated any timing.

## What Changes

- `render`: `TerrainRenderer` with persistent resources, `FrameTiming` (CPU, wall, GPU via timestamp queries), `execute_terrain` as a wrapper; the device requests timestamp queries when the adapter has them.
- `perf`/`app`: `test-render --perf --scene terrain` (1440p, four scripted views, 3 runs, 60 warm-up frames, budget verdicts, JSON with metrics and counts) and `--perf-smoke` for a labelled dry run; options `--cells`, `--tau`, `--frames`.
- REND-013 and TEST-012 active.

## Capabilities

### Modified Capabilities
- `renderer`: REND-013. `testing`: TEST-012.

## Impact

`crates/render`, `crates/perf`, `crates/app`. No measurement is taken by this change; the owner runs it on Rasierklinge.
