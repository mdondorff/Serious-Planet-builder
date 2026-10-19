## ADDED Requirements

### Requirement: VIEW-013 Globe overview
The editor's globe overview SHALL be an orbit camera, item markers on the screen and a flight to a clicked place (`planet_frame::globe`, GPU-free, f64 on the CPU, `libm` only; CON-03, CON-11, CON-14). A `GlobeView` is a unit direction (the point under the camera) and an altitude above the sphere of radius `R`; its camera sits over that point at `R + altitude`, looks at the planet's centre and has north up. The centre stays at least 1 degree from either pole (a view over a pole is moved to the edge of its cap; a zero or non-finite centre is +x; an altitude below 1,000 m or not finite is 1,000 m), so north stays up and the east axis is defined. A drag by `(dx, dy)` pixels turns the centre along the great circle in the drag's direction by exactly the angle its pixels span at the centre (`2 tan(fov/2) altitude / (height R)` per pixel; the surface follows the cursor, within 1 px of a flat surface at 100 km altitude) and keeps the altitude; a drag whose path would enter a cap stops at the cap's edge on the side it came from, so a view never flips; a viewport without height, a value that is not finite and a zero drag change nothing. Zoom multiplies the altitude and clamps it to `[1,000 m, 8 R]` (at least 1,000 m on a small planet); a factor that is not positive and finite changes nothing. A marker (id, unit direction, footprint radius) is shown when it faces the camera (the camera is above the tangent plane at the marker, so the horizon at `acos(R / (R + h))` from the centre point is the limit) and projects inside the viewport (`project`, pixel origin top-left); `visible_markers` returns them in the order given. The marker under the cursor is the nearest visible one within `hit_px` pixels, the radius inclusive, ties going to the lower id. The closing view of a marker is over it at four footprint radii, never below 1,000 m. A `GlobeFlight` between two views moves the centre along the great circle (spherical interpolation, the arc halved for an exactly opposite pair through the start's east axis) with a smootherstep time curve, interpolates the altitude geometrically and adds a rise that peaks mid-trip at `1.5 R` times the arc over `pi`; it starts and ends at the two views exactly (the time clamped to the flight, a time that is not a number the start; between, the centre stays outside the caps, so a flight over a pole bends round it) and lasts `0.8 s + 1.2 s` per radian of arc.

Verify: A
Status: active
Source: owner (M6 step U4; MVP scope 2026-10-18: 3D and globe views), VIEW-011, VIEW-012

#### Scenario: The view and the markers
- **WHEN** a view over any point (poles included) is built, and markers lie at the centre, on the far side, past the horizon (65 degrees from a camera at one radius up whose horizon is at 60) and 20 degrees north-east
- **THEN** the centre point projects to the middle of the viewport with north up and east right, the markers on the far side and past the horizon are left out and the other two are returned in order

#### Scenario: The click and the drag
- **WHEN** the cursor is within 12 px of one or of two markers, and a view is dragged by (50, -30) px at 100 km altitude or at the pole, and zoomed beyond both limits
- **THEN** the nearest marker is found (ties to the lower id, the radius inclusive), the old centre point appears at (850, 420) within 1 px, a drag at 85 degrees latitude north stops at the cap's edge without flipping the view, a drag of 300 px at 8 R turns the centre by exactly its angle, values that are not numbers change nothing and never panic, and the altitude stops at 1,000 m and at 8 R

#### Scenario: The flight
- **WHEN** a flight runs between two views 120 degrees apart at 2 km, between two nearby views, between the same place twice and between opposite places
- **THEN** every step is a unit centre with the angle from the start growing and the two arcs adding to the whole, the ends are exact and clamped, the long flight rises above 100 times its end altitudes mid-trip, the short one lasts under 1 s, and the opposite pair stays finite

#### Scenario: Click to fly
- **WHEN** a marker on the overview is clicked
- **THEN** the flight ends over it at four footprint radii (1,000 m at least)
