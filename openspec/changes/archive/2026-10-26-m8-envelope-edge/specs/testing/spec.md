## MODIFIED Requirements

### Requirement: TEST-054 The envelope scene runs the chunk stage
When its world has a stored coupled run, the envelope scene (TEST-047) SHALL give the world the A-net chunk stage of the viewer (cells of 8 m, its own worker pool, its horizon jobs, REND-038), drive the chunk jobs every frame from the plan's tiles and pass what landed and what was evicted to the managed loop, and start every flight with no chunk resident, the chunks of the start view ready and a chunk edge half way along the flight (the worst phase: fresh chunks ahead of the eye). The stand-in share (STRM-039) SHALL count in the judged share. A world without a stored run SHALL keep the tile-only scene.

Verify: A
Status: active
Source: M8 exit criterion 7 (owner, 2026-10-23, D4), TEST-047, STRM-039, STRM-046

#### Scenario: The start crosses an edge half way
- **WHEN** the start of a flight of any length is searched over a chunk grid with edges at fixed latitudes, from starts at many phases
- **THEN** the flight starts in one chunk, crosses exactly one edge at half its length and ends in the next, and a start with no edge within 0.2 degrees stays where it is

#### Scenario: A frame waits for the stage
- **WHEN** a frame has a plan node that stands in for a chunk, a chunk job in flight or a horizon job in flight, and when it has none of them
- **THEN** the frame is busy in the first three cases and not in the last, and the settle before a flight waits for a frame that is not busy

### Requirement: TEST-055 The envelope log gives what the chunk stage did
The log of the envelope scene (TEST-047) SHALL say whether the chunk stage ran, where along the flights the start put a chunk edge and, for each trial, give the chunks landed, the horizon maps landed, the median and the largest wait of a landed chunk (from the first frame that wanted it to the frame that saw it land), and the chunk and horizon jobs in flight at the end. One altitude (300 m) SHALL be flown again with the horizon jobs off and given as a comparison line. A log without a chunk stage SHALL say why.

Verify: A
Status: active
Source: M8 exit criterion 7 (owner, 2026-10-23, D4), TEST-054, planner decision 2026-10-26

#### Scenario: The wait of a chunk
- **WHEN** a chunk is first wanted by a frame at 1.0 s and seen landing by a frame at 3.5 s, another is wanted at 1.0 s and again at 2.0 s and lands at 7.0 s, and a third lands that the flight never asked for
- **THEN** the waits are 2.5 s, 6 s and 0 s, the median is 2.5 s and the largest 6 s

#### Scenario: The log
- **WHEN** the scene's log is written for a flight with a chunk stage and a comparison point, and for one without a stage
- **THEN** the first holds the per-trial stage facts and the comparison, and the second says `stage: null`, no comparison and the reason
