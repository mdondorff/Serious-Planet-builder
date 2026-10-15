## ADDED Requirements

### Requirement: STRM-018 Prefetch along travel
The prefetch set (STRM-005 class 3) SHALL be computed from the camera's recent translation only, on the injected clock: the position history of the last `window_ms` gives a velocity, the camera is predicted `look_ahead_ms` ahead along it, and the set is R of the predicted position minus R of the current position, ordered coarse before fine and nearer to the predicted position first. The prediction SHALL be a pure function of the sequence of `(time, position)` samples: no view direction, yaw or pitch rate enters it, so a rotation at a fixed position (zero velocity exactly) and a stationary camera produce an empty set and no request. The predicted displacement SHALL be capped (`max_distance_m`), a descent SHALL NOT be extrapolated below `min_altitude_fraction` of the current altitude, and a speed below `min_speed_mps` SHALL predict nothing. A step between consecutive samples larger than `teleport_base_m + teleport_altitude_fraction * altitude` SHALL be a teleport: the history is cleared and nothing is extrapolated from the old direction. The set SHALL be computed on the plan worker after R, on the plan's cache, never on the frame, and SHALL be a pure function of the two positions (the same for any thread count, CON-14). The managed loop SHALL hand the manager only as many missing prefetch tiles as there are free slots in the main pool (tiles already resident cost nothing), so a full share makes no prefetch request; the free count is that of the end of the previous iteration, so the bound that holds at every instant is the streamer's own (prefetch dispatches only into a free slot and the manager never evicts R or D for it, STRM-005). When the prediction changes the manager cancels the prefetch requests that left the set (`cancelled_prefetch`). The constants (`PrefetchConfig`: look-ahead 2000 ms, window 400 ms, minimum speed 0.5 m/s, distance cap 50 km, minimum altitude fraction 0.5, teleport step 500 m plus 10 % of the altitude) are conservative starting values; ADR 0031's owner question 2 (how far ahead) SHALL be decided from the first dive and teleport measurements by changing these constants only.

Verify: A
Status: active
Source: ADR 0031 points 2.3, 3 and 5, ADR 0032, STRM-005, STRM-015, STRM-017, CON-03, CON-14

#### Scenario: Rotation prefetches nothing
- **WHEN** a camera with its prefetch source on turns 360 degrees at a fixed position, R resident
- **THEN** the velocity is exactly zero, the prefetch set is empty and no request is made or cancelled

#### Scenario: Travel prefetches ahead
- **WHEN** the camera travels in a straight line at 600 m/s over a fixed script with the same machine speed, with and without the prefetch source
- **THEN** with it at most half as many draw-set tiles are first seen missing as without, and more are already resident when they first appear

#### Scenario: Free share only
- **WHEN** the share is too small for R and the camera travels
- **THEN** in every frame the missing prefetch tiles wanted number at most the free slots, none while no slot is free, and no frame is over the share or blank

#### Scenario: Direction change and teleport
- **WHEN** the camera turns round, or jumps farther than the teleport step
- **THEN** after turning round the queued prefetch requests that left the set are cancelled; after a jump the velocity is reset, nothing is predicted and the prefetch wants are empty within three frames

#### Scenario: Off the critical path and deterministic
- **WHEN** a frame is begun with a predicted position
- **THEN** it evaluates no height field; the set arrives with R from the worker and equals R-ahead minus R-here, identical on 1, 2 and 8 threads

## MODIFIED Requirements

### Requirement: STRM-017 Managed frame loop
The wants of the managed loop gain the prefetch set of STRM-018 (empty without a prefetch source); `--stream` has one camera and no travel, so its prefetch set is empty. The full text is in `openspec/specs/streaming-cache/spec.md`.

Verify: A, C
Status: active
Source: ADR 0031, STRM-018
