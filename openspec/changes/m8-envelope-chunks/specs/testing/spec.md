## ADDED Requirements

### Requirement: TEST-054 The envelope scene runs the chunk stage
When its world has a stored coupled run, the envelope scene (TEST-047) SHALL give the world the A-net chunk stage of the viewer (cells of 8 m, its own worker pool, and its horizon jobs, REND-038), drive the chunk jobs every frame from the plan's tiles and pass what landed and what was evicted to the managed loop, start every trial with no chunk of an earlier trial resident and after the chunks of the start view are ready, and count the chunk stand-in share (STRM-039) in the judged share. Its log SHALL say whether the stage ran and give per trial the chunks landed, the median and the largest chunk job time and the jobs in flight at the end. One altitude (300 m) SHALL also run with the horizon jobs off and the log give that as a comparison line. A world without a stored run SHALL keep the tile-only scene and the log SHALL say why.

Verify: A
Status: planned
Source: M8 exit criterion 7 (owner, 2026-10-23, D4), TEST-047, STRM-039, STRM-046

#### Scenario: The frame step
- **WHEN** a frame of a fake world with a chunk stage is ticked with a plan whose tiles read three chunks, one landed and one evicted
- **THEN** the chunk step is called once with the plan's tiles and the camera position, the landing and the eviction reach the managed loop, and a world without a stage does nothing

#### Scenario: The log
- **WHEN** the scene's log is written for a flight with and without a chunk stage
- **THEN** it says `chunk_field` true with the per-trial chunk fields in the first case, and false with the reason in the second
