## Why

M1 exit criteria require the walking skeleton: one CPU-generated tile through `FramePlan`, the executor and a debug view to pixels, in all three run modes, with a repro bundle and a live golden harness with exact-match debug views. This is the integration path every later milestone builds on, and the first real test of the "renderer executes a plan and decides nothing" boundary (CON-03).

## What Changes

- `frame`: `FramePlan` (tile draws with pixel rectangles, f64 origins, f32 camera-relative origins, colours), `PlanRequest` and `plan_tiles`, a stable hexadecimal text form with a committed snapshot, and `ReproBundle` (versions, seed, resolution, request, embedded plan, plan hash).
- `render`: `execute` (one pipeline, debug views face / tile-id / height, explicit bind layouts, count stats), `reference_frame` (CPU twin), `read_rgba8` helper, shader list for validation.
- `app`: `test-render --scene tiles`, `--repro`, `--bundle`; `editor --offscreen`; `generate --tile` with hashes, height map and face net.
- `xtask repro <bundle>` replays a bundle.
- REND-001, REND-006, TEST-008, TEST-009, TEST-010 become active; BUILD-004 now includes the skeleton in all modes; REND-001 becomes tier C and its GPU-culling clause moves to a new planned REND-008.

## Capabilities

### Modified Capabilities
- `testing`: TEST-008, TEST-009, TEST-010.
- `renderer`: REND-001, REND-006; new REND-008 (planned).
- `terrain-lod`: new LOD-006.
- `build-tooling`: new BUILD-007.

## Impact

`crates/frame`, `crates/render`, `crates/app`, `xtask`. Four candidate goldens (hello triangle, face / tile-id / height views) wait for the owner's `/bless`.
