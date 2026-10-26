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
- **THEN** all 384 are written in the first frame, before it is drawn (3.4 MiB at floor 3 with the 9,488 B slot of GEN-025, more than the cap), and every later frame writes at most the cap or one tile

#### Scenario: Teleport keeps the roots
- **WHEN** the dive and a teleport to the antipode run over many frames
- **THEN** no floor tile is evicted or loaded again after start-up, and the bytes of every later frame obey the cap

### Requirement: STRM-006 Upload and memory budgets
Terrain SHALL have its own share of the VRAM cap (2 GiB at the 8 GiB mid tier, 3 GiB at the 14 GiB high tier, configurable), a ceiling and not a target, counted in pooled fixed-size slots so that a test needs no device. The allocated terrain bytes (the blocks of the pool, index buffers, node records) SHALL be a pure function of the pool configuration and SHALL never exceed the share; a configuration whose fixed items exceed their reserve (0.125 GiB) or that holds no block is refused. The pool holds the compact format (ADR 0033 point 3, REND-016), the only tile format (ADR 0062): a block is one storage buffer of at most 128 MiB (ADR 0054; 14,146 slots of 9,488 bytes = 134,210,352 B at cells 32, GEN-025), so the mid share holds 15 blocks = 212,190 slots and the high share 23 = 325,358 (ADR 0033 point 3 re-cut, ADR 0063: -5.0 % against 231,090 and 354,338; ADR 0063 corrects ADR 0062 point 4 to this slot); the record and the climate block are part of the slot and charged per slot, so the fixed reserve holds no per-slot item and no tile size can overflow it with records (the share is a ceiling). A tile SHALL live in one slot of a pool with a free list; allocated, retiring and free slots SHALL always add up to the pool's capacity, and a slot that was given back SHALL NOT be reused until the frames in flight that may read it have retired. Resident tiles plus jobs in flight SHALL never exceed the pool's capacity in slots. When the share is smaller than the residency set R the manager SHALL degrade in this order and never touch the view first: tiles outside R and prefetch tiles are evicted first; then the leaves of R that are not in the draw set D, its ancestors or the roots are replaced by their ancestors (sibling groups merged), farthest first and, among equally far, finest first; only if D with its ancestors and the roots alone exceed the share (a budget failure, reported with the slot counts) are tiles of D replaced by their ancestors, farthest and finest first, as a declared exception to LOD-008's pixel guarantee. The result SHALL be a pure function of R, D and the capacity, and SHALL NOT depend on the order of the inputs. At the standard views the slots needed by R with its ancestors and by D with its ancestors SHALL stay within pinned counts and within pinned fractions of the share's slots.

The per-frame upload cap (ADR 0033 point 9) is a byte count of **tile data** written to the pool in one frame, counted per tile as the size of its upload in the sink's format (the compact slot of 9,488 B at cells 32; the cap is a configuration value, `UploadBudget::tile_bytes_per_frame`, starting at 1 MiB, about 114 compact slots at cells 32). Finished tiles the pool has not received SHALL wait in the order draw set D with its ancestors and the roots (ancestors before descendants, then plan order), then R with its ancestors, then the prefetch set, then the rest; a frame SHALL write the longest prefix of that order whose bytes fit the cap, SHALL stop at the first tile that does not fit (a tile is deferred to the next frame, never dropped, never overtaken by a later tile of any class), and SHALL always write at least one tile when any waits, so a tile larger than the cap is written alone and cannot starve; the cap is exceeded only by that one tile and by the start-up load of the roots (the tiles at the render floor, LOD-011), which is written in the first frame whatever its bytes, before the cap and the one-tile rule govern the tiles behind it (a declared exception, STRM-001, ADR 0064). A tile that is evicted while it waits is forgotten. The tiles that wait are not drawn: the frame draws their deepest resident ancestors or the cover (STRM-016), so no frame is blank for the deferral. The order and what is written are a pure function of the wants and the finished set, not of the thread count or the order the jobs finished (CON-14). The per-frame node records (the indirect path's `indirect_frame_bytes`) are a CPU write into a ring, cannot be deferred and are not tile data: they are reported as their own line (`node_bytes`) and count against CON-21's 2 ms (owner's decision, ADR 0033), outside the tile cap unless the configuration puts them inside (`node_records_in_cap`: the tile allowance is the cap minus the node bytes of the previous drawn plan). The cap of 1 MiB per frame is accepted (ADR 0033 point 9); changing it is a configuration change. The RAM split (point 8) is STRM-033.

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
- **THEN** R with ancestors is at most 10 % and the ground-1m D with ancestors at most 6 % of the mid tier's 212,190 compact slots (R at most 10 % of the high tier's 325,358), the allocation model stays inside both shares, and the compact slot grew from 8,840 B (8,712 B of texels and a 128-byte record) to 9,488 B (`MESH_VERSION` 8: the climate block and the record are inside the slot, GEN-025), so the pins are the same fractions of 212,190 and 325,358 slots

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
- **THEN** it takes 77 frames of 114 compact tiles (9,488 B each)

#### Scenario: Node records
- **WHEN** a frame is drawn
- **THEN** its node-record bytes are reported apart from the tile bytes, and shrink the tile allowance only when the configuration counts them inside the cap

#### Scenario: The record is charged per slot
- **WHEN** the tier pools are configured with the slot of 9,488 B and the fixed reserve of 0.125 GiB
- **THEN** the mid share holds 15 blocks and 212,190 slots and the high share 23 blocks and 325,358 slots, the fixed items stay inside their reserve
