## MODIFIED Requirements

### Requirement: TEST-054 The envelope scene runs the chunk stage
When its world has a stored coupled run, the envelope scene (TEST-047) SHALL give the world the A-net chunk stage exactly as the viewer's window does (the same function, STRM-046), drive the chunk jobs every frame from the plan's tiles and pass what landed and what was evicted to the managed loop, and start every flight with no chunk resident, the chunks of the start view ready and a chunk edge half way along the flight (the worst phase: fresh chunks ahead of the eye). The stand-in share (STRM-039) SHALL count in the judged share. A world without a stored run SHALL keep the tile-only scene.

Verify: A
Status: active
Source: M8 exit criterion 7 (owner, 2026-10-23, D4), TEST-047, STRM-039, STRM-046

#### Scenario: The start crosses an edge half way
- **WHEN** the start of a flight of any length is searched over a chunk grid with edges at fixed latitudes, from starts at many phases
- **THEN** the flight starts in one chunk, crosses exactly one edge at half its length and ends in the next, and a start with no edge within 0.2 degrees stays where it is

#### Scenario: A frame waits for the stage
- **WHEN** a frame has a plan node that stands in for a chunk, a chunk job in flight or a horizon job in flight, and when it has none of them
- **THEN** the frame is busy in the first three cases and not in the last, and the settle before a flight waits for a frame that is not busy
