# 0057 A coarse level of the water field: a summary the bake writes once
Status: proposed
Date: 2026-10-17
Source: docs/learnings.md 2026-10-17 (dive-lit lag), docs/status.md item 9a.1, ADR 0041 point 2, ADR 0053 points 4 and 5, ADR 0056 point 4; specs GEN-024, STRM-025, ATMO-012, ATMO-019

## Context
- A tile of a level above the bake's blocks spans many blocks (levels 0 to 3 of a 16384-cell bake in blocks of 1024 cells). `WaterBook::fill` asks W at the tile's 25 sample directions and loads up to 25 blocks of about 16 MiB through the 31-block cache.
- The dive's first wave is 36 such tiles. Measured on the RTX: about 900 block loads at about 180 ms, no tile resident before frame 430, descent lag mean 1.7 against a budget of 1. A cache eight times larger gives lag 2.07.
- Skipping the fill for coarse tiles removes the wait and moves the same reads to the frame thread (frames of 0.3 to 8 s).
- The tile's water is a function of the bake alone (CON-14), so the answer for a coarse tile can be a stored one.

## Options
- (a) Fill coarse tiles later and let the plan skip tiles without an entry: the plan then depends on timing (CON-14).
- (b) A coarse level of W, written once by the bake: chosen.
- (c) Fewer samples for coarse tiles: changes the selection and the lake-size limit of ADR 0053 without removing the reads.
- (d) A larger block cache: measured insufficient.

## Decision
1. W has a coarse level: a summary of the bake with one cell per 64 x 64 bake cells (the cell lies inside one block), written once by the bake as a record of its own (kind 5) from the sample blocks and the ocean channel. It holds the facts the water questions need and no sea level, so one summary serves any level.
2. A tile at or above the bake's block level is answered from the summary and reads no block. The answer is the union over the cells the tile touches: sea if any cell may hold sea, else the coastal band with a reach counted in cells (ATMO-019). Tiles below the block level keep the point query of ADR 0053 and its answers bit for bit.
3. A coarse answer is a superset of the point query's, and a parent is wet wherever its children are. Departure from ADR 0053: a lake or inlet smaller than the 25-sample spacing is no longer missed by a coarse tile, and a coarse tile wet by a corner of sea draws the sea over a dry basin inside it (the known limit of ADR 0053, now at more coarse tiles).
4. No `GLOBAL_VERSION` bump: the record has its own kind, implementation id (`cpu-water-summary-v1`) and key, as the ocean channel had (STRM-023). Sample blocks, ocean blocks, tiles and `MESH_VERSION` are unchanged.
5. A store with the ocean channel and without the summary is refused by the viewer, naming `generate --global-dir DIR`, which writes only the missing summary from the stored blocks (idempotent). No silent fallback to the point query.

## Consequences
- Coarse tile jobs read no block; the first wave of the dive costs the pack reads only.
- The bake gains one pass over the stored blocks (about 10 s at 16384 cells) and 0.45 MB.
- The 16384 store was migrated in place: 44 s in all, 451,928 bytes appended.
- Water goldens do not move: they use in-memory bakes, which have one block per face and so no coarse tile.
- Tests: the summary against a reference read off whole faces, the superset and parent-child properties over real bakes, zero block reads for coarse jobs, thread-count independence, migration and verification of the record.

## Licences of adopted code or techniques
None.
