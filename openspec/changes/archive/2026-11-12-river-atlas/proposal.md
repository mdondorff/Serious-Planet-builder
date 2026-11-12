## Why
The river-flight smoke hit the 256-layer coverage array: 256 river nodes carried, 26 to 45 dropped in a fine-LOD flight. A dropped river is a held-back tile under G-RT (architect, 2026-11-12: no silent drops; the atlas replaces the array; not taken: distance culling, adapter-dependent layer limits).

## What Changes
- The renderer's coverage texture becomes one 2D atlas of `RIVER_SLOTS` (2,048) slots, each `cells + 3` texels a side: the node's samples inside a one-texel border that repeats its edge texels, so the hardware's bilinear read at an edge sees what a clamped read of the node saw. The node's word carries the slot (16 bits), the strength (8) and the samples a side (8).
- REND-061's text: layers become slots. The river-flight scene FAILS when any river was dropped (G-RT asserts dropped == 0).
- The atlas is bit-identical to the array: the river golden candidate rendered from the atlas equals the one from the array (same adapter, same driver); the eight river GPU tests pass unchanged.

## Impact
- Code: `render::terrain` (shader read, texture, write), `app::litperf` (the assertion), tests. No format or golden moves.
