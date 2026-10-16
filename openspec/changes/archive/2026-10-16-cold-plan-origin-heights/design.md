## Where the plan's time goes

A plan is a selection (a tree descent that decides on the tiles' height-bounds volumes), the origin height `field.height(centre)` of each selected node (memoised per tile in the `SelectionCache`, LOD-009), and the mesh keys. The origin heights are not inputs of the selection, so they can be evaluated after it, all together. Above 500 km the selection reads no lattice cell (the height bounds of a wide cap are the global range) and costs under 1 ms; below 20 km it costs 55-95 ms (about 1-2 M lattice reads inside one or two blocks, already decoded).

At altitude an origin height reads about 18 lattice cells that sit in 2-4 different blocks (the erosion stencil), so the 52 nodes of the 20,000 km plan touch 188 blocks for 938 reads. A block costs about 9 ms to read and 12 ms to decode (LZ4 plus the channel transpose) however few of its cells are used: one thread needs 3.8 s.

## The batch

`HeightField::heights_threaded(dirs, threads)` returns, in the caller's order, exactly `height(dir)` per direction. For the lattice-reading fields it is `eval_gathered_threads`: (1) the footprint pass collects the cells every direction reads (a recording source, serial, arithmetic only), (2) the distinct cells are sorted by (face, Z-order block) and cut into runs of one block, (3) up to `threads` workers take the runs in order from an atomic counter and read the cells of one run at a time from the real source, (4) the cells form a table and the directions are evaluated from it in contiguous chunks over the threads. The table holds the source's own samples, so every result is what the single-sample path returns. With one thread it is `eval_gathered` with a floor of two directions (the tile builders keep 64).

`threads` blocks are in flight at once and each is read by one thread in one run, so a block is decoded once whatever the cache budget, if the source keeps the block a thread is reading while it reads it. `StoreSource` does: a thread pins the block it read last. The pin used to expire with the source's eviction epoch; under 20 decoding threads the epoch moves a block per insert, the pin was stale at once, and the next cell of the block was looked up in a cache that had already evicted it (261 decodes for 188 blocks with the batch in place). The pin is now valid while the thread reads the same block (tier A with the real source at budgets of 1, 3 and 31 blocks and 1, 2 and 20 threads). A stopped batch returns 0.0 for the unfinished directions, so a caller that can stop a plan must not memoise them; the plan passes no stop flag today.

## The plan builder

`origin_heights(selection, ...)`: nodes whose tile is memoised take the memo; the others are one batch. A field that reads cells (`reads_cells`) memoises nothing and evaluates every node, as before, but also as one batch. A non-finite result is not memoised. Nothing new: no call at all. The selection never reads an origin height, so there is no level-by-level dependency to batch around: one batch per plan, which is at most the tree depth.

## Bit identity

The plan equals the old per-node plan: tier A compares every node's origin bits with `height(centre)` for the ten standard views and the dive's five altitude bands above 100 km, and the plans of 1 and 20 threads for equality, over a synthetic multi-block bake behind a source that counts decodes like the store's cache. No golden, pinned count or `MESH_VERSION` moves.

## Not taken: the origin from a resident tile

A resident tile carries its own origin (`RenderTile`), but the plan builder is GPU-free and has no resident set (CON-03: the decision lives in the plan, the residency in the manager), and the origins that cost time are those of nodes that are not resident yet (a cold plan at altitude has nothing resident). The branch would add a coupling for no measured gain, so it is skipped.

## What the numbers say

See docs/learnings.md 2026-10-16 (release, 20 plan threads, real store, cold source and cache per measurement).
