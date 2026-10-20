## ADDED Requirements

### Requirement: TEST-044 The regional, ridge and region cases of the edit-latency scene
The `edit-latency` scene (TEST-019, TEST-041 to TEST-043) SHALL also run, for each altitude, regional stamps (hill stamps of the radii 5 km and 40 km, the largest an item can reach, `CELL_EDGE_M`) from the altitudes 2 km and 20 km, one ridge of 5 km (17 points, 30 m high, 200 m of half-width, straight, centred on the place the views look at) and one region of 2 km (a square, the layer `regions`, labelling only) from the altitudes 200 m and 2 km. The region case SHALL put the region with the surface type rock into the world first and edit its surface type to forest in place (the edit names its tiles by the region's anchor and reach, and the earlier state is taken out again after the repeats). The rows of these cases SHALL print the size as `<r> m` (the regional stamps), `<n> m ridge` and `<n> m region` in the budget, stages and refreshed-set tables, and their verdict SHALL read `recorded` with no budget (the regional stamps over 1 km, the ridge and the region are not gated; a number is set after measuring). A smoke run SHALL add the 5 km stamp, the ridge and the region at 200 m, and fail when an edit never becomes exact.

Verify: C
Status: active
Source: opus-planner (2026-10-20), M6 gate evidence G2 and G3, TEST-019, TEST-041, TEST-042, TEST-043

#### Scenario: The smoke run
- **WHEN** the scene runs in smoke mode on the software adapter
- **THEN** it prints rows `| 200 m | 5000 m |`, `| 200 m | 5000 m ridge |` and `| 200 m | 2000 m region |` in the budget table, each with an exact time and the verdict `recorded`, and `edit-latency.md` holds them
