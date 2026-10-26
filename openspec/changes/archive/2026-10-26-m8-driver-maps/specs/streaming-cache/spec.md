## MODIFIED Requirements

### Requirement: STRM-001 Injected time, I/O and spawning
Streaming and scheduling code SHALL receive time, I/O and task spawning through injected interfaces, and tests SHALL be able to run it with a fake clock, fake I/O with injectable latency and failures, and a deterministic single-threaded spawner. The start-up load of the roots (the tiles at the render floor, LOD-011: six level-0 tiles at floor 0, 384 level-3 tiles at floor 3) SHALL hand all its jobs to the spawner before it waits for any, so a pool loads them concurrently; the tiles SHALL become resident in tile-id order (face, then position) once all have arrived, so the resident set, its slots and its change order do not depend on the spawner, the thread count or the order the jobs finish (CON-14); the load of the roots is exempt from the per-frame upload cap (STRM-006, a declared exception, ADR 0064 amending ADR 0033 point 9: 384 slots are 3.4 MiB, three and a half frames at the cap, and a floor tile has no coarser stand-in), so all roots are written in the first frame, before it is drawn, as a load step of the body, not a steady frame (the same reading as the upload of a body's static tables); the roots stay resident for the body's life (STRM-007 never evicts them), so a teleport never reloads them, and the exemption is that of the tiles at the render floor level, so in practice that load only: every other tile counts against the cap, and the cap and the one-tile rule govern the tiles behind the roots in the first frame too; the readiness report (ADR 0045 point 5) SHALL show the roots loaded so far in the window title while they load; if one fails, the others SHALL be cancelled, nothing SHALL become resident, and the error SHALL name the failing tile.

Verify: A
Status: active
Source: CON-04, CON-14, LOD-011, ADR 0045 point 5, ADR 0062, ADR 0064

#### Scenario: Fake clock
- **WHEN** a scheduler test advances a fake clock by 5 s
- **THEN** exactly the jobs due within 5 s run, in a reproducible order

#### Scenario: Concurrent roots
- **WHEN** the roots (six at floor 0, 384 at floor 3) are loaded on the manual spawner and on pools of 1, 2 and 8 threads
- **THEN** all jobs were handed over before the first wait, a pool as large as the root count has all loads in flight together, and the resident set is identical in every case

#### Scenario: A failing root
- **WHEN** one root fails to load
- **THEN** the other roots are told to stop, no root is resident, the slots are given back and the error names the failing tile and its cause

#### Scenario: Roots beyond the cap
- **WHEN** the 384 floor tiles are loaded at start-up with the default cap of 1 MiB
- **THEN** all 384 are written in the first frame, before it is drawn (3.5 MiB at floor 3 with the 9,488 B slot of GEN-025, more than the cap), and every later frame writes at most the cap or one tile

#### Scenario: Teleport keeps the roots
- **WHEN** the dive and a teleport to the antipode run over many frames
- **THEN** no floor tile is evicted or loaded again after start-up, and the bytes of every later frame obey the cap

### Requirement: STRM-006 Upload and memory budgets
Terrain SHALL have its own share of the VRAM cap (2 GiB at the 8 GiB mid tier, 3 GiB at the 14 GiB high tier, configurable), a ceiling and not a target, counted in pooled fixed-size slots so that a test needs no device. The allocated terrain bytes (the blocks of the pool, index buffers, node records) SHALL be a pure function of the pool configuration and SHALL never exceed the share; a configuration whose fixed items exceed their reserve (0.125 GiB) or that holds no block is refused. The pool holds the compact format (ADR 0033 point 3, REND-016), the only tile format (ADR 0062): a block is one storage buffer of at most 128 MiB (ADR 0054; 14,146 slots of 9,488 bytes = 134,217,248 B at cells 32, GEN-025), so the mid share holds 15 blocks = 212,190 slots and the high share 23 = 325,358 (ADR 0033 point 3 re-cut: -8.2 % against 231,090 and 354,338; the driver block of GEN-074 grew ADR 0063's slot); the record and the climate block are part of the slot and charged per slot, so the fixed reserve holds no per-slot item and no tile size can overflow it with records (the share is a ceiling). A tile SHALL live in one slot of a pool with a free list; allocated, retiring and free slots SHALL always add up to the pool's capacity, and a slot that was given back SHALL NOT be reused until the frames in flight that may read it have retired. Resident tiles plus jobs in flight SHALL never exceed the pool's capacity in slots. When the share is smaller than the residency set R the manager SHALL degrade in this order and never touch the view first: tiles outside R and prefetch tiles are evicted first; then the leaves of R that are not in the draw set D, its ancestors or the roots are replaced by their ancestors (sibling groups merged), farthest first and, among equally far, finest first; only if D with its ancestors and the roots alone exceed the share (a budget failure, reported with the slot counts) are tiles of D replaced by their ancestors, farthest and finest first, as a declared exception to LOD-008's pixel guarantee. The result SHALL be a pure function of R, D and the capacity, and SHALL NOT depend on the order of the inputs. At the standard views the slots needed by R with its ancestors and by D with its ancestors SHALL stay within pinned counts and within pinned fractions of the share's slots.

The per-frame upload cap (ADR 0033 point 9) is a byte count of **tile data** written to the pool in one frame, counted per tile as the size of its upload in the sink's format (the compact slot of 9,488 B at cells 32; the cap is a configuration value, `UploadBudget::tile_bytes_per_frame`, starting at 1 MiB, about 110 compact slots at cells 32). Finished tiles the pool has not received SHALL wait in the order draw set D with its ancestors and the roots (ancestors before descendants, then plan order), then R with its ancestors, then the prefetch set, then the rest; a frame SHALL write the longest prefix of that order whose bytes fit the cap, SHALL stop at the first tile that does not fit (a tile is deferred to the next frame, never dropped, never overtaken by a later tile of any class), and SHALL always write at least one tile when any waits, so a tile larger than the cap is written alone and cannot starve; the cap is exceeded only by that one tile and by the start-up load of the roots (the tiles at the render floor, LOD-011), which is written in the first frame whatever its bytes, before the cap and the one-tile rule govern the tiles behind it (a declared exception, STRM-001, ADR 0064). A tile that is evicted while it waits is forgotten. The tiles that wait are not drawn: the frame draws their deepest resident ancestors or the cover (STRM-016), so no frame is blank for the deferral. The order and what is written are a pure function of the wants and the finished set, not of the thread count or the order the jobs finished (CON-14). The per-frame node records (the indirect path's `indirect_frame_bytes`) are a CPU write into a ring, cannot be deferred and are not tile data: they are reported as their own line (`node_bytes`) and count against CON-21's 2 ms (owner's decision, ADR 0033), outside the tile cap unless the configuration puts them inside (`node_records_in_cap`: the tile allowance is the cap minus the node bytes of the previous drawn plan). The cap of 1 MiB per frame is accepted (ADR 0033 point 9); changing it is a configuration change. The RAM split (point 8) is STRM-033.

