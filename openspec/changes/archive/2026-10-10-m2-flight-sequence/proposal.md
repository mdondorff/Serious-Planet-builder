## Why

M2 gate (owner asked): "orbit to 1 m without jitter" was proven only on the emulated shader path; the real-GPU flight needed the owner, but `test-render` rendered only single scripted views at 256x192 with small LOD parameters, and the tier-B oracles always forced the software adapter.

## What Changes

- `frame::flight_path`: deterministic orbit-to-1 m dive (log-spaced altitude, pitch 85 to 8 degrees) plus a 5 cm/frame glide at 1 m.
- `planet test-render --scene terrain --flight N --out-dir DIR [--size WxH] [--view V]`: renders it through `TerrainRenderer` with production LOD parameters (`--cells`, `--tau`), writes `frame_NNNN.png` and `flight.txt`; resident meshes capped at 12,000 (renderer rebuilt when exceeded). Mesh generation and upload shared with the perf workload (`upload_missing_meshes`).
- The tier-B oracles use `PLANET_ADAPTER`; `cargo xtask test gpu --real` runs them with tier C (`tier_filter`).
- Options a flight would ignore are rejected before any GPU work; the tier-C flight test follows `PLANET_ADAPTER` and writes to `target/review/flight-test/`.
- Added: TEST-013 (harness, tier C), TEST-014 (owner's real-GPU review, tier D, planned). Modified: BUILD-003 (`test gpu --real` adds tier B).

## Impact

`crates/frame`, `crates/app`, `crates/render/tests` (adapter selection only), `xtask`. No pixel or golden changes.
