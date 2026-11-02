## Why
Tracked owner bug (2026-10-23, RTX): pitch or yaw turns the sea glint into rectangles; a still camera restores it.

## What Changes
- Cause: `fs_water` gave the node's grid spacing to `water_waves`, whose resolution factor fades the waves a grid cannot hold. That is right for the vertex displacement; in the fragment stage it made the shading normal, and the glint, depend on the level of the tile that draws the pixel. A turn leaves stand-in tiles of coarser levels (quadrant draws) beside finer ones, so the normal jumped at each border.
- The fragment stage evaluates the waves with no grid limit (the pixel footprint still fades what a pixel cannot resolve); the vertex stage is unchanged.
- ATMO-013 states it; scenario "A tile's grid does not shape the normal"; tier C test `the_shading_normal_of_the_sea_does_not_depend_on_the_grid_spacing_of_the_tile_that_draws_it`.
- The recorded known-answer hash of the lit water frames moves (owner's /bless, ADR 0017); the stills of the owner package are written by the ignored `sun_glint_stills`.

## Capabilities
### Modified Capabilities
- `atmosphere-ocean`: ATMO-013
