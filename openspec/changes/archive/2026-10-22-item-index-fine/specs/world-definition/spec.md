## ADDED Requirements

### Requirement: WDEF-014 The footprint query finds its candidates by reach class
The footprint query (WDEF-007) SHALL take its candidates from an index by reach class (`ReachIndex`). Items are filed in five classes by reach (largest reach 400 m, 1.6 km, 6.4 km, 25.6 km, 40 km), in cells whose nominal edge is at least twice the largest reach of the class. A query looks up the cells within the rings its cap needs, or takes the whole class when the lookups cost more than the entries, and checks each candidate against the cap on a widened chord before the exact touch test. The result SHALL equal the scan of every item's. The index is derived data, kept in step with every put and remove and rebuilt on a radius change.

Verify: A
Status: active
Source: opus-planner (2026-10-22, SPIKE-29 fix 4), WDEF-005, WDEF-007

#### Scenario: Equal to the scan
- **WHEN** 900 items of every reach class, two layers and two priorities, at random places across face seams, corners and the poles and in a cluster, are queried by point caps, small and large caps, whole-sphere caps and tiles of levels 4 to 20 with halos 0, 0.5 and 1, after the puts, after replacing 60 items, after removing 100 and after a change of the radius
- **THEN** every query gives the ids, in application order, that the scan of every item gives

#### Scenario: Near the cap
- **WHEN** 10,000 items of 300 m reach on a grid 2 km apart and five of 30 km are queried by a point cap, caps of 600 m, 5 km and 20 km, and a point over empty ground
- **THEN** the point cap looks at fewer than 400 entries, the caps at most 500, 1,500 and all, and the empty ground at most 10
