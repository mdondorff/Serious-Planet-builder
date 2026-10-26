## ADDED Requirements

### Requirement: STRM-044 The world loads the stored coupled run or makes it
A world SHALL take its coupled run (GEN-040) from the stored channel (STRM-043) beside its bake when that holds the run, and otherwise make the run and write it there; a world over an in-memory bake makes it and stores nothing. The run's lattice is `min(bake resolution, 2048)` cells per face edge and its input is the bake's elevation reduced onto that lattice (GEN-056) with a constant uplift and erodibility, and its spec (lattice, steps, bake resolution, parameters, identity of the input) is the key of STRM-043, so another bake, lattice, step count or parameter is another run and never reads the stored one; finding a stored run reads nothing of the bake. The load or run SHALL happen once on a thread, never in a frame (CON-22); until it is ready the fields stay plain, and a failure is a message naming the cause, never a default.

Verify: A
Status: active
Source: opus-planner (2026-10-24, M8-1 part c), ADR 0077

#### Scenario: Made once, then loaded
- **WHEN** the run of a small world is asked for twice from a store directory, then once with another step count
- **THEN** the first call makes and stores it, the second reads it back with identical maps, the third makes another run with another identifier, and a world without a store makes the run in memory
