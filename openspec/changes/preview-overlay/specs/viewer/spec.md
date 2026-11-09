## ADDED Requirements
### Requirement: VIEW-068 The level-1 preview is produced by the editor and drawn through the egui path
The editor window SHALL, behind a flag, produce the level-1 preview of the edit being made (a job that builds the resident base once and runs `PreviewBase::preview` on each edit, the latest ask winning, VIEW-067) and the preview SHALL be drawn through the existing egui path over the planet. With the flag on, a frame with an accepted preview SHALL differ from the same frame with it off in the overlay's pixels only. The overlay SHALL be a debug view (CON-18).

Verify: A, C
Status: planned
Source: Architect order 2026-11-09 (inbox #99), A3 11.1, CON-18, VIEW-067; the tier-C test is a pixel assertion through the egui path (the form of `crates/render/tests/gpu_ui.rs`), not a golden

#### Scenario: Overlay on against off
- **WHEN** the same frame is drawn with a preview accepted and with the overlay off
- **THEN** the pixels differ only where the overlay's lines and label are, and the frame with it off is the frame without a preview