Verify: A, C
Status: active
Source: CON-14, CON-20, CON-21, ADR 0031 point 4, ADR 0033 points 1-3, 9 (accepted 2026-10-16) and 10, ADR 0054, ADR 0032, STRM-002, STRM-016, STRM-017, GEN-025, ADR 0063, ADR 0062 point 4, SPIKE-12 Q3

#### Scenario: Terrain share
- **WHEN** the pools are configured for a tier
- **THEN** the allocated bytes (all blocks, index buffers, node records) are at most the tier's terrain share, one more block would not fit, and the real GPU allocation equals the model for the blocks in use

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
- **WHEN** the ten standard views are planned over the default world, against the slots of `tier_compact_pool` (the pools the renderer allocates)
- **THEN** R with ancestors is at most 10 % and the ground-1m D with ancestors at most 6 % of the mid tier's 212,190 compact slots (R at most 10 % of the high tier's 325,358), the allocation model stays inside both shares, and the compact slot grew from 9,168 B to 9,488 B (`MESH_VERSION` 10: the driver block, GEN-074), so the pins are the same fractions of 212,190 and 325,358 slots

#### Scenario: Compact pools
- **WHEN** the tier pools are configured for the compact format at cells 32, 64 and 16
- **THEN** at cells 32 the mid and high shares hold 15 and 23 blocks of at most 128 MiB (14,146 slots at cells 32) and one more block would not fit, the fixed items (index buffers, node records) stay inside their reserve, and a tile size with many small slots per block gets fewer blocks

