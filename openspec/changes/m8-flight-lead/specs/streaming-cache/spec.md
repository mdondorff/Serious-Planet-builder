## MODIFIED Requirements

### Requirement: STRM-042 Chunk prefetch along travel
The chunks needed by the camera predicted by STRM-018's velocity, one chunk ahead along the direction of travel, SHALL be requested below the wanted set in priority, and a camera that only rotates SHALL prefetch no chunk. The viewer and the flight scenes SHALL make that request every frame from the managed loop's travel estimator (`chunkstage::chunk_prefetch`). The track SHALL ask the chunks whose centre lies within the larger of the plan's reach and one chunk edge, plus half an edge, of its samples: the next chunk is asked on entering the current one, and a chunk is asked as early before it is wanted as the lead says.

Verify: A, B
Status: active
Source: STRM-018, SPIKE-34 section 7, D7, opus-planner (2026-10-26, criterion 7: the request was never made; 2026-10-27, the lead trace of window 14)

#### Scenario: Travel and rotation
- **WHEN** the camera travels at 100 m/s over a fixed script, then turns 360 degrees in place
- **THEN** the next chunk along the travel is requested before it is wanted, and the rotation requests none

#### Scenario: A low view
- **WHEN** the plan's reach is a fraction of one chunk edge and the camera has just entered a chunk
- **THEN** the chunk across the nearest edge is asked at once, and the set equals the one for a reach of one edge
