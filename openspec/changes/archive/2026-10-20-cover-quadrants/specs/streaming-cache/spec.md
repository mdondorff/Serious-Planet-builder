## MODIFIED Requirements

### Requirement: STRM-016 Frame pipelining
A frame loop SHALL never wait for the plan of its camera and never wait for the GPU of the frame it has just submitted. Plans SHALL be computed on a worker, one at a time, a request that arrives while one runs replacing any request that waits; a plan SHALL be a pure function of its request (camera, LOD parameters, frame setup, field), so the plan made for a camera is the same for any thread count, spawner and timing, and equals the synchronous plan of that camera; the origin heights of the nodes a plan cache has not seen SHALL be evaluated on worker threads, once per tile. The frames in flight SHALL be counted and SHALL NEVER exceed the bound `MAX_FRAMES_IN_FLIGHT` (a configuration constant, 2): at the bound the loop waits for the oldest frame, the only wait it knows, and counts it; a slot the renderer releases SHALL be reusable only that many frames later. Frame N SHALL draw the newest completed plan if it is usable for frame N's camera (same frame size, view, grid and lens, the camera moved at most a quarter of the split distance of the plan's finest level and turned at most 0.5 degrees), rebased to that camera and covers the frame's view, otherwise the cover. A plan made for another camera covers the frame's view when, for every tile of the frame's own cover, it draws that tile or finer tiles filling every child of it that is in the frame's view (recursively), and none of its nodes is blank; a coarser ancestor does not count (the cover can show more detail), and a tile it does not draw at all is a sliver that turned into view (`plan_covers_view`, a pure function of the camera, the cover, the drawn set and the cached tile bounds); a plan made for exactly the frame's request needs no test. Every drawn node whose tile (with its mesh key) is not resident SHALL be drawn by its deepest resident ancestor as a stand-in over the quadrant of the ancestor that holds the node, and a resident tile under a quadrant that a stand-in draws is not drawn (no part is drawn twice) while the resident tiles beside it are, so a tile that has not streamed in does not take its resident siblings back to their parent; a node with no resident ancestor SHALL be counted blank (zero while the roots, the tiles at the render floor of LOD-011, are resident; a floor tile that has not arrived is blank, there is no coarser stand-in). A node SHALL carry the quadrants of its tile that it draws, as a mask of four bits (bit k for child k of the tile's children, `0b1111` the whole tile, the default); the fragment stage SHALL discard the fragments of a terrain or water node whose tile coordinates lie in a quadrant the mask leaves out, after the derivatives of the fragment, and a node drawn whole SHALL give the same frame as before the mask existed, drawn by pipelines whose fragment stage holds no `discard` for the mask (the nodes drawn over some quadrants go in their own draws, after the whole ones, through pipelines made with the cut on); the text form of a plan SHALL carry the mask only when it is not the whole tile. The cover SHALL be the plan's own selection with "resident" as the condition for descending, a pure function of the camera, the LOD parameters, the resident set and the cached bounds of the tiles of the newest R (each tile's geometry and height-bounds volume as the plan computed them; the frame path evaluates no height field): from the resident roots (floor tiles), a tile is split while the plan's LOD test says so (the distance to its volume below the level's split distance) and at least one child in view is resident, otherwise the tile is drawn whole; the resident children in view go on down the walk and the tile stands in over the quadrants of the children in view that are not resident; in view means what the plan means (not beyond the horizon, not outside the frustum, not outside the tight cut of LOD-007), and a child that the horizon, the frustum or the cut culls never stops its parent's descent, nor does a tile that is not in R when the bounds are those of R for exactly the camera's position and parameters (the plan never selects it); the leaves, the missing children among them, are then balanced as the plan's are (2:1), and a leaf that is not resident is drawn by its nearest resident ancestor over the quadrant that holds it; the tiles of the cover are the stand-in parent and its resident children side by side, and `plan_covers_view` counts a tile drawn over some quadrants as its drawn children. Known limits, until the missing tile has streamed in: a hairline crack can open along the cut between a stand-in's quadrant and a finer resident sibling (the stand-in has no skirt there), and the level difference across the cut is not held to 2:1 where the missing child lay beside finer tiles. With every tile of R resident the cover of a camera SHALL equal the plan of that camera, node for node and level for level, whatever the camera's direction, every tile drawn whole; with tiles missing it is the deepest resident tiles and never blank, and always a subset of the resident tiles. A plan made for exactly the frame's request with every node resident SHALL be drawn unchanged, so once plans have settled the frame equals the synchronous frame. The wants of a frame loop SHALL be the nodes of the newest delivered plan (the draw set) and the newest delivered R, handed to the residency manager (STRM-017); a loop that has not yet received R asks for the draw set alone.

Verify: A, C
Status: active
Source: ADR 0032 point 5, ADR 0031 point 3, optimization review item A, STRM-002, LOD-007, CON-03, CON-14, CON-22, LOD-011, ADR 0062, SPIKE-18 (2026-10-20)

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
- **THEN** each such node is drawn by its deepest resident ancestor over the quadrant that holds it, no part is drawn twice (a tile lies under another only beside the quadrants the other draws), the tiles are in ascending id order, and origins and matrix follow the frame's camera

#### Scenario: Cover
- **WHEN** only the roots are resident, or part of a plan's tiles with their ancestors
- **THEN** the cover holds resident tiles only, none under a quadrant another draws, serves every node of the full plan from above or below, and is the same for the same inputs

#### Scenario: A missing tile keeps its siblings
- **WHEN** the cover is made with R resident except one leaf in view and what is under it
- **THEN** its resident siblings are drawn at their level or finer, their parent is drawn over the missing leaf's quadrant only, and with R complete every tile is drawn whole

#### Scenario: The quadrants on the screen
- **WHEN** a node is drawn over one quadrant, over each of the four and over two
- **THEN** the pixels of the four single quadrants partition those of the whole node, each covers the centre of its own quadrant (child k = 2 times the x half plus the y half) and no other, and two quadrants cover the union of theirs

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

#### Scenario: The cover is the plan with R resident
- **WHEN** every tile of R of a position is resident and the cover is made for the ten standard views and for a rotation script (0.2 to 15 degrees a frame, pitch changes) at that position
- **THEN** the cover equals the plan of the same camera, node for node and level for level

#### Scenario: Partial residency
- **WHEN** a third of R's leaves (with their ancestors) is resident, with or without cached bounds
- **THEN** the cover holds resident tiles only, no tile under a quadrant another draws, and every node of the plan has a cover tile at it, above it or below it

#### Scenario: A plan that does not cover the frame
- **WHEN** a frame's camera is turned away from the plan's camera so that the plan does not cover its view, and again by small steps
- **THEN** the frame draws the cover (the plan of its own camera) where the plan does not cover the view and the plan where it does, exactly as `plan_covers_view` says, and no node of the frame's own plan is drawn coarser than the plan draws it or missing

#### Scenario: Cold start at the floor
- **WHEN** the orbit view is drawn from a cold start without a pre-fill, and then with the pre-fill of STRM-021
- **THEN** the start-up load of STRM-001 makes all 384 floor tiles resident before frame 0 (from the pack with the pre-fill, generated without it, a visible cold-start cost that is reported), so no frame is blank in either case
