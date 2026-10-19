## Why
M6 step U4 had two halves: the decision logic (`planet_frame::globe`, VIEW-013, #351) and the wiring into the editor window. This is the second half: the owner can look at the planet from outside, see item markers, and click a place to fly there. opus-planner (2026-10-19) also asked for the missing tier-A test of the coarse-LOD refusal fallback (VIEW-014), which belongs to the same window code.

## What Changes
- `planet_frame::globe`: `GlobeSession` (the overview the window runs: enter over the camera's point at 1.5 radii, drag, zoom, click on a marker or on bare surface through `pick_surface`, the flight, the step that ends it), `landing`, and the constants (12 px hit radius, 4 px click slop, 20 km landing altitude, 0.5 rad pitch).
- `ViewerState`: `globe`, `radius_m`, `Command::ToggleGlobe` (G); the overview is the camera while it is shown, and the frame a flight ends the free camera takes over at the landing pose with the flight clock paused.
- The window: the left button drags, a press and release within 4 px click, the wheel zooms; the egui overlay draws the markers the camera sees (`ui::draw_markers`); `--marker LAT,LON[,RADIUS_M]` (repeatable) gives the markers; the smoke script shows the overview with a marker under the camera, drags, zooms and clicks it, and the summary counts the frames, the markers shown, the flights and the landings.
- `viewer::with_coarse_fallback`: the coarse-LOD refusal fallback of the window as a pure function, with its tier-A test (VIEW-014).
- VIEW-013 (the window paragraph and scenario), VIEW-014 (the refusal scenario).

## Limits
- Markers come from `--marker` until the item commands (E4) exist; the overview takes `Marker`s and does not know where items live.
- Terrain relief is ignored when a marker is projected (the overview is far away); the click on the surface uses the height field.
- The cursor is mapped from the window's pixels to the render size by a plain scale; a window whose aspect differs from the render size is off by the aspect difference.

## No golden moves
Window and plan code only; the overview draws the planet as any orbit view does. `cargo xtask test gpu` ran with no candidate.