#### Scenario: Per frame through the loop
- **WHEN** the managed frame loop (STRM-017) turns through eight directions with a share much smaller than R
- **THEN** in every frame resident tiles and jobs in flight stay at or below the capacity, in use plus retiring plus free equal the capacity, no frame reports a budget failure, each view ends at full detail as the synchronous plan, and the frames report the tiles degraded and evicted

#### Scenario: Upload cap
- **WHEN** the managed frame loop (STRM-017) runs the dive script and a teleport on fakes with tiles of 58,608 bytes (larger than the compact slot, so that few tiles fit a frame's cap) and the default cap, tiles finishing faster than the cap lets them in
- **THEN** in every frame the bytes written are at most the cap or exactly one tile, a frame that defers is full (the next tile did not fit), the tiles written are in class order with ancestors first and nothing that waits ranks before the last tile written, the streamer's resident tiles are the sink's plus the waiting ones, and no frame is blank

#### Scenario: Larger than the cap
- **WHEN** every tile is larger than the cap
- **THEN** one tile is written per frame while any waits, the roots first, and the view is never blank once the roots are in

#### Scenario: Upload order on any thread count
- **WHEN** the same script runs with tile jobs finishing on 1, 2 and 8 threads
- **THEN** the tiles written, their bytes and the deferred counts of every frame are identical

#### Scenario: Fill time at the cap
- **WHEN** the 8,763 slots of a full ground-1m fill wait at the default cap
- **THEN** it takes 80 frames of 110 compact tiles (9,488 B each)

#### Scenario: Node records
- **WHEN** a frame is drawn
- **THEN** its node-record bytes are reported apart from the tile bytes, and shrink the tile allowance only when the configuration counts them inside the cap

#### Scenario: The record is charged per slot
- **WHEN** the tier pools are configured with the slot of 9,488 B and the fixed reserve of 0.125 GiB
- **THEN** the mid share holds 15 blocks and 212,190 slots and the high share 23 blocks and 325,358 slots, the fixed items stay inside their reserve

### Requirement: STRM-021 Pre-filled coarse tiles
The bake step SHALL also write the finished render tiles of levels `F..=N` (F the render floor, LOD-011, default 3, `--floor-level N` on `generate --prefill-dir` too; default `N = 4`, 1,920 tiles; `--prefill-levels N` names N, 0 disables, an N below F writes nothing and says so; F is the effective floor min(floor, max_level) of the run, as in LOD-011; at most 8) into the tile pack of the bake, as records of kind 2 (`Kind::Mesh`) with the existing record format and the ADR 0018 mesh key of the tile (`mesh_key_inputs`: field, tile, cells, skirt fraction, `MESH_VERSION` and the climate version word) plus one output parameter naming the form of the stored tile (the compact form, 1; a tile below the render floor is never stored or looked up, and a floor below 3 is refused, because levels 0-2 have no compact form), so that no existing key, record format or version changes. The payload SHALL be the tile exactly as the generators build it, every `f32`, `u16` and `f64` bit (`codec version | form | cells | raw length | LZ4 block`), and SHALL decode to a tile equal to the generated one (payload version 3 since MESH_VERSION 8: the compact form is the slot of GEN-025; a payload of the former vertex form (form byte 0) is refused naming it; a version-1 or version-2 payload is refused naming the version, and records written under MESH_VERSION 7 are unreachable because `MESH_VERSION` is part of the key, so a pack pre-filled under version 7 yields only misses, `missing_tiles` lists every tile and the pre-fill writes the tiles again; the stale records stay in the pack until compaction, ADR 0029); a payload that is short, of another version or form, of cells that are not a power of two from 2 to 1024, of a wrong raw length or damaged SHALL be refused naming what is wrong, and the decoder SHALL NOT allocate more than the cells allow. A pre-filled tile SHALL equal the tile the streamer generates on demand for the same field, cells and finest level, bit for bit, whatever the number of threads that wrote it (CON-14): tiles are generated on any number of threads, coarsest first, and appended in the fixed order level, face, row, column, so the bytes of the pack do not depend on the thread count. A tile at or below the run's finest level SHALL NOT be stored or looked up (until ADR 0062 its key was the finest-level class of GEN-010; nothing was ever stored under it, so the removal of the class cannot make a record answer for a tile whose key changed), and a pre-fill that reaches it, or goes deeper than 8, SHALL be refused before it writes. The pre-fill SHALL be idempotent: a tile the pack already holds under the current key whose payload decodes is skipped, a run that finds none missing opens no writer and changes no byte or file time, a deeper run adds only the new levels, and a record that does not decode counts as missing and is written again (the newer record wins). `generate --global-dir D` SHALL pre-fill after the bake and print the tiles, payload bytes, growth of the packs and wall time; `generate --prefill-dir D --global-res N [--prefill-levels L] [--threads T] [--cells C] [--finest-level F] [--tile-format F]` SHALL pre-fill an existing bake in place and refuse a store that holds no bake of that definition and size. The streamer's tile source (`MeshSource`) SHALL take a tile of level 8 or coarser from the pack when the pack holds it under the current key and generate it otherwise (a cache degrades to a miss: a damaged record, another version, another field, other cells or the other form is a generation, never an error), and a world opened `cold` SHALL never read a tile from the pack (the cold reader of `dive-cold`, TEST-019). Pre-filled tiles are caches evicted with their bake (ADR 0045 point 1) and are not read by `verify-cache` or `verify-global`, which report them as records of another kind.

Verify: A
Status: active
Source: owner (2026-10-16, option A), ADR 0045 point 3, ADR 0018, ADR 0019, STRM-008, STRM-003, CON-05, CON-13, CON-14, M4 fix queue (F9), LOD-011, GEN-025, ADR 0062

#### Scenario: Codec
- **WHEN** compact tiles (level 4, cells 16 and 32) are encoded and decoded, a payload of the former vertex form is decoded, and hostile payloads are decoded
- **THEN** every bit of the tile comes back, and a short payload, another version or form, bad or huge cells, a wrong raw length and a damaged block are each refused naming the cause without allocating by the claim

#### Scenario: Keys
- **WHEN** the key of a tile is made for another field, other cells or another tile
- **THEN** it differs

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
- **THEN** the level 3-4 tiles are taken from the pack and equal the generated ones (also through the staged, cancellable path), the level-5 tile is generated, a cold reader of the same store reads none from the pack and builds the same tiles

#### Scenario: The commands
- **WHEN** `generate --global-dir` bakes with `--prefill-levels 3`, `--prefill-dir` pre-fills an existing bake to level 3, then 4, then 4 again, and the options are misused
- **THEN** the reports count 384 then 1,920 tiles with 384, 1,536 and 0 written, the last run grows the packs by 0 bytes, `--prefill-levels 0` is disabled, and a missing `--global-res`, a store of another size, a depth over 8, a finest level inside the pre-fill and a clash with another command are each refused naming the cause

#### Scenario: A pack pre-filled under an older mesh version
- **WHEN** a pack holds records written under MESH_VERSION 7 keys with version-2 payloads and the tiles of levels 3 to 4 are looked up and pre-filled
- **THEN** every lookup is a miss, a version-2 payload is refused even under the current key, the pre-fill writes every tile again and the new records decode to the generated tiles

#### Scenario: Floor and levels
- **WHEN** a pre-fill with floor 3 and N = 4 runs, and one with floor 0 is requested
- **THEN** the pack holds 1,920 tiles, none of levels 0-2, every stored tile equals the generated one, and the floor 0 pre-fill is refused before it writes (levels 0-2 have no compact form)

#### Scenario: The floor on the command line
- **WHEN** `generate --prefill-dir D --global-res N --floor-level 0 --prefill-levels 1` and the same with `--floor-level 3` run, and `--floor-level 5` with `--finest-level 4` is given
- **THEN** the first is refused naming the floor below level 3 and the compact format, the second (N below F) writes nothing and says so, and the third is refused naming 5 and 4 (LOD-011)
