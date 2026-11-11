## ADDED Requirements
### Requirement: VIEW-068 The level-1 preview is produced by the editor and drawn through the egui path
The editor window SHALL, behind `--preview-l1` (with `--preview-grid N`, 8 to 3,072 cells a face edge, default 768), produce the level-1 preview of the edit being made on a thread of its own: a held handle asks with the definition's stamps when their footprint hash changed, the newest ask wins, an invalid edit is an error that does not stop it, and the base is built once from the default class source. The accepted preview (VIEW-067) SHALL be drawn through the existing egui path as a debug view (CON-18): the frame with it on differs from the frame with it off in the overlay's pixels only. Without the flag nothing runs.

Verify: A, C
Status: active
Source: Architect order 2026-11-09 (inbox #99), A3 11.1, CON-18, VIEW-067; the tier-C test is a pixel assertion through the egui path (the form of `crates/render/tests/gpu_ui.rs`), not a golden

#### Scenario: The producer and the pixels
- **WHEN** a handle is held over a stamp with the flag on, with it off, and a frame is composed with the overlay and without it
- **THEN** the worker answers and the preview is drawn and gone when the edit is committed, nothing runs without the flag, and the two frames differ only where the overlay's lines and label are
