## MODIFIED Requirements

### Requirement: STRM-005 Cancellation and prioritisation
Requests SHALL be in three classes, highest first: the missing nodes of the draw set D (ancestors before descendants; within the class by screen-space error times visibility), the rest of the residency set R (coarse before fine, nearer before farther; the view direction may order within this class but not change its membership), and prefetch (R at a position predicted from the camera translation only; yaw and pitch rates are never extrapolated). Prefetch SHALL be requested only while no request of the other two classes is waiting or running, and only into free capacity, and SHALL never evict a tile of R or D. A queued or in-flight request SHALL be cancelled only when its tile is in none of D, R and the prefetch set (with their ancestors); leaving the view frustum is never a reason. When the prefetch set changes (the travel direction changed), its requests that are no longer in it are cancelled first. An in-flight job SHALL stop at its next stage boundary, and the tile of a cancelled job SHALL NOT become resident (a cancelled job is never revived: if its tile is needed again it is requested anew after the job ends). The order of requests SHALL be a pure function of the inputs, independent of the thread count (CON-14).

Verify: A
Status: active
Source: report §4, ADR 0031 points 2, 3 and 6, CON-04, CON-14

#### Scenario: Rotation requests nothing
- **WHEN** the camera turns through 360 degrees at a fixed position with R fully resident and fitting the capacity
- **THEN** nothing is requested and nothing is cancelled (while R is still filling, a rotation only reorders queued requests)

#### Scenario: Cancel
- **WHEN** a node leaves R before its job starts
- **THEN** the job is cancelled and never runs, and a node that only leaves the frustum is not cancelled

#### Scenario: Order
- **WHEN** D, the rest of R and a prefetch set are all missing
- **THEN** D is dispatched before the rest of R and that before prefetch, ancestors before descendants in D and coarse before fine in R

#### Scenario: Prefetch is translation only
- **WHEN** a prefetch position is predicted for a camera with a velocity and any yaw or pitch rate
- **THEN** it depends on the position, the velocity and the look-ahead only, and prefetch requests wait for free capacity and for the higher classes

#### Scenario: Travel change
- **WHEN** the prefetch set changes while prefetch requests are queued
- **THEN** the queued requests outside the new set are cancelled and the others stay

#### Scenario: Stage boundary
- **WHEN** an in-flight job is cancelled
- **THEN** it stops before its next stage and its tile does not become resident

## ADDED Requirements

### Requirement: STRM-015 Residency set
The residency set R SHALL be a pure function of the camera position, the LOD parameters (LOD-008 distances and per-node height bounds) and the horizon cut. The view direction is not an input: yaw and pitch do not change it. R SHALL hold the selected leaves in all directions and all their ancestors, in ascending tile-id order, each with a distance, and SHALL contain the draw set (LOD-007) of every view from the same position.

Verify: A
Status: active
Source: ADR 0031 points 1 and 6, LOD-007, LOD-008, CON-03

#### Scenario: Yaw independent
- **WHEN** the draw set is selected from one position for many yaw angles
- **THEN** every draw-set tile is in R, and R is identical for all of them

#### Scenario: Closed under parents
- **WHEN** R is computed
- **THEN** every ancestor of every member is a member, in ascending tile-id order
