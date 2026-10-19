## Why
The owner wants to see the real mesh density of the terrain and how the level of detail hides tiles that are being updated (2026-10-19, via opus-planner). A debug overlay in the editor window shows it, chosen as plan data so that the bigger overlays of the main lane (maps, authored regions, surface type) plug into the same value.

## What Changes
- `planet_frame::look::Overlay { None, Wireframe }` and `LitPlan.overlay` (`with_overlay`); text token ` overlay wireframe` only when set (hashes of existing plans do not change); parsed and validated.
- The lit fragment stages (`fs_lit`, `fs_lit_atmo`) call `overlay_wire` when the globals say so (`extra.y`); the compact tile's version draws the tile tint, the tile outline and the cell lines (grid edges and the triangulation's diagonal) from the grid coordinates with `fwidth`; the reference renderer has an identity stub. The node record's last lane carries the tile's level.
- `Command::ToggleWireframe` (F3), `ViewerState.overlay`, the keys help, the plan of every lit window frame, `SmokeCounts.wireframe_frames` and the smoke summary (`N with the wireframe`); the smoke script presses F3 at frames 26 and 30.
- VIEW-014.

## Key and run command
F3 in `planet editor` (any lit view). Run: `cargo run --release -p planet -- editor`, then F3.

## Measured
At the production LOD bound (2.5 px) the mesh edge is about 1.4 px, so cell lines fade out and the tile outline and the level tint carry the picture; at a coarser bound the cell lines and the diagonals are drawn (inspection images `view3-coarse-wireframe.png`, `view6-coarse-wireframe.png` in `target/review/overlay-inspection/`).

## No golden moves
The overlay is off in every golden path; the tier-C test checks the `None` frame equals the plain lit frame; `cargo xtask test gpu` ran with no candidate.
