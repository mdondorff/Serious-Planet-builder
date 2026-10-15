## MODIFIED Requirements

### Requirement: STRM-005 Cancellation and prioritisation
Requests SHALL be in three classes, highest first: the missing nodes of the draw set D (ancestors before descendants; within the class by screen-space error times visibility), the rest of the residency set R (coarse before fine, nearer before farther; the view direction may order within this class but not change its membership), and prefetch (R at a position predicted from the camera translation only; yaw and pitch rates are never extrapolated). Prefetch SHALL be requested only while no request of the other two classes is waiting or running, and only into free capacity, and SHALL never evict a tile of R or D. A queued or in-flight request SHALL be cancelled only when its tile is in none of D, R and the prefetch set (with their ancestors); leaving the view frustum is never a reason. When the prefetch set changes (the travel direction changed), its requests that are no longer in it are cancelled first. An in-flight job SHALL stop at its next stage boundary, and the tile of a cancelled job SHALL NOT become resident (a cancelled job is never revived: if its tile is needed again it is requested anew after the job ends). Dropping the streamer (a teardown) SHALL cancel every request that is not resident: the queued ones never reach the spawner, the dispatched ones are told to stop, a dispatched job that has not started SHALL end without calling the source, and a job in flight SHALL stop at its next stage boundary, so a teardown waits only for the jobs in flight and for their current stage. Dropping the plan pipeline SHALL close its worker: a plan job that has not started does not run, and a running one starts no residency set, prefetch set or follow-up plan after its own. A teleport is served by the same path: the requests that left R and the prefetch set while still queued are cancelled before they start. The order of requests SHALL be a pure function of the inputs, independent of the thread count (CON-14).

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

#### Scenario: Teardown
- **WHEN** the streamer is dropped with N requests queued, some dispatched jobs waiting in the spawner and k jobs in flight
- **THEN** no queued or waiting job calls the source, each of the k jobs in flight stops at its next stage boundary, and no tile becomes resident

#### Scenario: Teardown of the loop
- **WHEN** the managed loop is dropped with a plan job waiting in the plan worker's spawner, or with a plan job running and a newer request pending
- **THEN** the waiting job plans nothing, and the running job finishes its plan and selects no residency set and spawns no follow-up job

#### Scenario: Teleport
- **WHEN** the camera jumps while many requests of the old R are queued
- **THEN** the requests the new R does not contain are cancelled before they start, and every request started after the jump is in the new wants

#### Scenario: Stage boundary
- **WHEN** an in-flight job is cancelled
- **THEN** it stops before its next stage and its tile does not become resident

#### Scenario: Rotation through the loop
- **WHEN** the managed frame loop (STRM-017) runs a 360 degree turn at a fixed position with R resident, plans and R coming from the worker and tiles from fake I/O
- **THEN** every frame of the turn dispatches, cancels, queues, uploads and evicts nothing, the worker reuses the previous R instead of selecting one, and each settled frame equals the synchronous plan of its view

#### Scenario: Order through the loop
- **WHEN** the camera jumps and the loop fills the new place
- **THEN** every request of the new draw set with its ancestors precedes the first request of the rest of R, ancestors precede descendants, and every frame draws something with no blank node
