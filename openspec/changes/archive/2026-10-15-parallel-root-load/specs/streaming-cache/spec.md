## MODIFIED Requirements

### Requirement: STRM-001 Injected time, I/O and spawning
Streaming and scheduling code SHALL receive time, I/O and task spawning through injected interfaces, and tests SHALL be able to run it with a fake clock, fake I/O with injectable latency and failures, and a deterministic single-threaded spawner. The start-up load of the six level-0 tiles SHALL hand all six jobs to the spawner before it waits for any, so a pool loads them concurrently; the tiles SHALL become resident in face order once all have arrived, so the resident set, its slots and its change order do not depend on the spawner, the thread count or the order the jobs finish (CON-14); if one fails, the others SHALL be cancelled, nothing SHALL become resident, and the error SHALL name the failing tile.

Verify: A
Status: active
Source: CON-04, CON-14

#### Scenario: Fake clock
- **WHEN** a scheduler test advances a fake clock by 5 s
- **THEN** exactly the jobs due within 5 s run, in a reproducible order

#### Scenario: Concurrent roots
- **WHEN** the six roots are loaded on the manual spawner and on pools of 1, 2 and 8 threads
- **THEN** all six jobs were handed over before the first wait, a pool of 6 has all six loads in flight together, and the resident set is identical in every case

#### Scenario: A failing root
- **WHEN** one root fails to load
- **THEN** the other roots are told to stop, no root is resident, the slots are given back and the error names the failing tile and its cause
