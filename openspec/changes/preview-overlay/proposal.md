# preview-overlay (VIEW-067, briefing item 16)

## What
- `planet_editor::preview_overlay`: the level-1 preview as plain data for the egui layer (projected trunk lines, watershed, changed flag, weight, the cap, the label), the latest-wins state and its removal on commit or undo; GPU-free like `overlay`.
- `planet::panels::draw_preview_overlay` draws it, coloured by watershed; `EditRuntime` holds the state (`submit_preview`, `accept_preview`, `preview_overlay`) and clears it where the commands' affected places are taken.
- Tier A tests in the editor, the window and the panels. No render, frame or shader path changes, so no golden moves.

## Not in this change
- The producer: the job that builds the resident base and runs `PreviewBase::preview` on an edit, and the flag that turns it on (next change, VIEW-068).
- The edit kinds other than stamps and climate regions; level 2's display.
