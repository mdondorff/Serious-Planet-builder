## Why

Owner's decision (2026-10-12): the two relief goldens are rejected (too small to show a relief defect, they change with nearly every terrain change). The relief view is covered by the 1280x720 flight review at milestone gates plus an automatic property test.

## What Changes

- Removed `terrain_orbit_relief` and `terrain_ground1m_relief` from `selected_views_match_the_adapters_goldens`, `tests/goldens/classes.txt` and `tests/goldens/pending.txt` (no PNGs existed).
- Added tier-B test `the_relief_view_shows_terrain_below_the_horizon_with_grid_and_contour_lines` (REND-006, TEST-014): both cameras, no clear-colour pixels below the horizon, at least 20 thin-line pixels. Mutation-checked: with the grid and contour mixes set to zero it fails (0 line pixels).
- TEST-014 text names the flight review as the owner's review at milestone gates and the property test as its automatic check.
