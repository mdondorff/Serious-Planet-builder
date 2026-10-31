## MODIFIED Requirements

### Requirement: VIEW-054 The shade view and the time of day of the lit viewer
J SHALL cycle `ViewerState::shade_view` through normal (`ShadeView::Lit`), horizon, shadow and, since M8-6 (GEN-074), soil depth, moisture and insolation; the lit frames SHALL draw that view unless the coupled lens (K) is showing. `[` and `]` SHALL move `ViewerState::sun_offset_h` by `-SUN_STEP_H` and `+SUN_STEP_H` (0.5 hours, wrapping at 24) and Backslash SHALL set it to 0. A lit frame SHALL take `sun_with_offset(look_sun, sun_offset_h)` for `LitPlan::sun_dir`: the look's declination with the sub-solar longitude moved west 15 degrees an hour (the convention of `shading::sun_direction`), the look's sun itself at 0; the editing light (H) SHALL win over it. Both states start at the look's own (normal view, offset 0), so every golden stays.

Verify: A
Status: active
Source: opus-planner (2026-10-25), owner: horizon shadows cannot be seen in the viewer; REND-038, VIEW-042

#### Detail
The title SHALL name a shade view other than normal, the local solar time at the camera of a moved sun (`local_solar_hours`: 12 when the sun is on the camera's meridian) while the editing light is off, and, on the lit path, a window without the horizon jobs with "no horizon shadows (open a store with a coupled run)". The usage and editor help texts SHALL name J, `[`, `]` and Backslash.

#### Scenario: Keys and sun
- **WHEN** J is pressed seven times, then `]` twice and `[` once, then `[` twice more, then Backslash
- **THEN** the shade view goes horizon, shadow, soil depth, moisture, insolation, normal, horizon; the offset is 0.5, then 23.5 hours, then 0

#### Scenario: A known pose
- **WHEN** the look's sun is noon on the equator at longitude 0 and a camera is at longitude 0
- **THEN** the camera reads 12:00; six hours later the sun is on the -y axis and the camera reads 18:00; a camera 90 degrees west of the first reads 12:00 for that sun; a sun of another declination keeps it

#### Scenario: The title
- **WHEN** the title is made at the start and with the horizon view and a sun six hours later
- **THEN** it adds nothing, and adds "shade view horizon" and "sun 18:00 local solar time"; with the editing light on it does not claim the time
