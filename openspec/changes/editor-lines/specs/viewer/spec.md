## ADDED Requirements

### Requirement: VIEW-027 Point lists of lines and regions
`planet_editor::polyline` SHALL hold the geometry of the line and region tools on points given as latitude and longitude in degrees, in f64 with `libm`, without a window, a GPU or an item. A line SHALL have 2 to 64 points, each finite and within 90 degrees of latitude and 180 of longitude, no two consecutive points equal. A region SHALL have 3 to 64 such points, every point within one cell edge (WDEF-007, 40 km) of the first, and an outline that does not cross itself (a touch counts); a region beyond the reach SHALL be refused with the limit in the message, and one that crosses itself with the two edges. A line whose extent from its first point plus its width fits one cell edge, with at most 64 points, SHALL stay one part; a longer one SHALL be split into parts that each reach, width included, at most 80 % of the cell edge from their first point, hold at most 64 points, and share their boundary point exactly (the last point of a part is the first of the next), with a segment longer than half a part cut at great-circle points; a width that leaves no room SHALL be refused. A click, given as a unit direction, SHALL pick the nearest point within a distance, and the nearest segment (open, or closed for a region) with the foot of the click on it, an end point when the click is beyond the segment. Inserting, moving and removing a point SHALL return a new list and leave the input unchanged; removing SHALL be refused when fewer than the kind's minimum would remain.

Verify: A
Status: active
Source: opus-planner (2026-10-19), ADR 0069, docs/status.md "Editor MVP requirements"

#### Scenario: A long line
- **WHEN** a line of 150 km with points every 10 km and a width of 200 m is split
- **THEN** it becomes at least four parts, each of 2 to 64 points reaching at most 32 km, each sharing its first point with the last of the one before, and every original point is a point of some part

#### Scenario: A region too large, and a bow tie
- **WHEN** a square region of 50 km sides and a square with two corners swapped are checked
- **THEN** the first is refused naming the 40 km limit and the second naming the two edges that cross

#### Scenario: A click
- **WHEN** a click lands 300 m beside the middle of the second segment of a line, and another lands beyond its last point
- **THEN** the first picks that segment with a foot on the line 300 m away, the second picks the last segment with its end point as the foot
