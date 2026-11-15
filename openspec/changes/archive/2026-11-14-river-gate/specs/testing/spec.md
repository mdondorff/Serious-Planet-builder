## ADDED Requirements
### Requirement: TEST-136 The river gate's measures on fixtures
Tier-A fixtures SHALL check the river gate (BUILD-055): a draining valley has no basin, canyon, erosion depth or lake; a pit on its river is a closed basin only from 0.5 m and the cell minimum (default 16), while SPIKE-114's reference counts a 0.3 m pit; a basin over D is a lake with no canyon below it, and the same basin breached is a canyon; a trench 60 m under its valley is a canyon, a surface lowered 60 m everywhere only erosion depth; half the relief reads 50 %; an H1 crossing is matched only by an L crossing of its direction within 2 km; the same input gives the same report; the xtask refuses bad arguments.

Verify: A
Status: active
Source: Lead 2026-11-14 (explorer #143), BUILD-055

#### Scenario: The pit limits
- **WHEN** a 25-cell pit 3 m deep lies on the river
- **THEN** it is one closed basin with a cell minimum of 4 or 16, none with 26, and a 9-cell or 0.3 m pit is none

#### Scenario: A store's border and the sea
- **WHEN** a burn ends at the edge of the counted tiles above a higher start surface, or the sea stands over the valley
- **THEN** the drained border leaves no basin there, and no cell under the sea is a river cell
