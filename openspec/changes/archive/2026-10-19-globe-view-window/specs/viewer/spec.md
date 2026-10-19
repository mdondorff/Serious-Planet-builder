## MODIFIED Requirements

### Requirement: VIEW-013 Globe overview
Adds to the requirement: in the editor window (G, `GlobeSession`, `ViewerState.globe`) the overview is the camera: G shows it over the point under the camera now at 1.5 radii up and G again leaves it with the camera mode untouched; the left button drags it (a press and release within 4 px are a click), the wheel zooms by 0.9 per line, and the overlay draws each marker the camera sees (the one within 12 px of the cursor larger); a click names the marker within 12 px, else the surface under the cursor (`pick_surface`), and starts the flight to the marker's closing view or to 20 km over the spot; while it flies the overview takes no drag, zoom or click; the frame it ends the free camera takes over over the place, looking north and 0.5 rad down, with the flight clock paused; a click on the sky does nothing; `--marker LAT,LON[,RADIUS_M]` gives the markers. New scenario "The window runs the overview".

### Requirement: VIEW-014 Debug overlay and coarse LOD view
Adds the scenario "A refused coarse plan": `viewer::with_coarse_fallback` uses the base plan, turns the switch off and counts one refusal when the coarse plan fails and the base one works; returns an error at once when the coarse view is off; returns the second error when both fail; changes nothing when the coarse plan works.
