## Why
D2 (owner, 2026-11-28): regions bake one at a time, and the editor shows every ready region of the file together; a stale region drops out until it is baked again (S-Region-Authoring E2, lead's 14:45 order).

## What Changes
- `WorldTerrain::open_regions_in_memory`: several commits over one class-source base in memory. It uses `FixtureLattice::merge` (several exact rectangles over one coarse globe), `multiregion::MultiLayer` (each tile from its commit, one cache of `LAYER_CACHE_TILES` for all), `MultiLakes` (lake ids offset per commit) and `ReaderRiverSource::join`.
- `editor --regions FILE` without `--commit`: the ready regions open (`regionedit::ready_commits`); each opened region's time and each one that does not open are printed; the camera starts over the first.
- The open line agreed with explorer: `cargo run --release -p planet -- editor --world F|--seed N [--items DIR] --regions FILE`; the Sites tab's Open copies it.
- Known limit: two adjacent regions are solved apart, so their shared edge may step.

## Impact
- Specs: VIEW-079 added; VIEW-077 and TEST-166 modified.
- Code: `crates/app/src/multiregion.rs` (new), `fixturebase.rs`, `rivers.rs`, `worldfield.rs`, `regionedit.rs`, `window.rs`; tests in `lib.rs`, `fixturebase.rs`, `multiregion.rs`, `regionedit.rs`.
