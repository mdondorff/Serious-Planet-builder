## MODIFIED Requirements

### Requirement: STRM-002 Never block a frame
For every visible node some level of detail SHALL be available: missing tiles SHALL be requested parent-first, and a frame SHALL never wait for generation, I/O or a plan (STRM-016).

Verify: A
Status: active
Source: CON-22, ADR 0032 point 5

#### Scenario: Fast dive
- **WHEN** a scripted orbit-to-ground dive runs with fake I/O of 200 ms latency
- **THEN** every frame has a resident tile for every visible node

#### Scenario: Teleport
- **WHEN** the camera jumps to the other side of the planet while the plan of the new place is still being computed
- **THEN** the frame after the jump draws a cover plan from the resident tiles (never blank, no height field evaluated by the frame), and the frame never waits for the plan

#### Scenario: Mouse look
- **WHEN** a seeded script of yaw and pitch steps with reversals and a 180 degree turn runs at a fixed position
- **THEN** every node of each frame's own plan is served by a drawn tile of its own, an ancestor or its descendants

## ADDED Requirements

### Requirement: STRM-016 Frame pipelining
A frame loop SHALL never wait for the plan of its camera and never wait for the GPU of the frame it has just submitted. Plans SHALL be computed on a worker, one at a time, a request that arrives while one runs replacing any request that waits; a plan SHALL be a pure function of its request (camera, LOD parameters, frame setup, field), so the plan made for a camera is the same for any thread count, spawner and timing, and equals the synchronous plan of that camera; the origin heights of the nodes a plan cache has not seen SHALL be evaluated on worker threads, once per tile. The frames in flight SHALL be counted and SHALL NEVER exceed the bound `MAX_FRAMES_IN_FLIGHT` (a configuration constant, 2): at the bound the loop waits for the oldest frame, the only wait it knows, and counts it; a slot the renderer releases SHALL be reusable only that many frames later. Frame N SHALL draw the newest completed plan if it is usable for frame N's camera (same frame size, view, grid and lens, the camera moved at most a quarter of the split distance of the plan's finest level and turned at most 0.5 degrees), rebased to that camera, otherwise the cover. Every drawn node whose tile (with its mesh key) is not resident SHALL be drawn as its deepest resident ancestor, a coarser stand-in covering any finer drawn tile under it, and a node with no resident ancestor SHALL be counted blank (zero while the six roots are resident). The cover SHALL be a pure function of the camera, the LOD parameters and the resident set that evaluates no height field: from the resident roots, a tile is split while the camera is within its level's split distance of the tile's resident origin and every child in view is resident, otherwise the tile is drawn. A plan made for exactly the frame's request with every node resident SHALL be drawn unchanged, so once plans have settled the frame equals the synchronous frame.

Verify: A
Status: active
Source: ADR 0032 point 5, optimization review item A, STRM-002, CON-03, CON-14, CON-22

#### Scenario: Same plans as the synchronous loop
- **WHEN** a fixed camera script runs with a worker that finishes one plan between frames
- **THEN** every plan delivered equals the synchronous plan of the camera it was requested for, in request order, and with the camera still the frame is the synchronous plan

#### Scenario: Thread count
- **WHEN** the same cameras are planned on 1, 2 and 8 worker threads
- **THEN** the plan hashes are equal, and equal to the synchronous plans

#### Scenario: Latest request wins
- **WHEN** four requests arrive while the first plan runs
- **THEN** the first and the last are planned, the two between are counted as superseded

#### Scenario: Frames in flight
- **WHEN** a loop submits frames to a GPU that finishes a frame every third submission
- **THEN** frames in flight never exceed the bound, the peak reaches it, the waits are counted, and a GPU that keeps up never makes the loop wait

#### Scenario: Stand-ins
- **WHEN** some tiles of a plan are not resident
- **THEN** each such node is drawn as its deepest resident ancestor, no drawn tile lies under another, the tiles are in ascending id order, and origins and matrix follow the frame's camera

#### Scenario: Cover
- **WHEN** only the roots are resident, or part of a plan's tiles with their ancestors
- **THEN** the cover holds resident tiles only, none under another, serves every node of the full plan from above or below, and is the same for the same inputs

#### Scenario: Jump
- **WHEN** the camera jumps with the plan of the new place still on the worker
- **THEN** frame 0 draws a cover without evaluating the field, the worker's plan arrives later and is drawn with stand-ins, and once its tiles are resident the frame is the synchronous plan

#### Scenario: Stale plan
- **WHEN** the camera moves by 0.9 or 1.1 times the limit, or turns by 0.4 or 0.6 degrees, or the frame size or view differs
- **THEN** the plan is usable below the limit and unusable above it, and for a different size or view

#### Scenario: Origin heights on workers
- **WHEN** a cold plan is made with four threads
- **THEN** its origin heights are evaluated on more than one thread, once per node, a warm plan evaluates none, and the plan equals the single-threaded one

#### Scenario: Pipelined image
- **WHEN** the terrain scene runs through the pipelined loop from a cold start, and after a jump to another view
- **THEN** the first frame is a cover and the image the loop settles on equals the synchronous path's
