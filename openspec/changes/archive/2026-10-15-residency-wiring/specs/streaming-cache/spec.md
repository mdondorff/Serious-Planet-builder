## MODIFIED Requirements

### Requirement: STRM-005 Cancellation and prioritisation
Requests SHALL be in three classes, highest first: the missing nodes of the draw set D (ancestors before descendants; within the class by screen-space error times visibility), the rest of the residency set R (coarse before fine, nearer before farther; the view direction may order within this class but not change its membership), and prefetch (R at a position predicted from the camera translation only; yaw and pitch rates are never extrapolated). Prefetch SHALL be requested only while no request of the other two classes is waiting or running, and only into free capacity, and SHALL never evict a tile of R or D. A queued or in-flight request SHALL be cancelled only when its tile is in none of D, R and the prefetch set (with their ancestors); leaving the view frustum is never a reason. When the prefetch set changes (the travel direction changed), its requests that are no longer in it are cancelled first. An in-flight job SHALL stop at its next stage boundary, and the tile of a cancelled job SHALL NOT become resident (a cancelled job is never revived: if its tile is needed again it is requested anew after the job ends). The order of requests SHALL be a pure function of the inputs, independent of the thread count (CON-14).

Verify: A
Status: active
Source: report §4, ADR 0031 points 2, 3 and 6, CON-04, CON-14

#### Scenario: Rotation requests nothing
- **WHEN** the camera turns through 360 degrees at a fixed position with R fully resident and fitting the capacity
- **THEN** nothing is requested and nothing is cancelled (while R is still filling, a rotation only reorders queued requests)

#### Scenario: Cancel
- **WHEN** a node leaves R before its job starts
- **THEN** the job is cancelled and never runs, and a node that only leaves the frustum is not cancelled

#### Scenario: Order
- **WHEN** D, the rest of R and a prefetch set are all missing
- **THEN** D is dispatched before the rest of R and that before prefetch, ancestors before descendants in D and coarse before fine in R

#### Scenario: Prefetch is translation only
- **WHEN** a prefetch position is predicted for a camera with a velocity and any yaw or pitch rate
- **THEN** it depends on the position, the velocity and the look-ahead only, and prefetch requests wait for free capacity and for the higher classes

#### Scenario: Travel change
- **WHEN** the prefetch set changes while prefetch requests are queued
- **THEN** the queued requests outside the new set are cancelled and the others stay

#### Scenario: Stage boundary
- **WHEN** an in-flight job is cancelled
- **THEN** it stops before its next stage and its tile does not become resident

#### Scenario: Rotation through the loop
- **WHEN** the managed frame loop (STRM-017) runs a 360 degree turn at a fixed position with R resident, plans and R coming from the worker and tiles from fake I/O
- **THEN** every frame of the turn dispatches, cancels, queues, uploads and evicts nothing, the worker reuses the previous R instead of selecting one, and each settled frame equals the synchronous plan of its view

#### Scenario: Order through the loop
- **WHEN** the camera jumps and the loop fills the new place
- **THEN** every request of the new draw set with its ancestors precedes the first request of the rest of R, ancestors precede descendants, and every frame draws something with no blank node

### Requirement: STRM-006 Upload and memory budgets
Terrain SHALL have its own share of the VRAM cap (2 GiB at the 8 GiB mid tier, 3 GiB at the 14 GiB high tier, configurable), a ceiling and not a target, counted in pooled fixed-size slots so that a test needs no device. The allocated terrain bytes (the blocks of the main pool, the small vertex pool for levels 0-2, index buffers, node records) SHALL be a pure function of the pool configuration and SHALL never exceed the share; a configuration whose fixed items exceed their reserve (0.125 GiB) or that holds no block is refused. A tile SHALL live in one slot of a pool with a free list; allocated, retiring and free slots SHALL always add up to the pool's capacity, and a slot that was given back SHALL NOT be reused until the frames in flight that may read it have retired. Resident tiles plus jobs in flight SHALL never exceed the pool's capacity in slots. When the share is smaller than the residency set R the manager SHALL degrade in this order and never touch the view first: tiles outside R and prefetch tiles are evicted first; then the leaves of R that are not in the draw set D, its ancestors or the roots are replaced by their ancestors (sibling groups merged), farthest first and, among equally far, finest first; only if D with its ancestors and the roots alone exceed the share (a budget failure, reported with the slot counts) are tiles of D replaced by their ancestors, farthest and finest first, as a declared exception to LOD-008's pixel guarantee. The result SHALL be a pure function of R, D and the capacity, and SHALL NOT depend on the order of the inputs. At the standard views the slots needed by R with its ancestors and by D with its ancestors SHALL stay within pinned counts and within pinned fractions of the share's slots.

The per-frame upload cap (ADR 0033 point 9) and the RAM budget (point 8) are proposed and not part of this requirement yet.

Verify: A, C
Status: active
Source: CON-20, CON-21, ADR 0031 point 4, ADR 0033 points 1-3, 10

