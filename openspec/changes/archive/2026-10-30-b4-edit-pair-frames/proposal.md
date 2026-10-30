## Why
B4-4 step 2 (design PR #928; planner reserved TEST-076, 2026-10-30): the owner sees a dragged edit differ from the same edit at rest. TEST-077 judges a pair of frames; nothing draws one through the editor's own edit path.

## What Changes
1. `planet::editperf::pair_frames`: per debug view, the stale frame, the edit as the editor makes it, the first frame after it (the REND-033 preview), the exact frame once the refresh settled, and a control frame after the edit is taken back.
2. A tier-C test on the smoke world (software adapter): the control equals the stale frame bit for bit, the edit is seen, and the verdict of each view is printed (reported, not asserted).
3. The real-store scene (`edit-pair`: lowland and hill site, 200 m and 2 km, lit frame, horizon off and on, pair.md) is the next step.

## First finding (software adapter, memory bake, a 50 m hill at 200 m)
- Normals view: the edit changes 9381 pixels, the preview leaves 0 (mean difference 0.01, largest 2): Within.
- Relief view: the edit changes 7152 pixels, the preview leaves 2836, about 40 % against the bound of 5 %: Over (largest 76, mean 0.14, differing pixels spread over x 166..767, y 118..409). The control is 0 pixels in both views.
- Reading: shading is right and the height contours of the preview are not those of the exact tiles; not checked: the size of the height difference in metres, the real store, the lit frame, the horizon. No fix in this change.

## Impact
- Spec: `testing` gains TEST-076. Code: `crates/app/src/editperf.rs` (`pair_frames`), `crates/app/tests/gpu_edit_pair.rs`, registered in `gpu_all.rs`. Needs `planet_testkit::pair` (TEST-077, #930).
