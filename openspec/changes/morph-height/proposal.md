## Why

The ragged dark band in the relief flight's 1 m glide frames (after LOD-008 put level transitions in front of a ground camera) prompted two investigations that found two real defects:

1. **The height was not geomorphed.** The vertex shader morphed the position towards the parent grid but passed the fine vertex's own height on, so everything that reads the height (relief contours and tint, the height view) disagreed with the geometry inside morphing tiles and jumped between a morphed and an unmorphed neighbour. Measured on the reference field: a morphing vertex's coarse height differs from its own by metres to tens of metres at the 2,000 and 200 km views.
2. **The band itself was the relief grid, not the height** (found while testing the first diagnosis: the band survives switching off contours, shading, and the morph of the local position, and disappears with the grid, y-axis only). The per-node grid phase for the coarse decades was a 1,000 km remainder in metres added to the tile-local offset in f32, which rounds to about 6 cm: the coordinate of the 10 m decade became a staircase of plateaus about 40 px wide at 1 m altitude, and a whole plateau satisfied "on a line".

## What Changes

- `MeshVertex::coarse_height` (generators): the parent-grid rule on heights, exactly as `coarse` applies it to positions. `MESH_VERSION` 4, vertex layout 11 floats (44 bytes), known answers updated. The vertex shader mixes it with the position's morph factor, so `height` is morphed.
- New debug view `morph-height`: the shader's morphed height minus the vertex's own, over the plan's height range as the delta window. It is the observable for the tier-B oracle (the `height` view quantises to about 31 m per grey level).
- Relief grid: the node uniform carries, per decade (1 cm to 1,000 km) and axis, the f64 fraction of `origin / spacing` (`grid_phases`, nine `vec4`s); the shader adds `local / spacing` to it, so the line coordinate stays small at every decade. Decades whose pixel is narrower than about 4e-7 cells fade out (an f32 fraction is good to about 6e-8 cells): distant 10 km to 1,000 km lines vanish near the ground instead of drifting centimetres.
- Tests: tier A (coarse heights equal the parent grid's; a fully morphed border height equals the coarser neighbour's own border height; grid phase maths including the ground-pixel resolution of every decade), tier B (oracle for the shader's height morph, shown to fail when the shader leaves the height unmorphed), tier C (the relief view at the ground cameras has only thin grid lines; the detector flags a committed image of the pre-fix defect).
- Known answers and snapshots that follow from `MESH_VERSION` 4 and the 44-byte vertex: mesh content hashes, the mesh key of the body-key test, the stress mesh, the terrain plan snapshots (the plan carries mesh keys), the upload byte budget per tile.

Expected golden changes:
- terrain_aerial200km_height: the height view's ramp and contours now follow the morphed height (849 pixels on WARP, max channel delta 33, derivative-drawn contour lines).

## Impact

`crates/generators` (mesh), `crates/render` (shader, vertex layout, node uniform), `crates/frame` (view enum, upload budget), `crates/app` (view names), tests, specs. The two relief goldens (first goldens, lit) stay pending for the owner and their candidates are regenerated; the earlier relief flight review should be redone on the corrected view. Normals are still not geomorphed (shading uses the fine vertex's normal); a follow-up if it shows.
