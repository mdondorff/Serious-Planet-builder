## Why
The owner wants to see the real mesh density of the terrain and how the level of detail hides tiles that are being updated (2026-10-19, via opus-planner). A debug overlay in the editor window shows it, chosen as plan data so that the bigger overlays of the main lane (maps, authored regions, surface type) plug into the same value. The owner's first screenshot (`docs/dev_screenshots/2026-10-19-wireframe-overlay-lod-levels.png`) showed the tile outlines but not the inner mesh: at the production error bound the cells are about 1.4 px, so the inner lines faded out. Hence a mesh state without fade-out and a coarse-LOD switch that makes the real triangles bigger on the screen (owner, 2026-10-19).

## What Changes
- `planet_frame::look::Overlay { None, Tiles, Mesh }` (`ALL`, `next`, `shader_code`) and `LitPlan.overlay` (`with_overlay`); text token ` overlay tiles` or ` overlay mesh` only when set (hashes of existing plans do not change); parsed and validated.
- The lit fragment stages (`fs_lit`, `fs_lit_atmo`) call `overlay_wire` when the globals say so (`extra.y` = 1 tiles, 2 mesh); the compact tile's version draws the level tint and the tile outline, and in mesh mode the cell lines (grid edges and the triangulation's diagonal, a fixed 1 px line, no fade-out) from the grid coordinates with `fwidth`; the reference renderer has an identity stub. The node record's last lane carries the tile's level.
- `LodParams::coarse` (the error bound times `COARSE_LOD_FACTOR` = 4, or the largest factor that keeps the parameters valid: the production 1080p ones, 6 px and 32 cells, allow about 2.6, as viewport / tau must stay above 2 x cells) and `viewer::effective_lod`: the editor plans terrain and water with the coarse parameters while the switch is on; decided in the GPU-free request, never in a shader. If the plan worker refuses the coarser bound on steep terrain (found by the window smoke: the morph interval of a level-5 tile had no room), the frame falls back to the base parameters and the switch turns itself off, counted in the smoke summary.
- `Command::CycleOverlay` (F3), `Command::ToggleCoarseLod` (F4), `ViewerState.overlay` and `.coarse_lod`, the keys help, `viewer::overlay_status` for the corner label (`Overlay: Mesh, LOD x4`) shown by the editor's shell panel (egui), `SmokeCounts.overlay_frames`, `.mesh_overlay_frames`, `.coarse_lod_frames` and the smoke summary; the smoke script cycles the overlay and turns the coarse LOD on while the mesh shows.
- VIEW-014.

## Keys and run command
F3 cycles off, Tiles, Mesh, off; F4 toggles the coarse LOD (tau x4, about x2.6 at the production 1080p parameters); both in `planet editor` on the lit path, off at the start. Run: `cargo run --release -p planet -- editor`.

## Measured
At the production LOD bound (2.5 px) the mesh edge is about 1.4 px: Tiles shows the 16-cell tile outlines and the level tint; Mesh covers the terrain with the cell lines (the picture is nearly solid); with F4 the cells are about four times larger and the triangles can be told apart (inspection images `view3-coarse-mesh.png`, `view6-coarse-mesh.png` and the production and off images in `target/review/overlay-inspection/`).

## No golden moves
The overlay is off in every golden path; the tier-C test checks the `None` frame equals the plain lit frame; `cargo xtask test gpu` ran with no candidate.
