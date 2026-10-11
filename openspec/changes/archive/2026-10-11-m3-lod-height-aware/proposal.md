## Why

First M3 change (owner, M2 gate go 2026-10-11): selection and morph measure distance to the reference sphere, so a camera low over high terrain gets coarse near tiles and, on steep ground, sits inside the drawn surface (learnings 2026-10-11; steep ground-1m: terrain 687.4 m, camera 688.4 m, nearest drawn vertex 689.5 m). Coverage metrics did not see it.

## What Changes

- `HeightField::height_bounds(centre, angular_radius)`: a rigorous `(min, max)` height over a spherical cap, computed per octave: the exact range over the box containing the cap, intersected with centre-value-plus-gradient and first-order Taylor bounds (the lattice-corner hull is the fallback for large caps); `Reference` and `Steep` implement it, the default is `+-max_abs_height_m`.
- `frame::lod`: a node's bounding volume is the sphere around its column at mid-height whose radius covers the horizontal extent and half the height range. Split and morph use the distance to that volume. The bounds are computed lazily (only for tiles that the global height range does not already exclude).
- The morph interval is per node: `end = split(parent)`, `start = max(0.7 end, split(level) + 2 rho)` with `rho` the largest volume radius among its four same-level neighbours. The shader measures the vertex distance to the vertex's real position (not the height-free `ref_pos`), so `ref_pos` leaves the vertex format.
- Tests: tier-A camera-above-drawn-surface check in every scripted view on the reference and steep fields; bounds soundness property tests.
- ADR 0016 (proposed) supersedes ADR 0014's height note.
- Modified: LOD-008 (planned -> active), LOD-004 (distance metric), REND morph requirement text where it names the reference position.
