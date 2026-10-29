## MODIFIED Requirements

### Requirement: TEST-061 The no-degradation gate counters and limits
The window SHALL, when `PLANET_GATE` is set, count the main-thread time of every frame, the tiles the plan holds back (ADR 0074), the horizon maps baked, the bake blocks the main thread decodes itself (VIEW-061) and the panics caught, and the gate SHALL fail the run on a frame over 100 ms, a held-back tile with no edit in reach, a decoded block, no horizon map over a lit run with a chunk stage, a caught panic, or a step between levels over the bound of M8 exit criterion 3. A failure SHALL name the counter, the frame, the phase or the tile IDs.

Verify: A, C
Status: active
Source: owner (2026-10-28), owner test 2026-10-27 findings 7, 11 and 16, ADR 0074, TEST-058, VIEW-061

Detail: A frame's time is its phases without the waits for the GPU, and the phase of the longest one is named. Held-back tiles are listed with their IDs and whether an edit is in reach; those under an edit are reported and not failed. The first 20 frames are not judged. On the software adapter (`PLANET_GATE_SOFTWARE`, the CI run) frame time, decoded blocks and horizon maps are reported and not judged, because its rendering shares the CPU with the main thread; held-back tiles and panics are always judged. The step between levels is measured by `cargo xtask gate` (the field-side `anet_steps_report`, levels above the entry level), not by the window.

#### Scenario: A held-back tile away from an edit
- **WHEN** the counters record a frame whose plan holds back 3 tiles and no edit reaches them
- **THEN** the verdict fails naming the 3 tile IDs, and the frame

#### Scenario: A held-back tile under an edit
- **WHEN** the counters record held-back tiles all within the reach of an edit
- **THEN** the verdict passes and the report lists the tiles as flagged

#### Scenario: A slow frame
- **WHEN** a frame takes 140 ms of main-thread time in the phase "chunk frame"
- **THEN** the verdict fails naming the frame, 140 ms and the phase

#### Scenario: A block decoded by the main thread
- **WHEN** the main thread decodes a bake block in the phase "plan and residency (begin_frame)" of frame 31
- **THEN** the verdict fails naming the count, frame 31 and the phase
