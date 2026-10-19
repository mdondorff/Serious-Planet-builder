## Why
M6 step U4 (docs/briefings/m6-editor-plan.md): the editor's globe overview with item markers, where a click flies to the place. The owner's MVP scope (2026-10-18) names a globe view beside the 3D view.

## What Changes
- `planet_frame::globe` (new, GPU-free): `GlobeView` (orbit camera, drag, zoom, closing view of a marker), `project`, `faces_camera`, `visible_markers`, `marker_at`, `GlobeFlight` (great-circle flight with a rise on long trips).
- VIEW-013.

## Staging
This change is the decision logic and its tier-A tests. The window wiring (a key to enter the overview, the egui markers over the lit planet, mouse drag and wheel, the click that starts the flight and hands the camera back to the free camera) follows as `globe-view-window` and shares `window.rs` with nothing else queued.

## Limits
- Markers sit on the sphere of radius `R`; terrain relief is ignored (the overview is far enough for it not to matter).
- Items have no list in the world definition for markers yet (E4 gives the item commands); the overview takes `Marker`s the caller builds.

## No golden moves
A new GPU-free module and its tests.
