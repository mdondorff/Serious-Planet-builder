## Why
The owner asked for the keys in the editor's left panel (VIEW-055 made `frame::keytable` the single list of keys; F1 prints it as text). A panel can show the keys by group and the state each is in (shade view, ground detail, sun time, ...) beside what it does, without leaving the editor.

## What Changes
- A fifth tab `Keys` in the left panel (after Near, Layers, Graph, Problems; `keys` in the layout file's `tabs`).
- Its rows are `keytable::rows(true, viewer, sun_time)` computed by the window each frame: the keys the editor accepts, in table order, grouped Editing, Camera and movement, View; each row is the key, its state in brackets when it has one, and its help line.

## Impact
- Spec: VIEW-057. Tier A (the egui harness of `panels.rs`).
- Code: `editor/layout.rs` (the tab), `app/panels.rs` (`keys_tab`, `PanelData::keys`), `app/editwin.rs` (`set_keys`), `app/window.rs` (the rows each frame).
