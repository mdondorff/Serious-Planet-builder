# G0 fixture: expectations (TEST-139)

The terrain is `g0_fixture.rs` (river-graph plan, G0 contract addendum section 2.7). This file is its contract-to-be: each feature, where it is, where its water goes and where its basin spills. Once G0 is agreed, the feature table becomes the expected graph; until then it is checked with today's routing (`route_rules`, round rules, epsilon 1 mm) and flood (`priority_flood_with`, graded, epsilon 1 mm), never with graph code.

## Grids
| Item | Value |
|---|---|
| Sphere radius | 814 x 24 x 2 / pi = 12,437.0 m |
| H1 grid | cube sphere, 24 cells a face edge (814 m at a face's centre), `6 x 24^2` cells |
| L grid | 8 L cells an H1 cell edge: 192 a face edge (101.75 m at a face's centre), `6 x 192^2` cells |
| Tiles | 64 L cells an edge, 3 x 3 a face; face 0's tile boundaries at longitude -15 and 15 on the equator |
| Cell order | face, row, column (`GlobalGrid::split`): id = face x n^2 + row x n + column |
| Sea level | 0 m; land is longitude -44 to 68.5, latitude -36 to 36, the sea floor -100 m outside it |
| Coordinates | degrees; longitude from +x towards +y; latitude towards +z; face 0 is +x, face 2 is +y, they meet at longitude 45 |

## Surfaces
| Surface | Definition |
|---|---|
| `B` | at L: valleys cut into a 1,100 m plateau along the four lines (bed plus 40 m a degree from the line), plus the ridge, minus the basin's cone |
| `z_b` | `B` at the H1 cell centres |
| `z_s` | `z_b`, the trunk's H1 row lowered to the trunk's bed (`min(z_b, bed)`), longitude -14.75 to 68: the cut that makes the trunk antecedent |
| `S0` | `B + C1(z_s - z_b)` at L; C1 is the Catmull-Rom read of each face's H1 grid (`RegionField`), clamped at the face's edge |

## Lines
| Line | From (lon, lat) | To (lon, lat) | Bed from, m | Bed to, m |
|---|---|---|---|---|
| trunk | (-14.75, 2.1) | (68, 2.1), the mouth | 490 | 1 |
| T1 | (-22, 28) | (5, 2.1), the explicit confluence | 800 | 373.3 |
| T2 | (40, -28) | (35, 2.1), a second confluence | 800 | 196.0 |
| W | (-15.25, 2.1) | (-44, 2.1), the west coast (the trunk's slope; a step into the sea) | 490 | 321.6 |

## Partition layouts
| Layout | Regions | Rule |
|---|---|---|
| one-region | 1 | every H1 cell in region 0 |
| two-regions | 2 | whole tiles, cut along face 0's tile line at longitude -15 (G0 contract section 11: through features 5 and 6): region 0 the tiles whose centre lies west of it, region 1 the rest. H1 cells take their tile's region, L cells their H1 cell's |

## Features
| # | Feature | Location | Expected outlet | Expected basin spill |
|---|---|---|---|---|
| 1 | Trunk and two tributaries | trunk from (-14.75, 2.1) east to (68, 2.1); T1 from the north, T2 from the south | the sea east of 68.5 on face 2 | - |
| 2 | Explicit confluence | T1 into the trunk at (5, 2.1); at L about 4.9 E, at H1 about 5.6 E | the trunk's | - |
| 3 | Antecedent river | the trunk crosses a north-south ridge at longitude 25 (300 m, half-width 1.2 degrees); `z_s` cuts it on the trunk's H1 row, `B` keeps it; S0 keeps a 291 m rise over its running minimum (a C1 trough west of the crest, then the crest) | the trunk's | - |
| 4 | Closed basin (protected lake) | a cone 200 m deep, radius 3 degrees, at (55, 14) on face 2 | the trunk's mouth, through its spill | at L: floor 372.1 m, spill 431.8 m at about (55.5, 11.1), south towards the trunk; 46 L cells (0.39 km2, the cells near the face corner are smaller) |
| 5 | Divide near a region boundary | the crest between W and the trunk's head on face 0's tile line at -15 (both heads 490 m, a quarter degree either side), the two-region cut | west side: the west coast (W); east side: the trunk's mouth | - |
| 6 | Reach across a tile boundary | tributary T1 across face 0's tile line at -15 (at L near 21.1 N), the two-region cut: a `tile_crossing` node (trunk reaches are never cut at tiles; the trunk also crosses the tile line at 15) | the trunk's | - |
| 7 | Reach across a cube-face edge | the trunk at longitude 45 (face 0 to face 2) | the trunk's | - |

## Recorded values
Measured by the TEST-139 test, recorded to three decimals; it fails when a value moves by more than 0.006 (metres, degrees, km2 or counts). Change a value only together with the fixture, in the same change.

| Key | Value |
|---|---|
| `h1.trunk.cells` | 23.000 |
| `h1.trunk.face_changes` | 1.000 |
| `h1.trunk.crosses_ridge` | 1.000 |
| `h1.T1.confluence_lon` | 5.625 |
| `h1.T2.confluence_lon` | 39.375 |
| `h1.W.outlet_lon` | -46.875 |
| `h1.divide.west_outlet_lon` | -46.875 |
| `h1.divide.east_outlet_lon` | 69.375 |
| `h1.divide.west_region` | 0.000 |
| `h1.divide.east_region` | 1.000 |
| `h1.trunk.region_changes` | 0.000 |
| `h1.W.region_changes` | 0.000 |
| `h1.bowl.depth_m` | 28.959 |
| `l.trunk.outlet_lon` | 68.672 |
| `l.trunk.face_changes` | 1.000 |
| `l.trunk.tile_changes` | 2.000 |
| `l.trunk.crosses_tile_at_15` | 1.000 |
| `l.T1.cut_crossings` | 1.000 |
| `l.T1.cut_crossing_lat` | 21.142 |
| `l.T1.tile_changes` | 4.000 |
| `l.T1.confluence_lon` | 4.922 |
| `l.T2.confluence_lon` | 35.391 |
| `l.divide.west_outlet_lon` | -44.297 |
| `l.divide.east_outlet_lon` | 68.672 |
| `l.trunk.max_lat_dev` | 1.175 |
| `l.ridge.s0_rise_m` | 290.521 |
| `l.ridge.s0_at_crossing_m` | 345.355 |
| `l.ridge.b_at_crossing_m` | 560.942 |
| `l.ridge.s0_crest_over_bed_m` | 90.253 |
| `l.bowl.floor_m` | 372.143 |
| `l.bowl.cells` | 46.000 |
| `l.bowl.area_km2` | 0.391 |
| `l.bowl.spill_m` | 431.845 |
| `l.bowl.spill_lon` | 55.547 |
| `l.bowl.spill_lat` | 11.080 |
| `l.bowl.outlet_lon` | 68.672 |
| `h1.trunk.outlet_lon` | 69.375 |
