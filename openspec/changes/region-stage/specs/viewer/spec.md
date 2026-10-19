## ADDED Requirements

### Requirement: VIEW-030 Region overlay
The debug overlay enum `planet_frame::look::Overlay` (VIEW-014) SHALL have a fourth value `Regions` (shader code 3) that tints the lit terrain by the surface type of the region over each fragment. The frame plan SHALL carry up to 8 region polygons nearest the camera (each with its surface-type tint and up to 64 points in the plan's tile-local frame, f32 camera-relative, CON-11), count the ones it left out, and write them in the plan text after the overlay token (` overlay regions` and one line per polygon) so the plan round-trips and hashes with them; a plan with no overlay has no such text. The lit fragment stage SHALL test the fragment's position against the polygons by the even-odd rule in tile-local arithmetic and mix the tint at a weight of 0.45 into the lit colour where it is inside, with no texture. F3 cycles the overlay through off, tiles, mesh, regions, off. With the overlay `None` the frame is the lit frame bit for bit, and the headless modes and goldens never set it (VIEW-014).

Verify: A, C
Status: planned
Source: ADR 0069, owner (2026-10-19), VIEW-014

#### Scenario: The plan carries the regions
- **WHEN** a lit plan with the regions overlay and three polygons is written as text, parsed back, and given a ninth polygon
- **THEN** it round-trips, the ninth (farthest) is left out and counted, and a plan without the overlay has no region text and the hash it had before

#### Scenario: F3
- **WHEN** F3 is pressed four times in the editor
- **THEN** the overlay goes tiles, mesh, regions, off, and the corner label names it

#### Scenario: The tint
- **WHEN** a lit frame of a close view with one region is drawn on the software adapter with the regions overlay and with none
- **THEN** the pixels inside the polygon differ from the plain frame by the tint at the stated weight and the pixels outside are identical