#### Scenario: Terrain share
- **WHEN** the pools are configured for a tier
- **THEN** the allocated bytes (all blocks, the vertex pool, index buffers, node records) are at most the tier's terrain share, one more block would not fit, and the real GPU allocation equals the model for the blocks in use

#### Scenario: Slot accounting
- **WHEN** slots are acquired and released in any order over many frames
- **THEN** in use plus retiring plus free always equal the capacity, no slot is handed out twice, and a slot released at frame f is not acquired before frame f plus the frames in flight

#### Scenario: Cap
- **WHEN** residency demand exceeds the pool
- **THEN** resident tiles plus jobs in flight stay at or below the capacity in slots, tiles outside R and prefetch go first, then R tiles farthest and finest first for draw requests, and D with its ancestors and the roots are never evicted

#### Scenario: Small share
- **WHEN** the share is smaller than R
- **THEN** the draw set and the roots stay at full detail, the surroundings are replaced by ancestors (farthest and finest first), every direction is still covered by some resident level, and the slots in use never exceed the share

#### Scenario: Budget failure
- **WHEN** D with its ancestors and the roots exceed the share
- **THEN** the report says so with the slot counts, the farthest and finest draw tiles are asked for as their ancestors, and the roots stay

#### Scenario: Headroom
- **WHEN** the ten standard views are planned over the default world
- **THEN** R with ancestors is at most 10 % and the ground-1m D with ancestors at most 6 % of the mid tier's slots of the compact tile, and the vertex-format fractions are pinned at 40 % and 12 % of the mid tier (R at most 26 % of the high tier's slots, 10 % of its compact slots)

#### Scenario: Per frame through the loop
- **WHEN** the managed frame loop (STRM-017) turns through eight directions with a share much smaller than R
- **THEN** in every frame resident tiles and jobs in flight stay at or below the capacity, in use plus retiring plus free equal the capacity, no frame reports a budget failure, each view ends at full detail as the synchronous plan, and the frames report the tiles degraded and evicted

### Requirement: STRM-007 Resident capacity
The streamer SHALL count its capacity in pooled slots (STRM-006): a slot is reserved when a job is dispatched and given back when the tile is evicted, cancelled or failed, so resident tiles plus jobs in flight SHALL NEVER exceed the capacity. It SHALL evict only to make room for waiting requests, in this order: tiles in none of D, R and the prefetch set, and prefetch-only tiles, least recently used first; for draw requests only, then tiles of R that are not in D, farthest first and finest first. It SHALL NEVER evict a level-0 tile or a tile of the current draw set or one of its ancestors, SHALL NOT let a residency or prefetch request evict a tile of R, and SHALL retry failed tiles only after a configured delay measured on the injected clock. A tile that left R stays resident until the capacity needs its slot. A loop that mirrors the resident set into a renderer SHALL place each tile that becomes resident in the slot the streamer assigned to it and never choose a slot itself, SHALL forget each evicted tile before any other tile is written to its slot, and SHALL hold exactly the streamer's resident set.

Verify: A
Status: active
Source: CON-04, CON-20, ADR 0031 point 3, ADR 0033 point 3

#### Scenario: Interest moves away
- **WHEN** the wanted set moves to other tiles and the capacity is full
- **THEN** the oldest unused tiles are evicted to make room, roots and the draw set stay, the capacity is never exceeded, and a failed tile is not retried before the delay

#### Scenario: Order of eviction
- **WHEN** a draw request finds no free slot and the pool holds stale tiles, prefetch tiles and R tiles
- **THEN** the stale tile goes first, then the prefetch tile, then the farthest R tile, and the draw set and the roots are untouched

#### Scenario: No eviction by R
- **WHEN** a residency request finds no free slot and only R tiles could be evicted
- **THEN** it waits and nothing is evicted

#### Scenario: Reuse after retirement
- **WHEN** a tile is evicted for another
- **THEN** the other tile takes that slot only after the frames in flight have retired

#### Scenario: Placement follows the manager
- **WHEN** a loop mirrors the streamer's resident set into a renderer's slots over many frames of eviction and refill
- **THEN** after every frame the renderer holds exactly the streamer's resident tiles, each in the slot the streamer assigned, no slot holds two tiles, and a tile is written to a slot only after the streamer has evicted its previous tile

### Requirement: STRM-015 Residency set
The residency set R SHALL be a pure function of the camera position, the LOD parameters (LOD-008 distances and per-node height bounds) and the horizon cut. The view direction is not an input: yaw and pitch do not change it. R SHALL hold the selected leaves in all directions and all their ancestors, in ascending tile-id order, each with a distance, and SHALL contain the draw set (LOD-007) of every view from the same position. In a frame loop R SHALL be computed on the plan worker from the camera position only, after the plan of the same job has been delivered (no frame waits for it), on the plan's own selection cache, so that R evaluates no origin height and no mesh key and adds only the height volumes of the tiles the plan's frustum cut did not visit; a request with the same position and parameters as the previous R SHALL reuse that set without selecting anything.

Verify: A
Status: active
Source: ADR 0031 points 1 and 6, LOD-007, LOD-008, CON-03

