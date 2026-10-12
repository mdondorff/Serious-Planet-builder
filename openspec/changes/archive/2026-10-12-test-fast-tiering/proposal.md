## Why

`cargo xtask test fast` was 15.8 s on a quiet machine (budget 10 s, report §16). The slowest tests were older planet-frame and cache tests; cutting what they sample would weaken them, so they move to tier B (`tests/oracle_*.rs`), which `cargo xtask test` and CI still run.

## What Changes

- Moved unchanged: `leaves_cover_the_visible_surface_exactly_once` and `culling_stays_conservative_for_known_hard_cameras` (LOD-003) to `crates/frame/tests/oracle_lod_coverage.rs` (shared helpers in `tests/support/lod.rs`); `morph_closes_the_gap_between_levels_on_steep_terrain_in_every_scripted_view` (TEST-015) to `oracle_steep_morph.rs`; `an_item_edit_rebuilds_exactly_the_tiles_whose_footprint_and_margin_hold_its_cell` (STRM-004) to `crates/cache/tests/oracle_tiles_item_edit.rs`.
- No Verify line changes: LOD-003 keeps tier-A tests (balancing, hand-built trees), STRM-004 keeps tier-A tests (verify, app, xtask), TEST-015 is Verify C.
- `cargo xtask test fast`: 452 tests in 7.1 s (8.1 s elapsed), was 15.8 s.
