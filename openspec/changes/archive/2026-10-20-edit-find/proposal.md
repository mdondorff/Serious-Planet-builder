## Why
Window 15 (2026-10-20): after the preview fix the preview time (12.7 to 32 ms against 16 ms) is the edit call (11 to 27 ms) plus a frame. The scene's find was its own scan of the resident tiles with a footprint query each, not the find the editor makes (`planet_editor::feed::edited_tiles`, a cap test per resident tile). Two things to repair: the scene must time the editor's find, and the editor's find must not cost a cap per resident tile (6,800 to 12,500 of them).

## What Changes
- **TEST-041 modified**: the scene makes the find as the editor does, and the stages table adds the find call and the instant of the first exact draw plan (`DrawKind::Exact`) after the edit, to tell the owner how far ahead of the idle frame the drawn tiles are exact.
- **`edited_tiles`**: coarse to fine; a tile that no edit can touch even through a cap three times as wide takes the tiles under it out of the search. The set returned is the one the scan returned (a test compares the two over a plan-shaped resident set); `HashSet` is only asked for membership (CON-14).

## Impact
- Spec: TEST-041 (modified, testing). Code: `crates/editor/src/feed.rs`, `crates/app/src/editperf.rs`, tests `crates/editor/tests/interaction.rs`, `crates/app/tests/gpu_edit_perf.rs`. No format, budget or golden changes. Drawn tiles are refreshed first already (the draw class precedes the residency class in the streamer's queue, STRM-026), so no ordering change is made.
