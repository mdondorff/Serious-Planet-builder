# Change: scale-refs

## Why
- M7 SR (owner, 2026-10-22, via opus-planner): a debug key drops plain, known-size boxes in front of the camera (a chair 0.9 m, a house 8 m, a boat 10 m, a tower 291 m, a block of boxes 1 km), so the size of the ground detail and the relief can be judged; the second ground-detail look pass (GT) needs them in its stills.

## What changes
- `planet_frame::scalerefs` (GPU-free): the sizes, the placement on the ground, the drop from a camera.
- `Command::ToggleScaleRefs` on F7; the window drops the set where the camera looks and removes it.
- `render::refs`: a pass of instanced boxes inside the terrain pass of the lit frames (no culling, the terrain's reversed-Z depth, Lambert shading at the plan's pre-exposure).
- VIEW-050, REND-036.

## Tests
- Tier A: the sizes and places; the key; the cube's winding.
- Tier C (`gpu_lit.rs`): drawn in front of the ground, hidden under it, absent when none, ignored by the debug path. No golden.

Expected golden changes: none (no reference is dropped in any golden).
