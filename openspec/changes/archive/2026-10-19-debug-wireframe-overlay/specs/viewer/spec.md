## ADDED Requirements

### Requirement: VIEW-014 Wireframe overlay
The editor window SHALL be able to draw a wireframe over the lit terrain so that the real mesh density and the levels of detail are visible. F3 toggles it; it is off at the start; it is drawn only by `planet editor` on the lit path (the headless modes, `test-render` and the golden paths never set it, so no golden moves). The overlay is plan data (CON-03): `planet_frame::look::Overlay` (`None`, `Wireframe`; further debug overlays plug into the same enum) is a field of `LitPlan` (`overlay`), `LitPlan::with_overlay` sets it, and the plan text and hash carry it only when it is not `None` (` overlay wireframe` after the other tokens, parsed back and validated). The renderer does not branch on a mode of its own: the plan's value reaches the lit fragment stages through the globals (`extra.y`) and the node record (`morph.w` carries the tile's level). The fragment form needs no wgpu feature, so it runs on WARP, lavapipe and the discrete GPU: from the compact tile's grid coordinates it draws a tinted tile (a faint tint of the level's colour), the tile's outline, and a line where the fragment is within about a pixel of a grid edge or of the diagonal of the grid's triangulation (the cell's `a` to `c`, as `grid_indices`), thinned and faded out where the cells are only a few pixels wide (the mesh edge the LOD keeps is about 2.5 px). The colour is one of eight per level, wrapping; the brightness falls with the vertex morph factor (full detail bright, fully morphed on the parent grid 0.3). The lines are radiance in scene units (the pre-exposure factor of the frame times 0.8), so they show at any exposure. With the overlay `None` the frame is the lit frame bit for bit.

Verify: A, C
Status: active
Source: owner (2026-10-19, via opus-planner), CON-03, CON-18, VIEW-011

#### Scenario: The plan carries the overlay
- **WHEN** a lit plan is made, given a wireframe, written as text, parsed back, and given an unknown overlay name
- **THEN** it has no overlay by default, the text of a plan without one has no overlay token (hashes are as before), the wireframe adds ` overlay wireframe`, the plan round-trips with its other tokens, and the unknown name is an error naming it

#### Scenario: F3
- **WHEN** F3 is pressed twice in the editor
- **THEN** the overlay turns on and off, it is off at the start, the keys help names F3, and the window smoke counts lit frames drawn with the wireframe

#### Scenario: The overlay changes the terrain's pixels only
- **WHEN** lit frames of a close view, a mid view and the orbit view are drawn on the software adapter with the overlay none and wireframe
- **THEN** the none frame equals the plain lit frame, the wireframe frame is deterministic, no pixel outside the planet changes, a close view has at least 50 changed terrain pixels, and inspection images (production LOD and a coarse one) are written to `target/review/overlay-inspection/`
