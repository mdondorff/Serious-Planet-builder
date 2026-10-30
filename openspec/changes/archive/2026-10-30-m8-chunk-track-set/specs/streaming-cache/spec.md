## MODIFIED Requirements

### Requirement: STRM-042 Chunk prefetch along travel
The chunks the plan would read along the camera's travel over the ground, predicted by STRM-018's velocity for the next 12 s, SHALL be requested below the wanted set in priority, as prefetch-class jobs, and a camera that only rotates, stands still or dives SHALL prefetch no chunk. The viewer and the flight scenes SHALL make that request every frame from the managed loop's travel estimator.

Verify: A, B
Status: active
Source: STRM-018, SPIKE-34 section 7, D7, opus-planner (2026-10-27, criterion 7: the request was never made; the lateral chunks; the plan's read set along the track), STRM-060

Detail: The set is the chunks whose centre lies within the reach of the plan now (the farthest wanted chunk centre from the camera) of a sample of the track, the samples half a chunk edge apart out to 12 s of travel at most 60 km; the nearest sample comes first, within a sample the nearest chunk. Chunks that are wanted are not asked again.

#### Scenario: Travel and rotation
- **WHEN** the camera travels at 100 m/s over a fixed script, then turns 360 degrees in place
- **THEN** the next chunk along the travel is requested before it is wanted, a wider reach of the plan gives a wider set, and the rotation requests none
