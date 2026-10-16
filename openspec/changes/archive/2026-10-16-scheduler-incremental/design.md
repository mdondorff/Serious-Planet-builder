## Principle

ADR 0031's semantics are a function of the wants (D, R, prefetch), the resident set, the failures and the pool. None of them changes on most frames. The scheduler therefore keeps what it derives from the wants between frames and updates it from differences. Same requests, in the same order, the same cancellations and evictions: the incremental scheduler is checked against digests of the previous implementation (see Equivalence).

## Layers

One `Layer` per request class holds `items` (tile, priority; sorted by tile, unique) and `count[t]` = 1 if `t` is listed + the number of children of `t` in the layer. A tile is in the layer (listed or an ancestor of a listed tile) iff `count[t] > 0`. Adding or removing a listed tile walks up only while membership flips, so a change costs O(changes x levels climbed) and reports exactly the tiles that flipped. A new list is compared with `items` in one merge pass (sequential, no hashing: the "scanned" counter); only the tiles that differ are written ("updated"). The class of a tile is its best layer (draw, residency, prefetch), as `needed` took the best key.

All three layers are replaced before any tile's class change is handled, and a tile is handled once from its old and new class. Handling them layer by layer would cancel a tile that moves from D to R on the way.

## What reacts to a class change

- resident tile: it joins or leaves the eviction candidates (tiles in no class or prefetch only, level 0 excluded), ordered by the frame it was last in use. A tile in use has no stored value: when it leaves use at frame f its value is f - 1 (or the frame it arrived, if later), exactly what the per-frame touch produced.
- tile in flight: cancelled once if it left every class (a cancelled job is never revived).
- queued tile: removed (counted as cancelled, prefetch separately) if it left every class; otherwise re-bucketed when it changed class.
- missing tile (wanted, not resident, not running, not waiting out a failure): queued. The set of tiles that may need a request is kept (`missing`): wanted tiles that arrived in a layer, tiles whose job ended without a resident tile, evicted wanted tiles (queued again from the next frame, as before), failures whose retry delay passed (kept ordered by failure time, so a frame looks only at the due ones).

## Queue

The queue is buckets by (class, rank), rank = level for residency and prefetch (coarse first) and 0 for draw. Within a bucket the order is the priority key. Keys are the layer's: for draw the best `priority - depth x 1e12` over the listed tiles below (the old arithmetic, with an early stop only when it provably gives the same value); for residency and prefetch the best priority below, which for a set closed under parents with parents never worse than children (a residency set made from a selection) is the tile's own priority (no key map is built). A change of a class's lists or priorities marks its buckets stale; a bucket is put in order only when read, the dispatcher and the eviction look-ahead read from the front, so a camera that moves costs the buckets at the front and not the queue (a backlog of thousands). Tiles queued since a bucket was last read are merged in individually.

## Fitting to the pool

`degrade` is a pure function of (D, R, capacity) and stays as it is. The manager runs it only when R with the closure of D could exceed the pool: an upper bound from the sizes (|R| if R is closed under parents, the sum of levels of D's tiles, the roots) against the capacity; the bound is evaluated when a list changes, never per frame. Under the bound nothing would be degraded, the lists go to the streamer as they are and the report's numbers come from counters the layers keep (tiles in the draw or residency class that count against the pool; the draw closure with the roots). Over the bound `degrade` fits them and the streamer is given the fitted lists, once per change. (D tiles absent from R, and the roots, are `pinned` entries of `degrade`'s R; they are in the draw class anyway, so leaving them out of the residency layer changes no class.)

## Managed loop

The loop rebuilds D when a different plan is delivered and R when a different R is delivered (compared with the held one by tiles and distances: the worker answers every request, a turn included, with an equal R); it hands the manager only the lists that changed. The prefetch set is capped by the free share only when its inputs changed: the same set with room for all of it stays the same wants. A rotation at a settled position changes D every frame (and its comparison costs O(|D|)) and R never.

## Cost model (STRM-019)

A frame that changes nothing touches nothing (0 scanned, 0 written). A frame in which the draw set turns scans O(|D|) and writes O(tiles that differ x levels climbed); R, prefetch and the queue are not touched unless a class changed. A frame in which c tiles join or leave R writes O(c x levels climbed) plus the requests that start; the one pass over R that finds them is a sequential comparison (scanned, no write). The tier-A tests assert the writes are equal at N and 4N tiles for the same c.

## Equivalence

Digests of the full per-frame record (the answers, the counts, the dispatch log, the resident changes; through the managed loop also the frame counts, the order tiles were written to the sink, the forgets and the pending set) of five scripted runs on the streamer alone (pool that holds everything, pool smaller than R, failures with retries, a small in-flight limit, a vertex pool) and five through the managed loop (prefetch, upload budget, pools smaller than R) were taken from the implementation before this change (a8d7fb0) and are pinned in `crates/frame/tests/scheduler_incremental.rs`. Property tests compare the layers' membership and keys, and the queue's order, with a brute-force rebuild on random sequences (including NaN, infinite and huge priorities).
