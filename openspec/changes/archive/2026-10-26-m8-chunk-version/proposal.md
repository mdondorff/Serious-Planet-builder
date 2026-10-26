## Why
M8-7 step 1 of `m8-edits-over-chunks` (D3, owner 2026-10-26): an edit applies under the erosion. Today a chunk is keyed by `(face, i, j)` and a ready chunk is never re-requested; `Under` is refused (GEN-039). A chunk needs a version that changes when an edit changes its bits, and an edit must make the touched chunks re-solve when they are read.

## What Changes
1. **The read set (GEN-081).** One definition of everything a chunk's bits can depend on in space: the chunk's region (square plus the A-net halo), and the routed network it reads: the coarse segments that can touch the region (the pruning of GEN-078) widened by `routing_reach_m`, and the upstream window of the beds (`law.window` segments back along the main stem). `anet::chunk_read_set` returns it as discs; the pruning of GEN-078 and the version use the same function.
2. **The version (GEN-082).** A chunk's version is the hash of the items (`footprint_hash`) whose reach intersects the read set, in the `Under` chain. It is part of the chunk's disk key and of its landed record. No item in the read set: the version is the empty hash, equal to today's chunk.
3. **Under (GEN-039 MODIFIED).** `Under` is the default for a world with a chunk stage; the A-net's base is base plus the stamps. `OnTop` stays representable.
4. **Stale, then re-solved when read (STRM-056).** An edit marks the ready chunks whose read set it touches stale (`edited_chunks` is the tile-side test; the read set is the chunk-side test). A stale chunk keeps drawing its old version (STRM-054); it is re-solved when the plan reads it, nearest first, ahead of prefetch, at most 2 of the 3 workers. The new version lands as a second landing (STRM-052, STRM-053).
5. **The horizon (STRM-057).** The state word of a chunk's horizon carries the hash of the items that touch the chunk and its halo (`HorizonBaker::edit_word`), not the definition's revision: an edit bakes only the maps it touches.
6. **Tests, tier A:** a re-solve after an edit equals a cold solve of the new version bit for bit (CON-14); an edit just outside the halo and inside the routing reach changes the version and the bits; an edit outside the read set changes neither.

## Impact
- Specs: GEN-039 (MODIFIED), GEN-081, GEN-082, STRM-056, STRM-057 (the horizon of a chunk is baked again only when an edit touches it), STRM-049 and WDEF-018 (MODIFIED).
- Code: `generators/anet.rs` (`chunk_read_set`), `app/anetworld.rs` (`WorldBase` with stamps, the version), `streaming/chunks.rs` (version, stale, order), `worldfield.rs` (the edit hook).
- Not in this change: the base-height and routing reuse (`m8-resolve-reuse`, next), debounce (step 3), the horizon re-bake of touched chunks only (step 4), the driver increment plane.
- Consequence to state: an edit on a river re-keys chunks well downstream (the bed window spans many coarse cells). The lazy re-solve keeps that cheap: only drawn chunks re-solve.
