## 1. edit-invalidation-reach
- [x] 1.1 Read the key code (cache `key.rs`, `ReadEntry::Query`, `inputs_hash_for`, `cells_covering`) and write step 1's design: kind 3 layout, margin, tag 3 effect on worlds without items, store migration
- [x] 1.2 Spec deltas: WDEF-005 (footprint is the invalidation unit), STRM-003/STRM-010 (node set id per tile), GEN-017 (tile level), TEST (reads contract)
- [ ] 1.3 Failing tests: footprint find returns exactly the tiles within reach (false positives allowed, no false negatives); moving an item outside a tile's reach changes no key; the declared-versus-recorded reads contract
- [ ] 1.4 Item `reach_m`, `max_delta_m`; refusal over the reach limit; query kind 3 and its result material; tag 3
- [ ] 1.4b Tier-A test: a key without footprint entries is byte-identical to today's; the encoding writes 16-byte ids and 16-byte content hashes
- [ ] 1.4c Note for E2/E4: the switch to 128-bit (ULID) ids is a task of whichever first reads or writes item files, before any item file exists
- [ ] 1.5 numerics-reviewer (core/generators), spec-guardian, verifier; PR

## 1b. edit-invalidation-summaries
- [ ] 1b.1 Design: the summary limit (spec value), kind 4 layout, node store and the rewrite of one node per level on an edit; spec deltas WDEF-005, STRM-003
- [ ] 1b.2 Failing tests: a coarse tile's keys change on an edit inside its cells and not outside; an edit rewrites one node per level; empty subtree hashes are level-dependent
- [ ] 1b.3 Implement; numerics-reviewer on the hash encodings; spec-guardian, verifier; PR

## 2. edit-invalidation-refresh
- [ ] 2.1 Design from SPIKE-13 variant D (`spike/spike-13-edit-latency`: `Streamer::refresh`, `force_replan`, stale draw); spec deltas STRM-005, STRM-015, STRM-019
- [ ] 2.2 Failing tests: a changed planned key refreshes the resident tile and the old one is drawn until the new one is uploaded; an edit event forces a plan while the camera is still; edited tiles are dispatched before the rest of R
- [ ] 2.3 Implement; keep the key memo per tile invalidated by the item revision counter
- [ ] 2.4 Reviewers; PR; indicative edit-to-exact numbers from `test gpu --real` (a measured window comes from the planner)

## 3. edit-invalidation-bounds
- [ ] 3.1 Spec deltas LOD (bounds by reach) and the `SelectionCache` key
- [ ] 3.2 Failing tests: bounds widen by the sum of `max_delta_m` of the returned items; an edit invalidates bounds and volumes of the found tiles only; the steep field at levels 16 to 18 plans without an empty morph interval
- [ ] 3.3 Implement; reviewers; PR
