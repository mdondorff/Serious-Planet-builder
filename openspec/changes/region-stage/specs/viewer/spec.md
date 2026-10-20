## ADDED Requirements

### Requirement: VIEW-030 Region overlay
The debug overlay enum `planet_frame::look::Overlay` (VIEW-014) SHALL have a fourth value `Regions` (shader code 3, plan token `overlay regions`) that tints the lit terrain by the surface type of the region over each fragment. For each frame that shows it the app SHALL decide a `planet_frame::regions::RegionList` and hand it to the renderer (the renderer decides nothing, CON-03): the at most 8 region items of the definition nearest the camera (ties by item id), each as a polygon of its 3 to 64 points in the orthographic frame of its anchor (the anchor relative to the camera, the tangent plane's basis, and the points in metres in that plane; f64 on the CPU, rounded once to f32, CON-11), with the surface type's tint as linear light, and the count of the regions it left out (the farthest). The list SHALL have a stable text form (a header line and one line per polygon, floats as bit patterns) that round-trips, and an empty list has no text. The lit fragment stage SHALL take the fragment's position relative to the camera, subtract the anchor, project onto the basis, and test the point against the polygons nearest first by the even-odd rule, skipping a polygon whose plane is farther from the fragment than its band or whose reach the fragment is beyond; where it is inside a polygon it SHALL mix the tint at the weight 0.45 into the lit colour (`mix(lit, tint * 0.8 * pre-exposure, 0.45)`), with no texture and no planet-radius value in the shader. F3 cycles the overlay through off, tiles, mesh, regions, off, and the corner label names it. With the overlay not `Regions`, or with no polygon in the list, the frame is the lit frame bit for bit, and the headless modes and goldens never set the overlay (VIEW-014).

Verify: A, C
Status: planned
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
