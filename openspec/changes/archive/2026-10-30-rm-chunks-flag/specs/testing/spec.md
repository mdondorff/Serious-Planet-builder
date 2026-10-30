## MODIFIED Requirements

### Requirement: TEST-054 The envelope scene runs the chunk stage
The envelope scene (TEST-047) SHALL run the horizon jobs from the base terrain when its world has a store, and no chunk stage (ADR 0086: the stage is removed). The jobs are driven every frame from the plan's tiles and every flight starts with no horizon map resident. A world without a store SHALL keep the tile-only scene.

Verify: A
Status: active
Source: M8 plan, TEST-047, STRM-064, ADR 0086

#### Scenario: The stage of a flight
- **WHEN** the envelope scene is set up with a store and with an in-memory bake
- **THEN** the first runs the horizon jobs and no chunk stage, and the second has none

#### Scenario: The start crosses an edge half way
- **WHEN** the start of a flight of any length is searched over a chunk grid with edges at fixed latitudes, from starts at many phases
- **THEN** the flight starts in one chunk, crosses exactly one edge at half its length and ends in the next, and a start with no edge within 0.2 degrees stays where it is

#### Scenario: A frame waits for the stage
- **WHEN** a frame has a plan node that stands in for a chunk, a chunk job in flight or a horizon job in flight, and when it has none of them
- **THEN** the frame is busy in the first three cases and not in the last, and the settle before a flight waits for a frame that is not busy

### Requirement: TEST-064 The gate runs without the chunk stage by default
`cargo xtask gate` SHALL run the sites of TEST-062 over the base terrain and report the step between levels of the base field alone; the stage-on run (`gate --chunks`) no longer exists (ADR 0086). `gate --small` SHALL run the small world once.

Verify: C
Status: active
Source: opus-planner (2026-10-28), owner fallback 2026-10-28, TEST-061, TEST-062, ADR 0086

#### Scenario: Stage off
- **WHEN** the window runs over a store with a coupled run
- **THEN** no chunk job is computed and the gate line reports `chunk jobs: None`
