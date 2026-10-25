## Why
The owner wants the keys visible in the app at all times and always current (opus-planner, 2026-10-25). The keys were in four places that drifted: the mapping in `window.rs`, `KEYS_HELP` in `frame/viewer.rs`, `EDIT_KEYS_HELP` in `app/editwin.rs`, and two label tables in the window tests.

## What Changes
- One key table, `frame::keytable::KEYS` (key names, label, action, group, mode, help, state): `key_command`, `edit_key`, `set_held`, the usage text, the F1 list and the rows of the panel are all made from it. `EditKey` moves there (re-exported by `editwin`); `KEYS_HELP` and `EDIT_KEYS_HELP` are generated statics.
- Tier A (VIEW-055): every command and every editor key has an entry and every entry is found by its own lookup; the modes list exactly what they accept; a key the window handles is in the table and a key the table lists is handled.
- The Keys tab of the editor's left panel is the next change; its rows are `keytable::rows(editor_on, viewer, sun_time)`.

## Impact
- Spec: VIEW-055.
- Code: `frame/keytable.rs` (new), `frame/viewer.rs`, `app/window.rs`, `app/editwin.rs`.
- The help text is now generated from the table's words: its sentences changed in wording, not in what they name; every word the older tests looked for is still there.
