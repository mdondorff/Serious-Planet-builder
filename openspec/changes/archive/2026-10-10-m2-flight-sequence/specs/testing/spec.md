## ADDED Requirements

### Requirement: TEST-013 Flight sequence for the real-GPU review
`planet test-render --scene terrain --flight N --out-dir DIR` SHALL render a scripted, deterministic flight (a log-spaced dive from 20,000 km to 1 m above the terrain, then a glide at 1 m in steps of 5 cm) through the persistent terrain renderer with the production LOD defaults (cells 32, tau 6 px; overridable with `--cells`, `--tau`) at a chosen size on the adapter selected by `--adapter` or `PLANET_ADAPTER`, writing `frame_NNNN.png` and an index `flight.txt` (frame, altitude above terrain, node count, plan hash). Options a flight would ignore, and flight-only options outside a flight, SHALL be rejected before any GPU work.

Verify: C
Status: active
Source: CON-13, ADR 0009, ADR 0014

#### Scenario: Deterministic sequence
- **WHEN** the flight runs twice on one adapter
- **THEN** every frame and the index are identical, the index starts at 20,000 km and ends at 1 m, and consecutive glide cameras move between 2.5 and 10 cm

#### Scenario: Bad arguments
- **WHEN** fewer than 10 frames, no output directory, another scene, a conflicting option (`--perf`, `--perf-smoke`, `--repro`, `--out`, `--bundle`, `--stream`, `--tile`, `--script`, `--frames`, `--res`, `--manifest`, `--face-net`), a flight-only option without `--flight`, or a size whose LOD parameters fail validation are given
- **THEN** the run fails with an actionable message and creates no output directory

### Requirement: TEST-014 Real-GPU flight review
The owner SHALL review the TEST-013 flight rendered on the reference GPU once and confirm that it shows no jitter, vertex swimming, cracks or popping beyond the LOD transitions.

Verify: D
Status: planned
Review: `target/review/flight-nvidia/` (240 frames, 1280x720, tile-id view, RTX 3080 Ti) approved by the owner at the M2 gate
Source: CON-13, CON-18, ADR 0014

#### Scenario: Glide at 1 m
- **WHEN** the owner steps through the glide frames at 1 m
- **THEN** tile borders move smoothly with the camera and no vertex jumps between consecutive frames
