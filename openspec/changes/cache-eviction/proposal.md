# cache-eviction (GEN-153, inbox #99)

## What
- `bake::memo::Memo`: a memo of vectors by key with the tick of each entry's last use, eviction least recently used first (ties by key), pinned entries never evicted.
- `PartCache` and `RoundCache` take a budget in bytes (`with_budget`, `set_budget`, `bytes`); the memos use `Memo`; `PartStats` and `RoundWork` report `evicted` and `evicted_bytes`.
- `solve_rounds_memo` pins what the solve uses and evicts after each round and at the end.
- Tests: tier A, G-EQ after eviction (a solve, an edit, its undo, replays) on both grids, the budget kept, no budget evicts nothing.

## Policy (A3 6.6)
| Data | Policy here |
|---|---|
| User-authored sources, definition, edit history | not in these caches |
| Current committed result | the caller's `z`; the newest entries are the last evicted |
| Active solve | pinned until it completes |
| Previous-round intermediates, inactive edit caches | evicted: previous results, then tile floods, then catchment rounds, LRU |
| Recomputable derived data | every entry is a memo: a miss recomputes it |

## Notes
- The change counts of `PartStats` (`tiles_*_changed`) read against the previous result still held; after an eviction of it they read as on a first call. A report, not a result.
- The explorer's GEN-158 plan keeps relying on `without_memory`, unchanged.
- No code outside `bake/{memo,partition,round}.rs` changes; the edit-cases example uses the default (no budget).

## Not in this change
- The G-SCALE scenarios (repeated edits, undo and redo after eviction at scale): a run of the example with a budget belongs to the scenario measurements, not here.
