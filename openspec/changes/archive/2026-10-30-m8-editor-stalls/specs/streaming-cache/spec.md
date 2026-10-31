## ADDED Requirements

### Requirement: STRM-063 The main thread never makes the version of a chunk that is not ready
`ChunkManager::update_with` SHALL ask the source of chunk versions (GEN-082) only for chunks that are ready (their versions decide staleness, STRM-056) and for their near-edit flags. The version of a chunk that is not ready SHALL be made by its job, on the worker, before the job reads the disk cache. A frame that wants chunks it does not have, with an item in a stamp layer under the chunk stage, SHALL take no more than 100 ms in the chunk step on its own thread.

Verify: A
Status: active
Source: opus-planner (2026-10-27), owner test 2026-10-27 group A, VIEW-056

#### Scenario: New chunks under an edit
- **WHEN** the chunk step is called with chunks that are not ready, over a world with an item in a stamp layer, and the version source records the thread of each call
- **THEN** every call for a chunk that was not ready came from a worker, every call from the caller's thread was for a chunk that was ready, and the step took under 100 ms on the caller's thread
