## MODIFIED Requirements

### Requirement: VIEW-006 Presentation blit
The window surface SHALL show the terrain renderer's colour target scaled to fit the window keeping its aspect, centred, with black bars; an outdated, lost, timed-out or occluded surface SHALL be a skipped frame (reconfigured where that recovers it), not an error. The 3D target SHALL follow the window: once a size has held for 150 ms the renderer SHALL make its targets again at it, keeping its tiles, pipelines and pools, and the camera's aspect SHALL be the new size's.

Verify: A, C
Status: active
Source: ADR 0015, ADR 0042 point 4, owner 2026-10-30 (the 3D target kept its size after a window resize)

#### Scenario: Fit
- **WHEN** a 4x2 frame is blitted into a square target
- **THEN** the middle rows hold the frame, scaled, each source texel's colour at its centre, and the rows above and below are black

#### Scenario: Resize
- **WHEN** a lit renderer (also with the atmosphere) that drew at one size is resized to another and draws a plan of the new size, and then back
- **THEN** the frame is the frame of a renderer built at that size, byte for byte, the colour, depth, lit, atmosphere and water targets are made again, the exposure snaps on the next lit frame, and a plan of the old size is refused with a size mismatch

#### Scenario: Settle
- **WHEN** a window size is reported and 10 ms later, and again after 150 ms
- **THEN** the target follows the second only, a size equal to the current one asks for nothing, the projection's aspect is the target's width over its height, and a minimised window (size 0) keeps the target
