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
| `z_s` | `z_b`, the trunk's H1 row lowered to the trunk's bed (`min(z_b, bed)`), longitude -29.75 to 68: the cut that makes the trunk antecedent |
| `S0` | `B + C1(z_s - z_b)` at L; C1 is the Catmull-Rom read of each face's H1 grid (`RegionField`), clamped at the face's edge |

## Lines
| Line | From (lon, lat) | To (lon, lat) | Bed from, m | Bed to, m |
|---|---|---|---|---|
| trunk | (-29.75, 2.1) | (68, 2.1), the mouth | 490 | 1 |
| T1 | (-5, 30) | (5, 2.1), the explicit confluence | 800 | 316.2 |
| T2 | (40, -28) | (35, 2.1), a second confluence | 800 | 166.1 |
| W | (-30.25, 2.1) | (-44, 2.1), the west coast | 560 | 1 |

## Partition layouts
| Layout | Regions | Rule |
|---|---|---|
| one-region | 1 | every H1 cell in region 0 |
| two-regions | 2 | region 0: H1 cells centred west of longitude -30 (a column boundary of face 0's H1 grid); region 1: the rest. L cells take their H1 cell's region |

## Features
| # | Feature | Location | Expected outlet | Expected basin spill |
|---|---|---|---|---|
| 1 | Trunk and two tributaries | trunk from (-29.75, 2.1) east to (68, 2.1); T1 from the north, T2 from the south | the sea east of 68.5 on face 2 | - |
| 2 | Explicit confluence | T1 into the trunk at (5, 2.1); at L about 4.9 E, at H1 about 9.4 E | the trunk's | - |
| 3 | Antecedent river | the trunk crosses a north-south ridge at longitude 25 (300 m, half-width 1.2 degrees); `z_s` cuts it on the trunk's H1 row, `B` keeps it; S0 keeps a 292 m rise over its running minimum (a C1 trough west of the crest, then the crest) | the trunk's | - |
| 4 | Closed basin (protected lake) | a cone 200 m deep, radius 3 degrees, at (55, 14) on face 2 | the trunk's mouth, through its spill | at L: floor 360.4 m, spill 420.6 m at about (55.6, 11.1), south towards the trunk; 45 L cells (0.38 km2, the cells near the face corner are smaller) |
| 5 | Divide near a region boundary | the crest between W and the trunk's head near longitude -30.9; the two-region boundary at -30 | west side: the west coast (W); east side: the trunk's mouth | - |
| 6 | Reach across a tile boundary | the trunk at longitude 15 (face 0, tile columns 1 to 2) | the trunk's | - |
| 7 | Reach across a cube-face edge | the trunk at longitude 45 (face 0 to face 2) | the trunk's | - |

## Recorded values
Measured by the TEST-139 test, recorded to three decimals; it fails when a value moves by more than 0.006 (metres, degrees, km2 or counts). Change a value only together with the fixture, in the same change.

| Key | Value |
|---|---|
| `h1.trunk.cells` | 27.000 |
| `h1.trunk.face_changes` | 1.000 |
| `h1.trunk.crosses_ridge` | 1.000 |
| `h1.T1.confluence_lon` | 9.375 |
| `h1.T2.confluence_lon` | 39.375 |
| `h1.W.outlet_lon` | -46.875 |
| `h1.divide.west_outlet_lon` | -46.875 |
| `h1.divide.east_outlet_lon` | 69.375 |
| `h1.divide.west_region` | 0.000 |
| `h1.divide.east_region` | 1.000 |
| `h1.trunk.region_changes` | 0.000 |
| `h1.W.region_changes` | 0.000 |
| `h1.bowl.depth_m` | 30.571 |
| `l.trunk.outlet_lon` | 68.672 |
| `l.trunk.face_changes` | 1.000 |
| `l.trunk.tile_changes` | 3.000 |
| `l.trunk.crosses_tile_at_15` | 1.000 |
| `l.T1.confluence_lon` | 4.922 |
| `l.T2.confluence_lon` | 35.391 |
| `l.divide.west_outlet_lon` | -44.297 |
| `l.divide.east_outlet_lon` | 68.672 |
| `l.trunk.max_lat_dev` | 1.175 |
| `l.ridge.s0_rise_m` | 291.796 |
| `l.ridge.s0_at_crossing_m` | 306.433 |
| `l.ridge.b_at_crossing_m` | 522.020 |
| `l.ridge.s0_crest_over_bed_m` | 90.323 |
| `l.bowl.floor_m` | 360.425 |
| `l.bowl.cells` | 45.000 |
| `l.bowl.area_km2` | 0.381 |
| `l.bowl.spill_m` | 420.552 |
| `l.bowl.spill_lon` | 55.547 |
| `l.bowl.spill_lat` | 11.080 |
| `l.bowl.outlet_lon` | 68.672 |
| `h1.trunk.outlet_lon` | 69.375 |
