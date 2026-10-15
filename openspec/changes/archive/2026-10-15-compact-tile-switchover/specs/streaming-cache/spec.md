## MODIFIED Requirements

### Requirement: STRM-006 Upload and memory budgets
Terrain SHALL have its own share of the VRAM cap (2 GiB at the 8 GiB mid tier, 3 GiB at the 14 GiB high tier, configurable), a ceiling and not a target, counted in pooled fixed-size slots so that a test needs no device. The allocated terrain bytes (the blocks of the main pool, the small vertex pool for levels 0-2 and fallback tiles, index buffers, node records, the slot records of the compact format) SHALL be a pure function of the pool configuration and SHALL never exceed the share; a configuration whose fixed items exceed their reserve (0.125 GiB) or that holds no block is refused. The product's main pool is the compact format (ADR 0033 point 3, REND-016): a block is one 4096 x 4096 `Rgba16Uint` atlas (134,217,728 B, 124 x 124 = 15,376 slots of 33 x 33 texels at cells 32), so the mid share holds 15 atlases = 230,640 slots and the high share 23 = 353,648; each slot has a 128-byte record in one buffer of `slots x 128` bytes counted with the fixed items, and a tile size whose records would overflow the reserve gets fewer atlases than the share's bytes allow (the share is a ceiling). The small vertex pool has 512 slots, the bottom 256 for the residency manager (levels 0-2) and the top 256 the renderer's own for fallback tiles (CON-13). The vertex-format configuration (`--tile-format vertex`, the reference) stays and is pinned too. A tile SHALL live in one slot of a pool with a free list; allocated, retiring and free slots SHALL always add up to the pool's capacity, and a slot that was given back SHALL NOT be reused until the frames in flight that may read it have retired. Resident tiles plus jobs in flight SHALL never exceed the pool's capacity in slots. When the share is smaller than the residency set R the manager SHALL degrade in this order and never touch the view first: tiles outside R and prefetch tiles are evicted first; then the leaves of R that are not in the draw set D, its ancestors or the roots are replaced by their ancestors (sibling groups merged), farthest first and, among equally far, finest first; only if D with its ancestors and the roots alone exceed the share (a budget failure, reported with the slot counts) are tiles of D replaced by their ancestors, farthest and finest first, as a declared exception to LOD-008's pixel guarantee. The result SHALL be a pure function of R, D and the capacity, and SHALL NOT depend on the order of the inputs. At the standard views the slots needed by R with its ancestors and by D with its ancestors SHALL stay within pinned counts and within pinned fractions of the share's slots.

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
- **WHEN** the ten standard views are planned over the default world, against the slots of `tier_compact_pool` (the pools the renderer allocates)
- **THEN** R with ancestors is at most 10 % and the ground-1m D with ancestors at most 6 % of the mid tier's 230,640 compact slots (R at most 10 % of the high tier's 353,648), the allocation model stays inside both shares, and the vertex-format fractions are pinned at 40 % and 12 % of the mid tier (R at most 26 % of the high tier's slots)

#### Scenario: Compact pools
- **WHEN** the tier pools are configured for the compact format at cells 32, 64 and 16
- **THEN** at cells 32 the mid and high shares hold 15 and 23 atlases and one more atlas would not fit, the fixed items (vertex pool, index buffers, node records, slot records) stay inside their reserve, and a tile size with many small slots per atlas gets fewer atlases

#### Scenario: Per frame through the loop
- **WHEN** the managed frame loop (STRM-017) turns through eight directions with a share much smaller than R
- **THEN** in every frame resident tiles and jobs in flight stay at or below the capacity, in use plus retiring plus free equal the capacity, no frame reports a budget failure, each view ends at full detail as the synchronous plan, and the frames report the tiles degraded and evicted

### Requirement: STRM-007 Resident capacity
The streamer SHALL count its capacity in pooled slots (STRM-006): a slot is reserved when a job is dispatched and given back when the tile is evicted, cancelled or failed, so resident tiles plus jobs in flight SHALL NEVER exceed the capacity. It SHALL evict only to make room for waiting requests, in this order: tiles in none of D, R and the prefetch set, and prefetch-only tiles, least recently used first; for draw requests only, then tiles of R that are not in D, farthest first and finest first. It SHALL NEVER evict a level-0 tile or a tile of the current draw set or one of its ancestors, SHALL NOT let a residency or prefetch request evict a tile of R, and SHALL retry failed tiles only after a configured delay measured on the injected clock. A tile that left R stays resident until the capacity needs its slot. A loop that mirrors the resident set into a renderer SHALL place each tile that becomes resident in the slot the streamer assigned to it and never choose a slot itself, SHALL forget each evicted tile before any other tile is written to its slot, and SHALL hold exactly the streamer's resident set; the one exception is a CON-13 fallback tile of the compact renderer, which keeps the main slot the streamer reserved empty and lives in the renderer's own fallback region of the vertex pool (a fallback beyond the region is refused with the tile id).

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
