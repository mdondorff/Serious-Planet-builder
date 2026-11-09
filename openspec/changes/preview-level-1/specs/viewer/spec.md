## ADDED Requirements
### Requirement: VIEW-067 The level-1 preview is drawn as an overlay
The viewer and the editor SHALL draw the trunk rivers and the watersheds of the level-1 preview as an overlay over the surface, marked as a preview, replaced when a newer preview is accepted and removed when the edit is committed or undone.

Verify: C
Status: planned
Source: Architect order 2026-11-09 (inbox #95), A3 11.1

#### Scenario: The overlay
- **WHEN** a stamp is dragged over a divide
- **THEN** the overlay shows the changed trunk rivers and watersheds marked as a preview, and a later preview replaces it
