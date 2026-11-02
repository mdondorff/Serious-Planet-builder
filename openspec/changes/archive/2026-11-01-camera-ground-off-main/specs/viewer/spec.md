## MODIFIED Requirements
### Requirement: VIEW-061 No main-thread phase waits for a bake block, and the stall report names the ones that do
The hover pick and the free camera's ground read SHALL each run on their own thread against a field it makes; a frame SHALL use the last finished read and start the next. A ground answer SHALL stand within max(1 km, half the camera's height) of the nadir; otherwise, or before the first answer, the frame SHALL read the ground itself. The bake cache SHALL keep the ground read's blocks out of eviction. A click that places or drags an edit SHALL pick on the spot. The stall probe SHALL count per phase the blocks the main thread decoded itself and the time it spent getting blocks; a stall report SHALL name each phase with a count above zero (CON-22).

Verify: A
Status: active
Source: opus-planner (2026-10-28, 2026-11-01), gate slow frames at frame-102263, VIEW-056, VIEW-059, CON-22, SPIKE-88

#### Scenario: A pick that blocks
- **WHEN** the hover pick of a frame is blocked inside the field for longer than the test waits
- **THEN** two frames run without waiting for it, the hit arrives on a later poll, and a pick that started before the pointer left the world is dropped when it finishes

#### Scenario: A phase that decodes
- **WHEN** a frame's phase reads a bake block nobody has decoded
- **THEN** the probe names that phase with the count of blocks the main thread decoded in it and the time it spent getting blocks, as in `blocks the main thread decoded in camera, ground heights and the globe overview: 4 (1050 ms getting blocks)`

#### Scenario: The camera's ground off the main thread
- **WHEN** the free camera's ground is polled each frame inside the camera phase over a cold stored bake, and when its read is blocked (a slow decode) for ten frames
- **THEN** the camera phase decodes no block on the main thread, every frame returns while the read is blocked, the first answer is the height at the nadir that was asked for, equal bit for bit to the field's own height there, and each later answer is for a request at most one behind the frame

#### Scenario: A jump reads the ground again
- **WHEN** the ground answer is for a nadir 0.9 km and 1.1 km from the camera's nadir at 2 m up, and 40 km and 60 km at 100 km up
- **THEN** it stands at 0.9 km and 40 km, and the frame reads the ground itself at 1.1 km and 60 km, as with no answer or a height that is not finite

#### Scenario: The camera's blocks are pinned
- **WHEN** the ground read's blocks are pinned in a stored bake's cache with a budget of 4 blocks, and four loader threads then read every block of the world
- **THEN** the budget holds, the loaders push blocks out, and the next ground read decodes nothing; with the pins removed the same pressure pushes the blocks out and the ground read decodes again
