## ADDED Requirements

### Requirement: VIEW-061 No main-thread phase waits for a bake block, and the stall report names the ones that do
The editor's hover pick SHALL run on a thread of its own against a field that thread makes, and a frame SHALL use the last pick that finished and start the next, so that a block the pick is the first to read is decoded off the main thread; a click that places or drags an edit SHALL still pick on the spot. The stall probe SHALL count, per phase of a frame, the bake blocks the main thread decoded itself, and a stall report SHALL name each phase with a count above zero (CON-22).

Verify: A
Status: active
Source: opus-planner (2026-10-28), gate slow frames at frame-102263, VIEW-056, VIEW-059, CON-22

#### Scenario: A pick that blocks
- **WHEN** the hover pick of a frame is blocked inside the field for longer than the test waits
- **THEN** two frames run without waiting for it, the hit arrives on a later poll, and a pick that started before the pointer left the world is dropped when it finishes

#### Scenario: A phase that decodes
- **WHEN** a frame's phase reads a bake block nobody has decoded
- **THEN** the probe names that phase with the count of blocks the main thread decoded in it
