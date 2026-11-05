## ADDED Requirements
### Requirement: TEST-090 The streaming smoke's scheduler limit follows the run's frame time
The streaming smoke (TEST-035) SHALL judge the median of the scheduler step against the larger of 0.5 ms and 0.1 of the same run's frame-record median (`record_ms`), and a log without that median SHALL be judged against 0.5 ms; the other checks of TEST-035 (concurrency, jobs, resident slots, pool, upload median) are unchanged. This supersedes the scheduler bound of TEST-035.

Verify: A
Status: active
Source: oracle_stream_smoke failures on #1078, #1072 and #1180 (docs/status.md), planner 2026-11-05, TEST-035

#### Scenario: A loaded runner and a dear scheduler
- **WHEN** the findings are read from logs with a scheduler median of 1.516 ms and a frame of 132.7 ms, of 0.10 ms and 2.5 ms, of 1.0 ms and 2.5 ms, of 14 ms and 132.7 ms, and of exactly 13.27 ms and 132.7 ms, and from a log without a frame median at 0.6 ms
- **THEN** the first, second and fifth give no finding, the third, fourth and last each give one finding naming `scheduler_ms`
