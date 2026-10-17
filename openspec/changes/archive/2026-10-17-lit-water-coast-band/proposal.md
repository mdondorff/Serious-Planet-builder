## Why
Window 6 stalls (8 s frames, a 0.5 s first frame, a 3 s cold frame) were the water selection reading the store on the frame thread. The coastal captures showed a staircase of water over a dry flat plain to the horizon: the bake calls low land "land" while erosion cuts it below the level. The wave shading aliased at a far grazing pixel.

## What Changes
- ADR 0056 (proposed): the coastal band, tile water computed in the tile job, a pixel-footprint wave fade, beach-based coast views.
- ATMO-012 modified: the selection reads the book the tile jobs fill; the band rule.
- ATMO-013 modified: the pixel-footprint weight.
- ATMO-019 added: the coastal band (numbers, arithmetic, oracle) and the coast views.
- Code: `WaterBook`, `WaterSource::{band_at, ocean_bit_at}`, `coast_band_elevation_m`, `erosion::flat_ground_dip_elevation_m`, `find_coast` by height, shader `footprint`.

## Impact
No `MESH_VERSION`, key or golden change. The lit known-answer hashes of the water frames change (WARP and RTX) because of the pixel fade. The four `water_class_*` goldens are unchanged (their views use a test sea).
