## Why
B6 PR 6 of `remove-chunk-stage` (owner, 2026-10-29; ADR 0086), the last: after PRs 1 to 5 the remaining mentions, dead code, one-off examples and the gate's level-step figure still described the removed stage.

## What changes
- `streaming` no longer depends on `planet-cache`. `world-def`: `same_but_items`, `changed_items_from` and `ChangedItem` removed (`item_changes_from` keeps its result; the definition hash is untouched).
- Examples `plan_ablation`, `residency_counts`, `scheduler_ablation` removed (M4 ablations of the chunk-era plan and scheduler); the wrapper count of `stamp_keys.rs` is 4.
- Comments and help text reworded to what is true now (horizon jobs); the dead `--base-threads` entry of the scene option lists removed. Specs reworded: STRM-046 (title: the viewer draws the base terrain only), STRM-049, STRM-059, STRM-064, TEST-054, TEST-060, TEST-061, TEST-066, VIEW-054; stale text of `m8-horizon-share`, `shadow-clipmap-march`, `b4-owner-checks` and `terrain-scorecard` no longer cites removed IDs as live.
- `base_steps_report` measures the step between a level and its parent on the base field (the tiles under one river's tile at levels 12 to 16, every vertex shared with the parent), printed per level; the gate prints it as "measured on the base field" and fails over 0.25 m. The four KNOWN `levels` rows (they described the stage-on run) are gone.
- New ignored check `the_real_store_opens_without_a_rebuild` (TEST-075, second scenario), run by `cargo xtask gate`: opens `C:/DEV/SeriousPlanetData/global16384` read only with the default world (seed 1) and loads the stored coupled run by its key; prints MATCH, NOT RUN (store absent) or a failure.
- `docs/status.md`: queue row B6 removed; two Tracked rows reworded.

## Default path
No golden, PNG, tile key, slot byte, shader or definition hash changes. `max_nodes` of the renderer comes from the fixed node-record reserve of the terrain share (`MAX_PLAN_NODES * 512` B = 16 MiB, `pool.rs`), not from the plan, so removing the fade planes changed no BUDG-005 number.
