## MODIFIED Requirements

### Requirement: REND-017 No one-time initialisation inside a measured frame
The terrain renderer SHALL be able to do the one-time work of its first frames before the first frame (`warm_up`): create the small vertex pool and the first block of the main pool (block 0 of the vertex format, or atlas 0 with its bind group), grow the per-frame ring to a count above the node count of the standard scenes' plans (2048 nodes, within the share's node-record bytes), and submit one throwaway frame, polled to completion, that is representative of the measured frames: it records from every set of the per-frame ring, draws a zeroed tile through every pipeline of the tile format in the active draw mode (the indirect path as a multi-draw with a count above one), and runs the timestamp pass with its resolve and readback where the adapter supports timestamps, so that the lazy first-use work of the driver and of wgpu (pipeline state, resource initialisation) is done outside any measured frame. The throwaway frame SHALL make no tile resident, SHALL NOT count as a frame (the frame clock of the slot retirement and the frames in flight do not move) and SHALL NOT change any image. The bytes it allocates SHALL equal the pure model of the pool configuration with one block and the vertex pool live (STRM-006); every other block, atlas and ring growth stays on demand and the share stays a ceiling. The renderer SHALL count the long-lived GPU objects it creates (pipelines, pool allocations, ring builds); after `warm_up`, a script whose first frames fit the warmed block and ring SHALL create none in frames 0 and 1. The renderer SHALL also count the parts of itself a submission has used (ring sets, pipelines, timestamp passes, draw modes); after `warm_up` these SHALL be the maximum a scene's frames reach, and frames 0 to 3 (untimed and timed) SHALL use no part for the first time. The pipelined loop and the streaming perf scenes SHALL warm the renderer in their setup, outside the timed loop; frame 0 stays in the measured frames (ADR 0032, TEST-018) and records like the frames after it.

Verify: C
Status: active
Source: ADR 0032, REND-013, REND-016, TEST-018, STRM-006, perf windows 1 and 2 (F4, F9)

#### Scenario: Nothing is created in the first frames
- **WHEN** a vertex-format and a compact renderer, per-draw and indirect, are warmed and then upload a plan's tiles and submit frames 0, 1 and 2
- **THEN** the pipeline, pool-allocation and ring-build counters and the live blocks are the warm-up's, none became resident during the warm-up, and a renderer that was not warmed allocates in the same frames

#### Scenario: Allocation equals the model
- **WHEN** a renderer is warmed
- **THEN** one block and the vertex pool are live, and `pool_allocated_bytes` equals `allocated_bytes_with(1, true)` with the node records at their allocated size, under the share

#### Scenario: Same image
- **WHEN** the same plan is rendered by a warmed and by a cold renderer
- **THEN** the pixels are identical

#### Scenario: Nothing is first-used by a measured frame
- **WHEN** a vertex-format and a compact renderer, per-draw and indirect, are warmed, then submit frames 0 to 3 alternating untimed and timed
- **THEN** the warm-up used every ring set, every pipeline of the format, the timed pass where supported and the active draw mode in one waited submission, frames 0 to 3 change none of those counts, and a cold renderer reaches every ring set only through its first two frames
