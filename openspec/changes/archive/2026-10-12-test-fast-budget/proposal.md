## Why

`cargo xtask test fast` took ~19 s of test time against its 10 s budget (report §16); one test, `morph_is_bounded_and_merges_do_not_pop_over_real_terrain` (LOD-004), took 18 s alone: 2 fields x ~300 receding camera heights, each a full node selection, in one thread.

## What Changes

- The sweep is a function (`morph_sweep`) and runs as ten consecutive height chunks per field, one test each, so the test runner uses its cores. Same heights (30 m x 1.03^k up to 200 km, up to float rounding), same per-step checks (morph in [0, 1], levels never increase, every merge goes from morph >= 0.8 to <= 0.5); each chunk starts one step early so the step across a chunk bound is checked too. The old "at least 8 merges" check cannot be counted per chunk; `the_chunked_morph_sweep_covers_many_level_changes_from_ground_to_orbit` checks that the level under the camera falls monotonically over at least 8 levels across the chunk bounds (equivalent to the merge count because each merge is now asserted to drop exactly one level, so the level span equals the number of merges; the other per-step checks are unchanged).
- No requirement changes (LOD-004 tests only). Nothing skipped or deleted.
- Result: that test is no longer the long pole; `test fast` test time 18.9 s -> ~12 s on the 20-thread dev machine. Still over 10 s: the next long tests are `planet-frame::lod` (culling_stays_conservative_for_known_hard_cameras, leaves_cover_the_visible_surface_exactly_once, morph_*) and `planet-frame::steep` (5-8 s each under load). Recorded in status.md, not changed here.
