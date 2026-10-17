## ADDED Requirements

### Requirement: STRM-024 Helper threads of a tile job keep the job's priority
Threads a tile job starts (the parallel block reads of `WaterBook::fill`) SHALL set the priority the job runs at before they read; a new thread starts at normal priority and would otherwise take cores from the frame thread that the below-normal workers leave it (STRM-020). A platform without priorities leaves them at normal.

Verify: A
Status: active
Source: perf window 7 dive frames 65-112 (spikes of 53-61 ms with 36 jobs in flight), STRM-020

#### Scenario: A fill in a below-normal job
- **WHEN** a thread at below-normal priority fills the water of a tile
- **THEN** every field query of the fill runs on a thread at below-normal priority
