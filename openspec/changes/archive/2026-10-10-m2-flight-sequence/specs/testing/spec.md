## ADDED Requirements

### Requirement: TEST-013 Flight sequence for the real-GPU review
`planet test-render --scene terrain --flight N --out-dir DIR` SHALL render a scripted, deterministic flight (a log-spaced dive from 20,000 km to 1 m above the terrain, then a glide at 1 m in steps of 5 cm) through the persistent terrain renderer with production LOD parameters at a chosen size, on any adapter, writing `frame_NNNN.png` and an index `flight.txt` (frame, altitude above terrain, node count, plan hash). `cargo xtask test gpu --real` SHALL also run the tier-B oracles on the real adapter. The owner reviews the sequence from the reference GPU once for jitter and vertex swimming (M2 gate).

Verify: C
Status: active
Source: CON-13, CON-21, ADR 0009

#### Scenario: Deterministic sequence
- **WHEN** the flight runs twice on one adapter
- **THEN** every frame and the index are identical, the index starts at 20,000 km and ends at 1 m, and consecutive glide cameras move between 2.5 and 10 cm

#### Scenario: Bad arguments
- **WHEN** fewer than 10 frames, no output directory, another scene, or a size whose LOD parameters fail validation are given
- **THEN** the run fails with an actionable message before writing frames
