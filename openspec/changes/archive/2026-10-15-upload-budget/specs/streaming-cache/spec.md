## MODIFIED Requirements

### Requirement: STRM-006 Upload and memory budgets
Terrain SHALL have its own share of the VRAM cap (2 GiB at the 8 GiB mid tier, 3 GiB at the 14 GiB high tier, configurable), a ceiling and not a target, counted in pooled fixed-size slots so that a test needs no device. The allocated terrain bytes (the blocks of the main pool, the small vertex pool for levels 0-2, index buffers, node records) SHALL be a pure function of the pool configuration and SHALL never exceed the share; a configuration whose fixed items exceed their reserve (0.125 GiB) or that holds no block is refused. A tile SHALL live in one slot of a pool with a free list; allocated, retiring and free slots SHALL always add up to the pool's capacity, and a slot that was given back SHALL NOT be reused until the frames in flight that may read it have retired. Resident tiles plus jobs in flight SHALL never exceed the pool's capacity in slots. When the share is smaller than the residency set R the manager SHALL degrade in this order and never touch the view first: tiles outside R and prefetch tiles are evicted first; then the leaves of R that are not in the draw set D, its ancestors or the roots are replaced by their ancestors (sibling groups merged), farthest first and, among equally far, finest first; only if D with its ancestors and the roots alone exceed the share (a budget failure, reported with the slot counts) are tiles of D replaced by their ancestors, farthest and finest first, as a declared exception to LOD-008's pixel guarantee. The result SHALL be a pure function of R, D and the capacity, and SHALL NOT depend on the order of the inputs. At the standard views the slots needed by R with its ancestors and by D with its ancestors SHALL stay within pinned counts and within pinned fractions of the share's slots.

