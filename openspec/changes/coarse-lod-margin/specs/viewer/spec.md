## MODIFIED Requirements

### Requirement: VIEW-014 Debug overlay and coarse LOD view
The editor window SHALL be able to draw a debug overlay over the lit terrain so that the real mesh density and the levels of detail are visible, and to show the mesh at a coarser level of detail for inspection. F3 cycles the overlay (off, tiles, mesh, off) and F4 toggles the coarse LOD; both are off at the start; both act only in `planet editor` and on the lit path (the headless modes, `test-render` and the golden paths never set them, so no golden moves). The overlay is plan data (CON-03): `planet_frame::look::Overlay` (`None`, `Tiles`, `Mesh`; further debug overlays plug into the same enum; `ALL` and `next` give the cycle, `shader_code` 0, 1, 2) is a field of `LitPlan` (`overlay`), `LitPlan::with_overlay` sets it, and the plan text and hash carry it only when it is not `None` (` overlay tiles` or ` overlay mesh` after the other tokens, parsed back and validated). The renderer does not branch on a mode of its own: the plan's value reaches the lit fragment stages through the globals (`extra.y`) and the node record (`morph.w` carries the tile's level). The fragment form needs no wgpu feature, so it runs on WARP, lavapipe and the discrete GPU: from the compact tile's grid coordinates it draws, in `Tiles`, a faint tint of the level's colour and the tile's outline, and in `Mesh` also a line where the fragment is within about a pixel of a grid edge or of the diagonal of the grid's triangulation (the cell's `a` to `c`, as `grid_indices`), a fixed line that does not fade out however fine the mesh is. The colour is one of eight per level, wrapping; the brightness falls with the vertex morph factor (full detail bright, fully morphed on the parent grid 0.3). The lines are radiance in scene units (the pre-exposure factor of the frame times 0.8), so they show at any exposure. With the overlay `None` the frame is the lit frame bit for bit. The coarse LOD multiplies the LOD error bound `tau_px` by 4 (`COARSE_LOD_FACTOR`), or by the largest factor below 4 that keeps `LodParams::validate` satisfied (`LodParams::coarse`, `coarse_factor`; the production 1080p parameters, 6 px and 32 cells, allow about 1.7, VIEW-032), in the request the frame is planned with (`effective_lod`; terrain and water alike), so the mesh edges are about that many times longer on the screen: it changes the view, not the data, and is decided in the GPU-free plan, never in a shader. The coarser bound can fail on steep terrain (the plan worker reports a tile whose morph interval has no room): the frame then falls back to the base parameters, the switch turns itself off (F4 asks again) and the smoke summary counts the refusal; the window never stops for it. The editor's corner label shows the state (`overlay_status`: `Overlay: Mesh, LOD x4`, `Overlay: Tiles, LOD x2.6`, `Overlay: off, LOD x1`; the factor is the one the plan uses).

Verify: A, C
Status: active
Source: owner (2026-10-19, via opus-planner), CON-03, CON-18, VIEW-011

#### Scenario: The plan carries the overlay
- **WHEN** a lit plan is made, given tiles or mesh, written as text, parsed back, and given an unknown overlay name
- **THEN** it has no overlay by default, the text of a plan without one has no overlay token (hashes are as before), the overlays add ` overlay tiles` and ` overlay mesh`, the plan round-trips with its other tokens, and the unknown name is an error naming it

#### Scenario: F3 and F4
- **WHEN** F3 is pressed three times and F4 twice in the editor, in the lit and debug paths
- **THEN** the overlay goes tiles, mesh, off (off at the start, kept when the debug path is shown), the coarse LOD turns on and off independently, the keys help names both, the coarse parameters are the base ones with only `tau_px` changed, times 4 where that is valid and else the largest valid factor (always valid, the production 1080p ones about 1.7), the corner label reads `Overlay: Mesh, LOD x4` or `Overlay: off, LOD x1`, and the window smoke counts lit frames drawn with an overlay, with the mesh, and frames planned with the coarse LOD

#### Scenario: A refused coarse plan
- **WHEN** the plan with the coarse LOD fails and the base one works, when the coarse view is off and the plan fails, when both fail, and when the coarse plan works
- **THEN** the first uses the base plan, turns the switch off and counts one refusal, the second returns its error at once with no second attempt, the third returns the second error and counts one refusal, and the fourth changes nothing

#### Scenario: The overlays change the terrain's pixels only
- **WHEN** lit frames of a close view, a mid view and the orbit view are drawn on the software adapter with the overlay none, tiles and mesh
- **THEN** the none frame equals the plain lit frame, the mesh frame is deterministic, no pixel outside the planet changes, a close view has at least 50 changed terrain pixels in each overlay and the mesh differs from the tiles, and inspection images (production LOD and a coarse one, three overlays each) are written to `target/review/overlay-inspection/`

## ADDED Requirements

### Requirement: VIEW-032 The coarse view keeps clear of the plan's refusal at the large levels, and a late refusal does not end the window
`LodParams::coarse` SHALL return parameters that the plan accepts wherever it accepts the base ones at the levels whose node radius is at least half of `max_height_m` (`COARSE_WORST_CASE_RADIUS_FACTOR`; over the production 1080p parameters about 1.7 times the error bound, over the window's 320x200 ones about 2.2), so that over a field that has only a global height bound at most one pose in fifty of a sweep from 20 km altitude up is refused for the coarse parameters and not for the base ones; a refusal that remains falls back to the base parameters as VIEW-014 says: the morph interval of every level keeps 5 % of its end plus a margin of 1 % by the node radius, and at those levels where the base parameters meet the plan's own start (`split(l) + 2 x extent(r (1 + H / R), H)`, `LodParams::morph_interval_worst`) the coarse ones meet it too with the same margin. `LodParams::validate` SHALL stay the node-radius rule. A plan failure whose request carried other LOD parameters than the frame that receives it SHALL be dropped and counted (`PlanPipeline::dropped_plan_errors`), not returned; a failure of the frame's own request SHALL be returned as before.

Verify: A
Status: active
Source: opus-planner (2026-10-20), SPIKE-19, VIEW-014, LOD-003, LOD-011

#### Scenario: The window's coarse view over the eroded world's bound
- **WHEN** the window's LOD at 320x200 over a field whose global height bound is 9,735 m is made coarse and planned from five orbital poses, and again from 16 altitudes between 20 km and 4,000 km at two pitches at 320x200 and at 1920x1080
- **THEN** the plan accepts the five orbital poses (it refused them all with the former rule), and refuses the coarse LOD at no more than one pose in fifty among those where it accepts the base LOD

#### Scenario: A late refusal
- **WHEN** the plan of parameters too coarse for 16 cells is asked for and the frame then asks again with the base parameters, and later asks for the coarse parameters themselves
- **THEN** the first refusal is dropped and counted while the frame draws, and the refusal of the frame's own parameters is returned
