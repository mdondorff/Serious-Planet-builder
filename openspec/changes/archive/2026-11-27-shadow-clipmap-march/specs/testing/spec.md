## MODIFIED Requirements

### Requirement: TEST-054 The envelope scene runs the chunk stage
The envelope scene (TEST-047) SHALL run over the base terrain when its world has a store, with no chunk stage (ADR 0086) and no horizon jobs (ADR 0085). A world without a store SHALL keep the tile-only scene. The text is reworded against the spec at the time of PR 7, after B6 has removed the chunk scenarios.

Verify: A
Status: active
Source: M8 plan, TEST-047, ADR 0086, ADR 0085

#### Scenario: The stage of a flight
- **WHEN** the envelope scene is set up with a store and with an in-memory bake
- **THEN** neither runs a chunk stage or a horizon job

#### Scenario: The start crosses an edge half way
- **WHEN** the start of a flight of any length is searched over a chunk grid with edges at fixed latitudes, from starts at many phases
- **THEN** the flight starts in one chunk, crosses exactly one edge at half its length and ends in the next, and a start with no edge within 0.2 degrees stays where it is

#### Scenario: A frame waits for the stage
- **WHEN** a frame has a plan node that stands in for a missing tile, and when it has none
- **THEN** the frame is busy in the first case and not in the second, and the settle before a flight waits for a frame that is not busy

## REMOVED Requirements

### Requirement: TEST-065 The gate reports the horizon re-bake after its edit; the edit-latency scene tabulates it
**Reason**: ADR 0085 point 7; the shadow follows an edit in the frame its tile lands (REND-047, REND-050).
**Migration**: PR 7 of `shadow-clipmap-march`.

### Requirement: TEST-066 The edit-latency horizon rows are measured on a quiet horizon
**Reason**: ADR 0085 point 7.
**Migration**: PR 7.
