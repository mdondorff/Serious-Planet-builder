## Why
Owner report 2026-10-29: a dragged edit looks different from the same edit at rest, everywhere. Cause (measured as a model in `crates/frame/tests/preview_drag.rs`): the editor keeps the oldest state until the exact tiles are in, and the preview delta always runs from it (REND-045, REND-033). A tile refreshed at an intermediate state of a drag already holds part of the edit, but the delta was applied to it too: a 60 m hill dragged 0, 300, 500 m east showed a 49 m dent at the old place and a 33 to 94 m excess elsewhere on such a tile. A named tile refreshed ahead of the plan had the same double count for the frames until the plan arrived.

## What changes
1. `ManagedLoop::set_preview` and the new `set_preview_list` record the key of every resident tile when the first non-empty list since the preview ended is set (the oldest state); `set_preview` with an empty list (the end of the preview) forgets them, an empty `set_preview_list` in the middle of a drag keeps them.
2. A stale node is flagged by the delta only while its resident key is its oldest key. Tiles refreshed at an intermediate state, loaded since, or refreshed ahead of the plan are drawn as they are until the exact tile arrives (a step behind the pointer, never wrong).
3. `PreviewList` carries the stamps the delta left out at the cap (`left_out`); a tile one of them touches is drawn without the delta.
4. The editor hands `preview_list` over (`window.rs`, `editwin.rs`); `edit-latency` does the same.

## Not changed
No shader, no golden, no cap (32 stamps, 16 for the delta). A tile at an intermediate state could be given its own delta if each tile were tagged with the definition state it was built from; the plan keeps that as a later step.
Hypothesis (a) of the owner's report (a ground-relative stamp reading ground that already holds the moving edit) was not examined.

## Tests
`tests/preview_drag.rs` (flags, the cap, the heights of a flagged and an unflagged tile), `tests/managed.rs` (two drag steps through the managed loop). REND-045, tier A.
