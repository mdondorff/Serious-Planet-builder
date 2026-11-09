# preview-overlay (VIEW-067, briefing item 16)

## What
- `planet_editor::preview_overlay`: the level-1 preview as plain data for the egui layer (projected trunk lines, watershed, changed flag, weight, the cap, the label), the latest-wins state and its removal on commit or undo; GPU-free like `overlay`.
- `planet::panels::draw_preview_overlay` draws it, coloured by watershed; `EditRuntime` holds the state (`submit_preview`, `accept_preview`, `preview_overlay`) and clears it where the commands' affected places are taken.
- Tier A tests in the editor, the window and the panels. No render, frame or shader path changes, so no golden moves.

## The split (architect, 2026-11-09)
| Requirement | Tier | Content |
|---|---|---|
| VIEW-067 | A, active | the overlay as plain data and the egui draw function; does not claim the overlay is seen on a screen |
| VIEW-068 | A, C, planned | the producer job and its flag; the overlay drawn through the egui path as a debug view (CON-18); active only with a tier-C pixel assertion (overlay on against off, the change confined to the overlay's pixels; the form of `crates/render/tests/gpu_ui.rs`), not a new golden |
| Tier D | at the prototype review | the owner's look, on top of the tests |

VIEW-068 carries CON-18 (the overlay is a debug view).

## Not in this change
- The producer: the job that builds the resident base and runs `PreviewBase::preview` on an edit, and the flag that turns it on (next change, VIEW-068).
- The edit kinds other than stamps and climate regions; level 2's display.
