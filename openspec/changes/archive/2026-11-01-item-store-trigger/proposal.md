## Why
ADR 0066 point 6 decides the item store's backend by measurement: more than 20,000 items in one body, or an item load over 2 s. Nothing counted or timed either, so the trigger could be reached without anyone knowing (Tracked item "Item store revisit trigger").

## What changes
- `planet_editor::store_watch`: the thresholds, a `LoadTimer` (wall clock; a fixed duration in tests) and `StoreWatch`, which keeps the item count per body and the last load time, issues one warning per crossing (the value, the threshold, "ADR 0066 point 6: revisit the backend") and gives the diagnostics line.
- `Editor` times each `load` and counts after every added or removed item (one comparison until the editor holds more than 20,000 items); the warning goes to the error stream and into the watch.
- The editor's status bar shows both values (`store: N items (limit 20000), last load T ms (limit 2000 ms)`), marked when one is over.
- The Tracked line is deleted from `docs/status.md`.

## Impact
- Spec: AUTH-006 (new, reserved by the planner 2026-11-01).
- Code: `crates/editor` (`store_watch.rs`, `commands.rs`), `crates/app` (`panels.rs`, `editwin.rs`). Tests use synthetic stores at 19,999, 20,000 and 20,001 items and an injected load time.
