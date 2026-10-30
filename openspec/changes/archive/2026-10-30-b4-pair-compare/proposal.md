## Why
B4-4 step 1 (design PR #928; planner reserved TEST-077, 2026-10-30): the owner sees a dragged edit differ from the same edit at rest. `gpu_preview.rs` holds the bound (the preview leaves at most a twentieth of the pixels the edit changes) inline; the real-store pair scene (TEST-076, later) needs the same judgement as a function with its own tests.

## What Changes
1. `planet_testkit::pair`: `compare_pair(stale, preview, exact, grey)` over three RGBA8 frames gives the pixels the edit changes, the pixels the preview leaves, the mean and largest difference, the bounding box and a verdict (Within, Over, TooSmall); `first_over` names the first view whose pair is over the bound.
2. Tier-A tests on planted pairs (equal, stale as preview, exactly at the bound and one over, a tiny edit, a size mismatch).

## Impact
- Spec: `testing` gains TEST-077. Code: `crates/testkit/src/pair.rs`. No scene and no GPU run in this change.
