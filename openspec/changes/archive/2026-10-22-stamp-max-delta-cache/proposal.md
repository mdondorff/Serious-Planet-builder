# Change: stamp-max-delta-cache

## Why
- SPIKE-29 fix 1: 99.8 % of a warm plan job at 1,000 items is `StampStage::max_abs_height_m` (an O(items) walk per call, about 20,000 calls a plan). opus-planner approved the cache (2026-10-22).

## What changes
- `StampStage` keeps the sum of the layer's `max_delta_m` per item revision (`Mutex<Option<(revision, sum)>>`).
- GEN-032.

## Tests
- Tier A (`stamp_stage.rs`): the cached bound equals the uncached walk after items are added, removed, changed and moved between layers.

Expected golden changes: none (no output changes).
