## Why

Perf window 1 (2026-10-16, RTX, `perf stream all` over the 16384 store): the frame thread's scheduler column was median/p99 13.5/18.2 ms in the dive, 15.6/20.5 in teleport-1m and 9.5/10.6 in look-1m with **zero requests**. The scheduler was most of every frame. A settled look has nothing to do, so the cost scaled with |R| (about 11.5k tiles at ground-1m, finest level 22), not with changes.

The ablation (docs/learnings.md, 2026-10-16) shows why: every frame the manager's `degrade` (a BTreeMap of R, pinned set, ancestors closure) and the streamer's `needed` map (every tile of D and R walked up through all its ancestors, about 170k hash writes) were rebuilt from the same unchanged wants, plus a `last_used` touch of every resident needed tile, a scan of `needed` against the resident set and, with a backlog, a re-key of every queued request. At a settled look: 6.1 ms, 56% `needed`, 36% `degrade`, 5% touch.

## What Changes

- streaming: the scheduler holds the wants as persistent layers per request class (draw, residency, prefetch): the listed tiles with priorities and, with them, their ancestors as child-counted closure sets. A new list is compared with the held one in one merge pass and only the tiles that differ are written; the tiles whose best class changed are handled once, after all three layers changed (queue, cancellation, eviction candidates). Priority keys are derived lazily (a residency set made from a selection keys by its own priorities, no map).
- streaming: the request queue is ordered lazily (class, then level, then priority key, then tile): a change of a class's priorities marks its buckets stale, and a bucket is put in order only when the dispatcher or the eviction look-ahead reads it. Eviction candidates (tiles in no class or prefetch only, oldest first) are kept in order as classes change; the R-only tier is found on demand. `last_used` is no longer touched per frame (a tile in use is "now"; the frame it left use is stored when it does).
- streaming: `Streamer::set_draw / set_residency / set_prefetch / step`, `ResidencyManager::set_draw / set_residency / set_prefetch / step` take only the lists that changed; `begin_frame_wants` and `ResidencyManager::begin_frame` stay (all lists every call) and give the same requests, cancellations and evictions. `degrade` runs only when R with D's closure could exceed the pool.
- frame: the managed loop hands over only the lists that changed (D when a plan is delivered, R when a different R is delivered: a delivered R equal to the held one is compared, not rebuilt, prefetch when the capped set changed; an unchanged prefetch set is not capped again).
- work counters (`SchedWork`: elements scanned and written per frame, per layer) so that tier A asserts the cost in counts (CON-20).
- spec: STRM-019 (new): the scheduler's frame work follows what changed.

Not in this change: the pending-upload ranking and the tile writes (`upload_ms`, F5), the cold plan at altitude (F3), first-frame record (F4), test-fast time (F6). No golden, pinned count or mesh version changes.
