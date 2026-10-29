## ADDED Requirements

### Requirement: TEST-067 No point is drawn coarser than the frame before while the camera nears it, through the managed loop
Over a scripted slow approach run through the managed loop with its streamer and residency manager on manual spawners, no ground point that stays in the probe cap and nears the camera by more than 1 m SHALL be drawn at a coarser level than in the frame before. The approach SHALL be run twice: with tiles streamed 2 a frame, and under a tight pool that evicts. Each run SHALL prove it exercises the case: more than 20 frames that draw the cover or a stale plan instead of the exact plan, and at least one eviction.

Verify: A
Status: active
Source: owner live test 2026-10-19/20 (P1-4, P2-8), opus-planner (2026-10-29), STRM-016, LOD-007, TEST-018

Detail: 2000 probe directions within 0.45 rad of the dive direction; the drawn level of a probe is the level of the drawn node whose tile contains it. The camera descends from 3000 km, 2 % per frame, turns 0.3 degrees per frame and looks 1.2 rad down. A failure names the frame, the probe, both levels and the tile. `cover.rs` holds the same rule over a hand-built lagging plan and residency (TEST-018).

#### Scenario: Slow streaming
- **WHEN** the camera dives over tiles that finish 2 a frame
- **THEN** no probe that neared the camera is drawn coarser than in the frame before, and more than 20 frames did not draw the exact plan

#### Scenario: Eviction
- **WHEN** the pool is tight enough to evict while the camera dives
- **THEN** no probe that neared the camera is drawn coarser than in the frame before, and at least one tile was evicted