#### Scenario: Yaw independent
- **WHEN** the draw set is selected from one position for many yaw angles
- **THEN** every draw-set tile is in R, and R is identical for all of them

#### Scenario: Closed under parents
- **WHEN** R is computed
- **THEN** every ancestor of every member is a member, in ascending tile-id order

#### Scenario: R on the worker shares the plan's cache
- **WHEN** a plan and then R are made from one position on one selection cache
- **THEN** R evaluates no origin height and no mesh key, adds the height volumes of the tiles outside the frustum, and a repeat of R or R for a turned view at the same position evaluates the field zero times and equals the first R

#### Scenario: Thread count and reuse
- **WHEN** the worker makes R for three requests (a position, a turn at that position, another position) on 1, 2 and 8 threads
- **THEN** every R equals the pure function of its position, the turn gets the very same set, and only the other position selects a new one

### Requirement: STRM-016 Frame pipelining
A frame loop SHALL never wait for the plan of its camera and never wait for the GPU of the frame it has just submitted. Plans SHALL be computed on a worker, one at a time, a request that arrives while one runs replacing any request that waits; a plan SHALL be a pure function of its request (camera, LOD parameters, frame setup, field), so the plan made for a camera is the same for any thread count, spawner and timing, and equals the synchronous plan of that camera; the origin heights of the nodes a plan cache has not seen SHALL be evaluated on worker threads, once per tile. The frames in flight SHALL be counted and SHALL NEVER exceed the bound `MAX_FRAMES_IN_FLIGHT` (a configuration constant, 2): at the bound the loop waits for the oldest frame, the only wait it knows, and counts it; a slot the renderer releases SHALL be reusable only that many frames later. Frame N SHALL draw the newest completed plan if it is usable for frame N's camera (same frame size, view, grid and lens, the camera moved at most a quarter of the split distance of the plan's finest level and turned at most 0.5 degrees), rebased to that camera, otherwise the cover. Every drawn node whose tile (with its mesh key) is not resident SHALL be drawn as its deepest resident ancestor, a coarser stand-in covering any finer drawn tile under it, and a node with no resident ancestor SHALL be counted blank (zero while the six roots are resident). The cover SHALL be a pure function of the camera, the LOD parameters and the resident set that evaluates no height field: from the resident roots, a tile is split while the camera is within its level's split distance of the tile's resident origin and every child in view is resident, otherwise the tile is drawn. A plan made for exactly the frame's request with every node resident SHALL be drawn unchanged, so once plans have settled the frame equals the synchronous frame. The wants of a frame loop SHALL be the nodes of the newest delivered plan (the draw set) and the newest delivered R, handed to the residency manager (STRM-017); a loop that has not yet received R asks for the draw set alone.

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

## ADDED Requirements

### Requirement: STRM-017 Managed frame loop
The frame loop SHALL be driven by the residency manager. Each iteration SHALL (1) hand the manager the wants (STRM-016): the draw set D of the newest delivered plan and the residency set R of the newest delivered worker result, prefetch empty until its source exists; (2) execute the changes of the resident set on the renderer: a tile that became resident is written to the slot the manager assigned (`upload_at`), an evicted tile is forgotten, and the renderer decides nothing about placement; (3) draw from the tiles the renderer holds. The manager's capacity SHALL be the renderer's pool, in slots (STRM-006), so that what the manager places the renderer can hold. The loop SHALL report, for every frame it draws, the counts of its iteration after the time columns (TEST-012): `resident`, `in_flight`, `queued`, `dispatched`, `cancelled`, `blocked`, `evicted`, `uploaded` and `degraded`. The one-shot streamed path (`--stream`) SHALL use the same manager with D from the plan and R of the camera position selected on the plan's cache, and SHALL draw the same meshes as the synchronous path. No golden, plan count or mesh version changes with the wiring: once the plans have settled, the image of the managed loop equals the synchronous frame.

Verify: A, C
Status: active
Source: ADR 0031 points 2, 3, 4 and 6, ADR 0032 point 5, ADR 0033 points 1-3, STRM-005, STRM-006, STRM-007, STRM-015, STRM-016, CON-03, CON-04

#### Scenario: Wants from the worker
- **WHEN** the plan and R of a position are delivered
- **THEN** the next iteration's wants are exactly that plan's nodes in plan order and that R's tiles with their distances, and a rotation that delivers only a new plan leaves the residency wants unchanged

#### Scenario: Per-frame counts
- **WHEN** the pipelined terrain scene runs on the software adapter with `--out-dir`
- **THEN** its per-frame log has the time columns followed by the nine count columns, the last frame shows resident tiles in slots and no blocked or degraded tile, and the run summary reports the slots resident of the pool

#### Scenario: Streamed equals synchronous
- **WHEN** the terrain scene runs with `--stream`
- **THEN** the image equals the synchronous scene's image bit for bit

#### Scenario: Pipelined equals synchronous
- **WHEN** the terrain scene runs through the managed loop from a cold start and after a jump
- **THEN** the loop settles on the synchronous frame of the last view
