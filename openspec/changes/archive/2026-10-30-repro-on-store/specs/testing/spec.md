## ADDED Requirements

### Requirement: TEST-071 A bundle of a stored bake replays from the store
A terrain bundle that records a bake of more cells than a run makes in memory (1024 per face edge) SHALL replay with `--store DIR`, the stored bake of that size opened read-only, and SHALL be refused without it, naming the recorded size and `--store DIR`. The replay SHALL check the recorded definition hash, field id and radius as for any recorded world (TEST-008).

Verify: A
Status: active
Source: opus-planner (2026-10-30, B5), CON-19, TEST-008

#### Scenario: A bake beyond the memory limit
- **WHEN** a bundle records a bake of 4096 cells and is replayed without a store
- **THEN** the replay is refused naming 4096 cells and `--store DIR`, before any replanning

#### Scenario: From the store
- **WHEN** a bundle of a small bake is replayed with the stored bake of the same size and again with the bake made in memory
- **THEN** both worlds give the same heights bit for bit, and a directory that is no store is refused naming it
