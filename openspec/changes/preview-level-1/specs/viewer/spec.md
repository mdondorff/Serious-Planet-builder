## ADDED Requirements
### Requirement: VIEW-067 The level-1 preview is drawn as an overlay
The editor SHALL turn the level-1 preview into plain data for the egui layer (`planet_editor::preview_overlay`, GPU-free, the camera's f64 projection): the trunk lines (a cell to its receiver) with their watershed, changed flag and discharge; only lines with both ends on the visible side and inside the viewport, the heaviest 20,000 with the rest counted; and a label marking the preview as not committed. A newer preview SHALL replace the shown one, the result of an older ask SHALL be dropped, and a committed or undone edit SHALL remove the preview. The window's draw function (`panels::draw_preview_overlay`) SHALL turn that data into egui lines coloured by watershed; this requirement does not show the overlay on a screen (VIEW-068).

Verify: A
Status: active
Source: Architect order 2026-11-09 (inbox #95, #99), A3 11.1, VIEW-020

#### Scenario: The overlay
- **WHEN** a line lies 400 m either side of the point below a camera 2 km up, another has an end on the far side, and ten lines compete for four places
- **THEN** the near line is at the pixels of the projection, the far one is not drawn, the four heaviest are kept with six counted, a newer preview replaces an older, a stale result is dropped, and a committed or undone edit removes the preview
