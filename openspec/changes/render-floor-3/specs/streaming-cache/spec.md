## MODIFIED Requirements

### Requirement: STRM-001 Injected time, I/O and spawning
Streaming and scheduling code SHALL receive time, I/O and task spawning through injected interfaces, and tests SHALL be able to run it with a fake clock, fake I/O with injectable latency and failures, and a deterministic single-threaded spawner. The start-up load of the roots (the tiles at the render floor, LOD-011: six level-0 tiles at floor 0, 384 level-3 tiles at floor 3) SHALL hand all its jobs to the spawner before it waits for any, so a pool loads them concurrently; the tiles SHALL become resident in tile-id order (face, then position) once all have arrived, so the resident set, its slots and its change order do not depend on the spawner, the thread count or the order the jobs finish (CON-14); if one fails, the others SHALL be cancelled, nothing SHALL become resident, and the error SHALL name the failing tile.

Verify: A
Status: planned
Source: CON-04, CON-14, LOD-011, ADR 0062

#### Scenario: Fake clock
- **WHEN** a scheduler test advances a fake clock by 5 s
- **THEN** exactly the jobs due within 5 s run, in a reproducible order

#### Scenario: Concurrent roots
- **WHEN** the roots (six at floor 0, 384 at floor 3) are loaded on the manual spawner and on pools of 1, 2 and 8 threads
- **THEN** all jobs were handed over before the first wait, a pool as large as the root count has all loads in flight together, and the resident set is identical in every case

#### Scenario: A failing root
- **WHEN** one root fails to load
- **THEN** the other roots are told to stop, no root is resident, the slots are given back and the error names the failing tile and its cause

### Requirement: STRM-007 Resident capacity
The streamer SHALL count its capacity in pooled slots (STRM-006): a slot is reserved when a job is dispatched and given back when the tile is evicted, cancelled or failed, so resident tiles plus jobs in flight SHALL NEVER exceed the capacity. It SHALL evict only to make room for waiting requests, in this order: tiles in none of D, R and the prefetch set, and prefetch-only tiles, least recently used first; for draw requests only, then tiles of R that are not in D, farthest first and finest first. It SHALL NEVER evict a root (a tile at the render floor level, LOD-011) or a tile of the current draw set or one of its ancestors, SHALL NOT let a residency or prefetch request evict a tile of R, and SHALL retry failed tiles only after a configured delay measured on the injected clock. A tile that left R stays resident until the capacity needs its slot. A loop that mirrors the resident set into a renderer SHALL place each tile that becomes resident in the slot the streamer assigned to it and never choose a slot itself, SHALL forget each evicted tile before any other tile is written to its slot, and SHALL hold exactly the streamer's resident set; the one exception is a CON-13 fallback tile of the compact renderer, which keeps the main slot the streamer reserved empty and lives in the renderer's own fallback region of the vertex pool (a fallback beyond the region is refused with the tile id).

Verify: A
Status: planned
Source: CON-04, CON-20, ADR 0031 point 3, ADR 0033 point 3, LOD-011, ADR 0062

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
The residency set R SHALL be a pure function of the camera position, the LOD parameters (LOD-008 distances and per-node height bounds) and the horizon cut. The view direction is not an input: yaw and pitch do not change it. R SHALL hold the selected leaves in all directions and all their ancestors down to the render floor (a tile above the floor is never a member, LOD-011), in ascending tile-id order, each with a distance, and SHALL contain the draw set (LOD-007) of every view from the same position. In a frame loop R SHALL be computed on the plan worker from the camera position only, after the plan of the same job has been delivered (no frame waits for it), on the plan's own selection cache, so that R evaluates no origin height and no mesh key and adds only the height volumes of the tiles the plan's frustum cut did not visit; a request with the same position and parameters as the previous R SHALL reuse that set without selecting anything.

Verify: A
Status: planned
Source: ADR 0031 points 1 and 6, LOD-007, LOD-008, CON-03, LOD-011, ADR 0062

