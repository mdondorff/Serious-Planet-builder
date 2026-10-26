## MODIFIED Requirements

### Requirement: GEN-052 A-net chunk time
An A-net chunk of 24 km2 of core SHALL take at most 4 s at 8 m cells with the 3 km halo (1.8 million cells) on the reference machine with the stage's production thread setting (the viewer's chunk workers), measured by the performance harness through the architect on the production geometry (the stored bake) and never in a test (CON-20); the one-thread time and the times at 4 and 10 m are recorded with the run (4 m was about 55 s per chunk on one thread: not met, ADR 0077 allows 4 to 10 m).

Verify: C
Status: active
Source: SPIKE-33, M8 plan risks, CON-22, planner decision 2026-10-24 (8 m default), planner decision 2026-10-26 (production threads), perf window 2026-10-26, production geometry, 3.30/3.33 s at 4 threads

#### Scenario: Chunk time
- **WHEN** a chunk of 24 km2 at 8 m cells runs on the reference machine in a perf window on the production threads
- **THEN** it finishes within 4 s at 8 m cells and the log holds the one-thread time and the times at 4 and 10 m
