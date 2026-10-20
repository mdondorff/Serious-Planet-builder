## ADDED Requirements

### Requirement: VIEW-030 Region overlay
The debug overlay enum `planet_frame::look::Overlay` (VIEW-014) SHALL have a fourth value `Regions` (shader code 3, plan token `overlay regions`) that tints the lit terrain by the surface type of the region over each fragment. For each frame that shows it the app SHALL decide a `planet_frame::regions::RegionList` and hand it to the renderer (the renderer decides nothing, CON-03): the at most 8 region items of the definition nearest the camera (ties by item id), each as a polygon of its 3 to 64 points in the orthographic frame of its anchor (the anchor relative to the camera, the tangent plane's basis, and the points in metres in that plane; f64 on the CPU, rounded once to f32, CON-11), with the surface type's tint as linear light, and the count of the regions it left out (the farthest). The list SHALL have a stable text form (a header line and one line per polygon, floats as bit patterns) that round-trips, and an empty list has no text. The lit fragment stage SHALL take the fragment's position relative to the camera, subtract the anchor, project onto the basis, and test the point against the polygons nearest first by the even-odd rule, skipping a polygon whose plane is farther from the fragment than its band or whose reach the fragment is beyond; where it is inside a polygon it SHALL mix the tint at the weight 0.45 into the lit colour (`mix(lit, tint * 0.8 * pre-exposure, 0.45)`), with no texture and no planet-radius value in the shader. F3 cycles the overlay through off, tiles, mesh, regions, off, and the corner label names it. With the overlay not `Regions`, or with no polygon in the list, the frame is the lit frame bit for bit, and the headless modes and goldens never set the overlay (VIEW-014).

Verify: A, C
Status: active
Source: ADR 0069, owner (2026-10-19), VIEW-014

#### Scenario: The list
- **WHEN** ten regions lie along the equator and the list is made for a camera over the first
- **THEN** it holds the eight nearest, nearest first, and counts the two left out; a definition without regions gives an empty list with no text

#### Scenario: The text
- **WHEN** a list of three polygons is written as text, parsed back, and given a header announcing nine polygons, one line fewer than announced, or a misspelt key
- **THEN** it round-trips exactly and the three malformed texts are errors naming the problem

#### Scenario: The plane
- **WHEN** a grid of points over and around a concave polygon is tested at the datum in the polygon's plane and by the gnomonic test that decides the stamp mask (WDEF-012), and the far side of a point inside the polygon is tested
- **THEN** the two agree on every grid point and the far side is outside

#### Scenario: F3
- **WHEN** F3 is pressed four times in the editor
- **THEN** the overlay goes tiles, mesh, regions, off, and the corner label names it

#### Scenario: The tint
- **WHEN** a lit frame of a close view with one polygon under the camera is drawn on the software adapter with the regions overlay and with none
- **THEN** every pixel the polygon covers is the plain lit colour mixed with the tint at the stated weight (within the half-float rounding of the scene target), every other pixel and every pixel outside the planet is identical, a polygon on the other side of the planet and an empty list change nothing, and the frame is deterministic

#### Scenario: The tint lands where the polygon is
- **WHEN** an L-shaped polygon is put under the ground the view shows and the pixels of a grid of ground points far from its edges are looked up through the camera
- **THEN** a pixel is tinted exactly when its ground point is inside the L, with points on both sides in view (a mirrored or swapped plane, or a missing edge, fails)

## MODIFIED Requirements

### Requirement: VIEW-014 Debug overlay and coarse LOD view
The editor window SHALL be able to draw a debug overlay over the lit terrain so that the real mesh density and the levels of detail are visible, and to show the mesh at a coarser level of detail for inspection. F3 cycles the overlay (off, tiles, mesh, regions, off; the regions overlay is VIEW-030) and F4 toggles the coarse LOD; both are off at the start; both act only in `planet editor` and on the lit path (the headless modes, `test-render` and the golden paths never set them, so no golden moves). The overlay is plan data (CON-03): `planet_frame::look::Overlay` (`None`, `Tiles`, `Mesh`, `Regions`; further debug overlays plug into the same enum; `ALL` and `next` give the cycle, `shader_code` 0, 1, 2, 3) is a field of `LitPlan` (`overlay`), `LitPlan::with_overlay` sets it, and the plan text and hash carry it only when it is not `None` (` overlay tiles`, ` overlay mesh` or ` overlay regions` after the other tokens, parsed back and validated). The renderer does not branch on a mode of its own: the plan's value reaches the lit fragment stages through the globals (`extra.y`) and the node record (`morph.w` carries the tile's level). The fragment form needs no wgpu feature, so it runs on WARP, lavapipe and the discrete GPU: from the compact tile's grid coordinates it draws, in `Tiles`, a faint tint of the level's colour and the tile's outline, and in `Mesh` also a line where the fragment is within about a pixel of a grid edge or of the diagonal of the grid's triangulation (the cell's `a` to `c`, as `grid_indices`), a fixed line that does not fade out however fine the mesh is. The colour is one of eight per level, wrapping; the brightness falls with the vertex morph factor (full detail bright, fully morphed on the parent grid 0.3). The lines are radiance in scene units (the pre-exposure factor of the frame times 0.8), so they show at any exposure. With the overlay `None` the frame is the lit frame bit for bit. The coarse LOD multiplies the LOD error bound `tau_px` by 4 (`COARSE_LOD_FACTOR`), or by the largest factor below 4 that keeps `LodParams::validate` satisfied (`LodParams::coarse`, `coarse_factor`; the production 1080p parameters, 6 px and 32 cells, allow about 2.6), in the request the frame is planned with (`effective_lod`; terrain and water alike), so the mesh edges are about that many times longer on the screen: it changes the view, not the data, and is decided in the GPU-free plan, never in a shader. The coarser bound can fail on steep terrain (the plan worker reports a tile whose morph interval has no room): the frame then falls back to the base parameters, the switch turns itself off (F4 asks again) and the smoke summary counts the refusal; the window never stops for it. The editor's corner label shows the state (`overlay_status`: `Overlay: Mesh, LOD x4`, `Overlay: Tiles, LOD x2.6`, `Overlay: Regions, LOD x1`, `Overlay: off, LOD x1`; the factor is the one the plan uses).

Verify: A, C
Status: active
Source: owner (2026-10-19, via opus-planner), CON-03, CON-18, VIEW-011, VIEW-030

#### Scenario: The plan carries the overlay
- **WHEN** a lit plan is made, given tiles, mesh or regions, written as text, parsed back, and given an unknown overlay name
- **THEN** it has no overlay by default, the text of a plan without one has no overlay token (hashes are as before), the overlays add ` overlay tiles`, ` overlay mesh` and ` overlay regions`, the plan round-trips with its other tokens, and the unknown name is an error naming it

#### Scenario: F3 and F4
- **WHEN** F3 is pressed four times and F4 twice in the editor, in the lit and debug paths
- **THEN** the overlay goes tiles, mesh, regions, off (off at the start, kept when the debug path is shown), the coarse LOD turns on and off independently, the keys help names both, the coarse parameters are the base ones with only `tau_px` changed, times 4 where that is valid and else the largest valid factor (always valid, the production 1080p ones about 2.6), the corner label reads `Overlay: Mesh, LOD x4`, `Overlay: Regions, LOD x1` or `Overlay: off, LOD x1`, and the window smoke counts lit frames drawn with an overlay, with the mesh, and frames planned with the coarse LOD

#### Scenario: A refused coarse plan
- **WHEN** the plan with the coarse LOD fails and the base one works, when the coarse view is off and the plan fails, when both fail, and when the coarse plan works
- **THEN** the first uses the base plan, turns the switch off and counts one refusal, the second returns its error at once with no second attempt, the third returns the second error and counts one refusal, and the fourth changes nothing

#### Scenario: The overlays change the terrain's pixels only
- **WHEN** lit frames of a close view, a mid view and the orbit view are drawn on the software adapter with the overlay none, tiles and mesh
- **THEN** the none frame equals the plain lit frame, the mesh frame is deterministic, no pixel outside the planet changes, a close view has at least 50 changed terrain pixels in each overlay and the mesh differs from the tiles, and inspection images (production LOD and a coarse one, three overlays each) are written to `target/review/overlay-inspection/`
