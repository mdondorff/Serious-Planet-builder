## Why
E4 step 3: the command layer over the world definition, so the editor panels (U3) can put, change and delete items with undo and redo, and so the M6 exit criteria "undo over 1,000 operations" and "item files merge cleanly in git" are tier-A tests (docs/briefings/m6-editor-plan.md, ADR 0066 points 6 and 7).

## What Changes
- `planet_editor::ids`: `IdMaker` makes monotonic ULIDs from an injected `Clock` and `Entropy` (`SystemClock`, `OsEntropy`, `FixedClock`, `SeededEntropy`); within a millisecond the entropy is the last plus one, an overflow takes the next millisecond.
- `planet_editor::commands`: `Editor` over a `WorldDefinition` and an `ItemStore`: `put`, `change`, `delete`, `begin_edit` / `edit` / `end_edit` / `cancel_edit` (a drag is one command, its file written once), `undo`, `redo` (in-memory stack of at most 10,000 commands, files written through every time), `load` (the items of a body from the store, versions checked), and for each command the places to refresh (`Affected`: anchor and reach before and after) for the footprint find.
- AUTH-004 active with its new text.

## Notes
- A failed command changes nothing: the index refuses an item before it changes anything, a store failure puts the definition back, and a failed undo or redo stays on its stack.
- The window wiring (U3) and the call of `ManagedLoop::edit` with the tiles of the footprint find (#359) come with the editor panels.
- The tier-A git test needs `git` on the machine (CI has it); it fails with a message where it is missing.

## No golden moves
Editor-crate logic and tests only.
