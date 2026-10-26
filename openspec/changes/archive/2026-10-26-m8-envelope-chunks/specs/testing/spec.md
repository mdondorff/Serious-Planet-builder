## ADDED Requirements

### Requirement: TEST-054 The envelope scene runs the chunk stage
When its world has a stored coupled run, the envelope scene (TEST-047) SHALL give the world the A-net chunk stage of the viewer (cells of 8 m, its own worker pool, and its horizon jobs, REND-038), drive the chunk jobs every frame from the plan's tiles and pass what landed and what was evicted to the managed loop, start every trial with no chunk of an earlier trial resident and after the chunks of the start view are ready, and count the chunk stand-in share (STRM-039) in the judged share. Its log SHALL say whether the stage ran and give per trial the chunks landed, the median and the largest chunk job time and the jobs in flight at the end. One altitude (300 m) SHALL also run with the horizon jobs off and the log give that as a comparison line. A world without a stored run SHALL keep the tile-only scene and the log SHALL say why.

Verify: A
Status: active
Source: M8 exit criterion 7 (owner, 2026-10-23, D4), TEST-047, STRM-039, STRM-046

#### Scenario: The wait of a chunk
- **WHEN** a chunk is first wanted by a frame at 1.0 s and seen landing by a frame at 3.5 s, another is wanted at 1.0 s and again at 2.0 s and lands at 7.0 s, and a third lands that the flight never asked for
- **THEN** the waits are 2.5 s, 6 s and 0 s (median 2.5 s, largest 6 s), and a frame waits for the chunk stage while a plan node stands in or a chunk or horizon job is in flight

#### Scenario: The log
- **WHEN** the scene's log is written for a flight with and without a chunk stage
- **THEN** it says `chunk_field` true with the per-trial chunk fields in the first case, and false with the reason in the second
