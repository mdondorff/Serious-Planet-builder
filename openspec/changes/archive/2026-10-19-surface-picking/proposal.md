## Why
M6 step U2 (docs/briefings/m6-editor-plan.md): hover and selection in the editor (U3) need the surface point under the cursor and the item whose footprint holds it, before the editor panels exist. The answer must not depend on the tiles in view (resident level, pop-in), so it runs against the height field in f64 on the CPU.

## What Changes
- `planet_frame::pick` (new, GPU-free): `cursor_ray`, `pick_surface` (sphere tracing with a slope bound in the shell around the radius, then bisection), `Footprint`, `pick_footprint`, `surface_distance_m`.
- VIEW-012.

## Limits
- The height field is any `Fn(Vec3) -> f64`; the caller gives the shell (`max_height_m`) and the slope bound. A slope beyond the bound can be stepped over (a thin spike); the default bound is 2.
- Items have no footprint radius in the world definition yet (E2 stamps add it); the picker takes `Footprint`s the caller builds.
- Wiring the picker to the window's cursor and the hover outline is U3.

## No golden moves
Only a new GPU-free module and its tier-A tests.