The per-frame upload cap (ADR 0033 point 9) is a byte count of **tile data** written to the pool in one frame, counted per tile as the size of its upload in the sink's format (the vertex tile today, the compact tile's texels and record later; the cap is a configuration value, `UploadBudget::tile_bytes_per_frame`, starting at 1 MiB, about 118 compact slots or 19 vertex slots at cells 32). Finished tiles the pool has not received SHALL wait in the order draw set D with its ancestors and the roots (ancestors before descendants, then plan order), then R with its ancestors, then the prefetch set, then the rest; a frame SHALL write the longest prefix of that order whose bytes fit the cap, SHALL stop at the first tile that does not fit (a tile is deferred to the next frame, never dropped, never overtaken by a later tile of any class), and SHALL always write at least one tile when any waits, so a tile larger than the cap is written alone and cannot starve; the cap is exceeded only by that one tile. A tile that is evicted while it waits is forgotten. The tiles that wait are not drawn: the frame draws their deepest resident ancestors or the cover (STRM-016), so no frame is blank for the deferral. The order and what is written are a pure function of the wants and the finished set, not of the thread count or the order the jobs finished (CON-14). The per-frame node records (the indirect path's `indirect_frame_bytes`) are a CPU write into a ring, cannot be deferred and are not tile data: they are reported as their own line (`node_bytes`) and count against CON-21's 2 ms (owner's decision, ADR 0033), outside the tile cap unless the configuration puts them inside (`node_records_in_cap`: the tile allowance is the cap minus the node bytes of the previous drawn plan). **The cap value is provisional**: ADR 0033 point 9 stays proposed until a perf window measures what it costs against the 2 ms, and changing it is a configuration change. The RAM budget (point 8) is proposed and not part of this requirement yet.

Verify: A, C
Status: active
Source: CON-14, CON-20, CON-21, ADR 0031 point 4, ADR 0033 points 1-3, 9 (proposed, value provisional) and 10, ADR 0032, STRM-002, STRM-016, STRM-017

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

#### Scenario: Upload cap
- **WHEN** the managed frame loop (STRM-017) runs the dive script and a teleport on fakes with tiles of the vertex format's size and the default cap, tiles finishing faster than the cap lets them in
- **THEN** in every frame the bytes written are at most the cap or exactly one tile, a frame that defers is full (the next tile did not fit), the tiles written are in class order with ancestors first and nothing that waits ranks before the last tile written, the streamer's resident tiles are the sink's plus the waiting ones, and no frame is blank

#### Scenario: Larger than the cap
- **WHEN** every tile is larger than the cap
- **THEN** one tile is written per frame while any waits, the roots first, and the view is never blank once the six roots are in

#### Scenario: Upload order on any thread count
- **WHEN** the same script runs with tile jobs finishing on 1, 2 and 8 threads
- **THEN** the tiles written, their bytes and the deferred counts of every frame are identical

#### Scenario: Fill time at the cap
- **WHEN** the 8,763 slots of a full ground-1m fill wait at the default cap
- **THEN** it takes 75 frames of 118 compact tiles (8,840 B each), or 462 frames of 19 vertex tiles (53,724 B each)

#### Scenario: Node records
- **WHEN** a frame is drawn
- **THEN** its node-record bytes are reported apart from the tile bytes, and shrink the tile allowance only when the configuration counts them inside the cap

### Requirement: STRM-017 Managed frame loop
The frame loop SHALL be driven by the residency manager. Each iteration SHALL (1) hand the manager the wants (STRM-016): the draw set D of the newest delivered plan and the residency set R of the newest delivered worker result, and the prefetch set of the newest delivered worker result capped by the free share (STRM-018; empty when the loop has no prefetch source or the camera is not travelling); (2) execute the changes of the resident set on the renderer: a tile that became resident is written to the slot the manager assigned (`upload_at`) within the per-frame upload budget (STRM-006: the finished tiles wait in class order and the rest stay finished-but-not-placed for a later frame), an evicted tile is forgotten, and the renderer decides nothing about placement; (3) draw from the tiles the renderer holds. The manager's capacity SHALL be the renderer's pool, in slots (STRM-006), so that what the manager places the renderer can hold. The loop SHALL report, for every frame it draws, the counts of its iteration after the time columns (TEST-012): `resident`, `in_flight`, `queued`, `dispatched`, `cancelled`, `blocked`, `evicted`, `uploaded`, `degraded`, `upload_bytes` (tile bytes written), `deferred` (finished tiles waiting for the budget) and `node_bytes` (the frame's node-record bytes, STRM-006). The prefetch counts of a frame (wants after the cap, prefetch requests cancelled) are carried in the frame's counts but are not log columns. The one-shot streamed path (`--stream`) SHALL use the same manager with D from the plan and R of the camera position selected on the plan's cache, and SHALL draw the same meshes as the synchronous path; it has one camera and no travel, so its prefetch set is empty (STRM-018). No golden, plan count or mesh version changes with the wiring: once the plans have settled, the image of the managed loop equals the synchronous frame.

Verify: A, C
Status: active
Source: ADR 0031 points 2, 3, 4 and 6, ADR 0032 point 5, ADR 0033 points 1-3 and 9, STRM-005, STRM-006, STRM-007, STRM-015, STRM-016, CON-03, CON-04

#### Scenario: Wants from the worker
- **WHEN** the plan and R of a position are delivered
- **THEN** the next iteration's wants are exactly that plan's nodes in plan order and that R's tiles with their distances, and a rotation that delivers only a new plan leaves the residency wants unchanged

#### Scenario: Per-frame counts
- **WHEN** the pipelined terrain scene runs on the software adapter with `--out-dir`
- **THEN** its per-frame log has the time columns followed by the twelve count columns (the nine of the scheduler, then `upload_bytes deferred node_bytes`), the last frame shows resident tiles in slots and no blocked or degraded tile, and the run summary reports the slots resident of the pool

#### Scenario: Streamed equals synchronous
- **WHEN** the terrain scene runs with `--stream`
- **THEN** the image equals the synchronous scene's image bit for bit

#### Scenario: Pipelined equals synchronous
- **WHEN** the terrain scene runs through the managed loop from a cold start and after a jump
- **THEN** the loop settles on the synchronous frame of the last view