#### Scenario: Yaw independent
- **WHEN** the draw set is selected from one position for many yaw angles
- **THEN** every draw-set tile is in R, and R is identical for all of them

#### Scenario: Closed under parents
- **WHEN** R is computed
- **THEN** every ancestor of every member that is not above the render floor is a member, in ascending tile-id order, and no member is above the floor

#### Scenario: R on the worker shares the plan's cache
- **WHEN** a plan and then R are made from one position on one selection cache
- **THEN** R evaluates no origin height and no mesh key, adds the height volumes of the tiles outside the frustum, and a repeat of R or R for a turned view at the same position evaluates the field zero times and equals the first R

#### Scenario: Thread count and reuse
- **WHEN** the worker makes R for three requests (a position, a turn at that position, another position) on 1, 2 and 8 threads
- **THEN** every R equals the pure function of its position, the turn gets the very same set, and only the other position selects a new one

### Requirement: STRM-016 Frame pipelining
A frame loop SHALL never wait for the plan of its camera and never wait for the GPU of the frame it has just submitted. Plans SHALL be computed on a worker, one at a time, a request that arrives while one runs replacing any request that waits; a plan SHALL be a pure function of its request (camera, LOD parameters, frame setup, field), so the plan made for a camera is the same for any thread count, spawner and timing, and equals the synchronous plan of that camera; the origin heights of the nodes a plan cache has not seen SHALL be evaluated on worker threads, once per tile. The frames in flight SHALL be counted and SHALL NEVER exceed the bound `MAX_FRAMES_IN_FLIGHT` (a configuration constant, 2): at the bound the loop waits for the oldest frame, the only wait it knows, and counts it; a slot the renderer releases SHALL be reusable only that many frames later. Frame N SHALL draw the newest completed plan if it is usable for frame N's camera (same frame size, view, grid and lens, the camera moved at most a quarter of the split distance of the plan's finest level and turned at most 0.5 degrees), rebased to that camera and covers the frame's view, otherwise the cover. A plan made for another camera covers the frame's view when, for every tile of the frame's own cover, it draws that tile or finer tiles filling every child of it that is in the frame's view (recursively), and none of its nodes is blank; a coarser ancestor does not count (the cover can show more detail), and a tile it does not draw at all is a sliver that turned into view (`plan_covers_view`, a pure function of the camera, the cover, the drawn set and the cached tile bounds); a plan made for exactly the frame's request needs no test. Every drawn node whose tile (with its mesh key) is not resident SHALL be drawn as its deepest resident ancestor, a coarser stand-in covering any finer drawn tile under it, and a node with no resident ancestor SHALL be counted blank (zero while the roots, the tiles at the render floor of LOD-011, are resident; a floor tile that has not arrived is blank, there is no coarser stand-in). The cover SHALL be the plan's own selection with "resident" as the condition for descending, a pure function of the camera, the LOD parameters, the resident set and the cached bounds of the tiles of the newest R (each tile's geometry and height-bounds volume as the plan computed them; the frame path evaluates no height field): from the resident roots (floor tiles), a tile is split while the plan's LOD test says so (the distance to its volume below the level's split distance) and every child in view is resident, otherwise the tile is drawn; in view means what the plan means (not beyond the horizon, not outside the frustum, not outside the tight cut of LOD-007), and a child that the horizon, the frustum or the cut culls never stops its parent's descent, nor does a tile that is not in R when the bounds are those of R for exactly the camera's position and parameters (the plan never selects it); the leaves are then balanced as the plan's are (2:1) where the tiles of the split are resident. With every tile of R resident the cover of a camera SHALL equal the plan of that camera, node for node and level for level, whatever the camera's direction; with tiles missing it is the deepest resident tiles and never blank, and always a subset of the resident tiles. A plan made for exactly the frame's request with every node resident SHALL be drawn unchanged, so once plans have settled the frame equals the synchronous frame. The wants of a frame loop SHALL be the nodes of the newest delivered plan (the draw set) and the newest delivered R, handed to the residency manager (STRM-017); a loop that has not yet received R asks for the draw set alone.

Verify: A
Status: planned
Source: ADR 0032 point 5, ADR 0031 point 3, optimization review item A, STRM-002, LOD-007, CON-03, CON-14, CON-22, LOD-011, ADR 0062

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

#### Scenario: The cover is the plan with R resident
- **WHEN** every tile of R of a position is resident and the cover is made for the ten standard views and for a rotation script (0.2 to 15 degrees a frame, pitch changes) at that position
- **THEN** the cover equals the plan of the same camera, node for node and level for level

#### Scenario: Partial residency
- **WHEN** a third of R's leaves (with their ancestors) is resident, with or without cached bounds
- **THEN** the cover holds resident tiles only, no tile under another, and every node of the plan has a cover tile at it, above it or below it

#### Scenario: A plan that does not cover the frame
- **WHEN** a frame's camera is turned away from the plan's camera so that the plan does not cover its view, and again by small steps
- **THEN** the frame draws the cover (the plan of its own camera) where the plan does not cover the view and the plan where it does, exactly as `plan_covers_view` says, and no node of the frame's own plan is drawn coarser than the plan draws it or missing

#### Scenario: Cold start at the floor
- **WHEN** the orbit view is drawn from a cold start without a pre-fill, and then with the pre-fill of STRM-021
- **THEN** the start-up load of STRM-001 makes all 384 floor tiles resident before frame 0 (from the pack with the pre-fill, generated without it, a visible cold-start cost that is reported), so no frame is blank in either case

### Requirement: STRM-021 Pre-filled coarse tiles
The bake step SHALL also write the finished render tiles of levels `F..=N` (F the render floor, LOD-011, default 3, `--floor-level N` on `generate --prefill-dir` too; default `N = 4`, 1,920 tiles; `--prefill-levels N` names N, 0 disables, an N below F writes nothing and says so; at most 8) into the tile pack of the bake, as records of kind 2 (`Kind::Mesh`) with the existing record format and the ADR 0018 mesh key of the tile (`mesh_key_inputs`: field, tile, cells, skirt fraction, `MESH_VERSION` and the climate version word) plus one output parameter naming the form of the stored tile (vertex or compact; a tile below the floor is never stored, and the vertex form finds no record in the pack), so that no existing key, record format or version changes. The payload SHALL be the tile exactly as the generators build it, every `f32`, `u16` and `f64` bit (`codec version | form | cells | raw length | LZ4 block`), and SHALL decode to a tile equal to the generated one (payload version 3 since MESH_VERSION 8: the compact form is the slot of GEN-025 and the vertex form is unchanged; a version-1 or version-2 payload is refused naming the version, and records written under MESH_VERSION 7 are unreachable because `MESH_VERSION` is part of the key, so a pack pre-filled under version 7 yields only misses, `missing_tiles` lists every tile and the pre-fill writes the tiles again; the stale records stay in the pack until compaction, ADR 0029); a payload that is short, of another version or form, of cells that are not a power of two from 2 to 1024, of a wrong raw length or damaged SHALL be refused naming what is wrong, and the decoder SHALL NOT allocate more than the cells allow. A pre-filled tile SHALL equal the tile the streamer generates on demand for the same field, cells, finest level and form, bit for bit, whatever the number of threads that wrote it (CON-14): tiles are generated on any number of threads, coarsest first, and appended in the fixed order level, face, row, column, so the bytes of the pack do not depend on the thread count. A tile at or below the run's finest level SHALL NOT be stored or looked up (its format depends on its relief, CON-13), and a pre-fill that reaches it, or goes deeper than 8, SHALL be refused before it writes. The pre-fill SHALL be idempotent: a tile the pack already holds under the current key whose payload decodes is skipped, a run that finds none missing opens no writer and changes no byte or file time, a deeper run adds only the new levels, and a record that does not decode counts as missing and is written again (the newer record wins). `generate --global-dir D` SHALL pre-fill after the bake and print the tiles, payload bytes, growth of the packs and wall time; `generate --prefill-dir D --global-res N [--prefill-levels L] [--threads T] [--cells C] [--finest-level F] [--tile-format F]` SHALL pre-fill an existing bake in place and refuse a store that holds no bake of that definition and size. The streamer's tile source (`MeshSource`) SHALL take a tile of level 8 or coarser from the pack when the pack holds it under the current key and generate it otherwise (a cache degrades to a miss: a damaged record, another version, another field, other cells or the other form is a generation, never an error), and a world opened `cold` SHALL never read a tile from the pack (the cold reader of `dive-cold`, TEST-019). Pre-filled tiles are caches evicted with their bake (ADR 0045 point 1) and are not read by `verify-cache` or `verify-global`, which report them as records of another kind.

Verify: A
Status: planned
Source: owner (2026-10-16, option A), ADR 0045 point 3, ADR 0018, ADR 0019, STRM-008, STRM-003, CON-05, CON-13, CON-14, M4 fix queue (F9), LOD-011, GEN-025, ADR 0062

#### Scenario: Codec
- **WHEN** a vertex tile (level 2) and a compact tile (level 4) are encoded and decoded, and hostile payloads are decoded
- **THEN** every bit of the tile comes back, and a short payload, another version or form, bad or huge cells, a wrong raw length and a damaged block are each refused naming the cause without allocating by the claim

#### Scenario: Keys
- **WHEN** the key of a tile is made for another field, other cells, the other form or another tile
- **THEN** it differs, while levels 0-2 give the same key and the same tile in both forms

#### Scenario: Idempotent and thread-independent
- **WHEN** levels 3-4 are pre-filled on one thread and on five, the pre-fill is run again, and a deeper one is run
- **THEN** the pack files of the two runs are byte-identical, every stored tile equals the generated one, the second run generates and writes nothing and touches no file, and the deeper run adds only the new levels

#### Scenario: Limits and misses
- **WHEN** a pre-fill reaches the run's finest level or goes deeper than 8, and a store pre-filled for one field is read for another seed, other cells or a finest tile
- **THEN** the first is refused before writing anything and the others are misses

#### Scenario: Damaged record
- **WHEN** a record under a tile's key holds a payload that does not decode
- **THEN** the tile counts as missing, a repeated pre-fill writes it again and the newer record is the one read

#### Scenario: The streamer reads the pack
- **WHEN** a bake of an eroded-field world is pre-filled to level 4 through `generate`, and the streamer's tile source loads tiles of levels 3-4 and a level-5 tile
- **THEN** the level 3-4 tiles are taken from the pack and equal the generated ones (also through the staged, cancellable path), the level-5 tile is generated, a cold reader of the same store reads none from the pack and builds the same tiles, and the vertex form finds none

#### Scenario: The commands
- **WHEN** `generate --global-dir` bakes with `--prefill-levels 3`, `--prefill-dir` pre-fills an existing bake to level 3, then 4, then 4 again, and the options are misused
- **THEN** the reports count 384 then 1,920 tiles with 384, 1,536 and 0 written, the last run grows the packs by 0 bytes, `--prefill-levels 0` is disabled, and a missing `--global-res`, a store of another size, a depth over 8, a finest level inside the pre-fill and a clash with another command are each refused naming the cause

#### Scenario: A pack pre-filled under an older mesh version
- **WHEN** a pack holds records written under MESH_VERSION 7 keys with version-2 payloads and the tiles of levels 3 to 4 are looked up and pre-filled
- **THEN** every lookup is a miss, a version-2 payload is refused even under the current key, the pre-fill writes every tile again and the new records decode to the generated tiles

#### Scenario: Floor and levels
- **WHEN** a pre-fill with floor 3 and N = 4 runs, and the same with floor 0
- **THEN** the pack holds 1,920 tiles with floor 3 and 2,046 with floor 0, none of levels 0-2 with floor 3, and every stored tile equals the generated one
