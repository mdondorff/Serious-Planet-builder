## MODIFIED Requirements

### Requirement: TEST-013 Flight sequence for the real-GPU review
`planet test-render --scene terrain --flight N --out-dir DIR` SHALL render a scripted, deterministic flight (a log-spaced dive from 20,000 km to 1 m above the terrain, then a glide at 1 m in steps of 5 cm) through the persistent terrain renderer with the production LOD defaults (cells 32, tau 6 px; overridable with `--cells`, `--tau`) at a chosen size on the adapter selected by `--adapter` or `PLANET_ADAPTER`, in the `relief` view unless `--view` names another, writing `frame_NNNN.png`, an index `flight.txt` (frame, altitude above terrain, node count, plan hash, finest level) and `contact_sheet.png` (up to 12 frames evenly spaced from the first to the last, each labelled with its frame number and altitude). Options a flight would ignore, and flight-only options outside a flight, SHALL be rejected before any GPU work.

Verify: C
Status: active
Source: CON-13, ADR 0009, ADR 0014

#### Scenario: Deterministic sequence
- **WHEN** the flight runs twice on one adapter
- **THEN** every frame and the index are identical, the index starts at 20,000 km and ends at 1 m, and consecutive glide cameras move between 2.5 and 10 cm

#### Scenario: Bad arguments
- **WHEN** fewer than 10 frames, no output directory, another scene, a conflicting option (`--perf`, `--perf-smoke`, `--repro`, `--out`, `--bundle`, `--stream`, `--tile`, `--script`, `--frames`, `--res`, `--manifest`, `--face-net`), a flight-only option without `--flight`, or a size whose LOD parameters fail validation are given
- **THEN** the run fails with an actionable message and creates no output directory

#### Scenario: Contact sheet and index
- **WHEN** a 30-frame flight is rendered at 160x120
- **THEN** `contact_sheet.png` holds 12 thumbnails in 4 columns, and every index row has five fields with the finest level growing from the first frame to the last

### Requirement: TEST-010 Debug view per visual feature
Every visual feature SHALL ship with a flat-colour debug view and at least one tier-C test; debug views SHALL be independent of lighting and compare exactly. The terrain `relief` view (REND-006) is the one exception to flat colour: it uses a fixed sun so that frames stay legible at every altitude, and it still compares exactly.

Verify: C
Status: active
Source: CON-18

#### Scenario: Face view
- **WHEN** the face debug view is rendered
- **THEN** the six faces appear in six distinct flat colours with no background pixels inside the planet disc (checked from the LOD change in M2; M1 checks the two-tile case below)

#### Scenario: Tile views
- **WHEN** the face, tile-id and height views of two generated tiles are rendered
- **THEN** no clear-colour pixel appears inside a planned rectangle, the face view is exactly the face colour, and the images match the adapter's goldens (or are written as candidates)

### Requirement: TEST-014 Real-GPU flight review
The owner SHALL review the TEST-013 flight rendered on the reference GPU once, in the default relief view, and confirm that it shows no jitter, vertex swimming, cracks or popping beyond the LOD transitions.

Verify: D
Status: planned
Review: `target/review/flight-relief-nvidia/` (240 frames, 1280x720, relief view, RTX 3080 Ti) is rendered and waits for the owner. The earlier `target/review/flight-nvidia/` (tile-id view) approved at the M2 gate covered the dive; its glide frames were solid colour and showed nothing.
Source: CON-13, CON-18, ADR 0014

#### Scenario: Glide at 1 m
- **WHEN** the owner steps through the glide frames at 1 m
- **THEN** the grid lines and contours of the relief view move smoothly with the camera and no vertex jumps between consecutive frames

#### Scenario: Relief flight
- **WHEN** the owner reviews the relief flight rendered on the reference GPU
- **THEN** the owner confirms or rejects it, and the verdict is recorded in `docs/status.md`

## ADDED Requirements

### Requirement: TEST-017 Flight legibility
Every frame of a flight in its default view SHALL show structure: at least 10 % of its terrain pixels sit next to a clearly different pixel (a channel differs by at least 12). The check SHALL be shown to fail on the flat-colour `tile-id` flight, so that it cannot pass vacuously.

Verify: C
Status: active
Source: CON-13, CON-18, ADR 0014

#### Scenario: Measure
- **WHEN** a frame is measured
- **THEN** terrain pixels are those whose right and lower neighbours are not the clear colour either, and the check uses the production LOD parameters (cells 32) because the flat-colour views only turn solid when tiles are large

#### Scenario: Default view
- **WHEN** a 30-frame flight is rendered in the default view at 768x432 with cells 32
- **THEN** no frame, orbit or glide, falls below the structure threshold

#### Scenario: Negative control
- **WHEN** the same flight is rendered with `--view tile-id`
- **THEN** every frame of its 1 m glide falls below the threshold
