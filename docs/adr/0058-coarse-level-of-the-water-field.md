# 0058 A coarse level of the water field: a summary the bake writes once
Status: proposed
Date: 2026-10-17
Source: docs/learnings.md 2026-10-17 (dive-lit lag), docs/status.md item 9a.1, ADR 0041 point 2, ADR 0053 points 4 and 5, ADR 0056 points 1 and 4; specs GEN-024, STRM-025, ATMO-012, ATMO-019

## Context
- A tile at or above the level of the bake's blocks spans one block or many (levels 0 to 4 of a 16384-cell bake in blocks of 1024 cells). `WaterBook::fill` asks W at the tile's 25 sample directions and loads up to 25 blocks of about 16 MiB through the 31-block cache.
- The dive's first wave is 36 such tiles: about 900 block loads at about 180 ms, no tile resident before frame 430, descent lag mean 1.7 against a budget of 1. A cache eight times larger does not fix it.
- Skipping the fill for coarse tiles moves the same reads to the frame thread.
- A tile's water is a function of the bake alone (CON-14), so a stored answer is as good as a computed one.

## Options
- (a) Fill coarse tiles later and let the plan skip tiles without an entry: the plan then depends on timing (CON-14).
- (b) A coarse level of W, written once by the bake: chosen.
- (c) Fewer samples for coarse tiles: changes the selection without removing the reads.
- (d) A larger block cache: insufficient.

## Decision
1. W has a coarse level: a summary of the bake with one cell per block of bake cells smaller than a block (so a cell lies inside one block), written once by the bake as a record of its own (kind 5) from the sample blocks and the ocean channel. It holds no sea level, so one summary serves any level.
2. A tile at or above the block level is answered from the summary and reads no block: sea if any cell it touches may hold sea, else the coastal band with a reach counted in cells. Tiles below the block level keep the point query of ADR 0053 and its answers bit for bit. A field without a summary (an in-memory bake) answers every tile point by point.
3. Departures. From ADR 0053 points 4 and 5 and ADR 0056 point 1, for coarse tiles: the answer is the union over the tile's cells, not the most frequent of 25 samples, and the band's reach is counted from the cells with a larger fraction of the tile's edge. A coarse answer is a superset of the point query's and a parent is wet wherever its children are. A coarse tile wet by a corner of sea draws the sea over a dry basin inside it (the known limit of ADR 0053, at more tiles). From ADR 0056 point 4: a coarse tile's job reads no block for its water.
4. The key names the summary's implementation and the ocean channel's, so a change of either re-keys it. No `GLOBAL_VERSION` bump: the record has its own kind and key, as the ocean channel had (STRM-023). Sample blocks, ocean blocks, tiles and `MESH_VERSION` are unchanged.
5. A store with the ocean channel and without the summary is refused by the viewer, naming `generate --global-dir DIR`, which writes only the missing summary from the stored blocks (idempotent). No silent fallback to the point query.

## Consequences
- Coarse tile jobs read no block; the first wave of the dive costs the pack reads only.
- The bake gains one pass over the stored blocks and a small record. Existing stores need the command once.
- Water goldens do not move: they use in-memory bakes, which have no summary.
- The same world selects water differently from a store (coarse union) and from an in-memory bake (points) for tiles at or above the block level.
- On acceptance, the status lines of ADR 0053 and ADR 0056 name this ADR for the points above.

## Licences of adopted code or techniques
None.
