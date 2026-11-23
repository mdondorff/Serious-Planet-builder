## MODIFIED Requirements

### Requirement: TEST-048 Pose stills
`test-render --scene capture-stills --poses FILE --out-dir DIR` (a world source as in TEST-027; `--size WxH` default 2560x1440; `--views A,B` selects poses by name) SHALL draw each pose on the lit path over the base terrain of the world and write `DIR/<pose>.png` and `DIR/stills.txt` (pose, size, eye latitude, longitude and height, heading, pitch, sun hours, plan nodes, capped nodes). A bad file or call SHALL be refused before it draws. The tool is not a measurement: no gate, no timestamp query, no perf log. The eye SHALL be `altitude_m` over the ground of the world's field.

Verify: A, C
Status: active
Source: opus-planner (2026-10-24), M8-4 and M8-8 look reviews, TEST-027, ADR 0086, ADR 0085

#### Scenario: Two poses
- **WHEN** the scene runs on the software adapter over a small store with two poses on the ground
- **THEN** it writes one PNG of the requested size per pose and `stills.txt` with a line per pose, and the two images differ in their sun

#### Scenario: A bad call
- **WHEN** `--poses` is missing, `--views` names no pose, or `--poses` is given to another scene
- **THEN** the run is refused before it draws

### Requirement: TEST-050 Poses file
The poses file (TOML) SHALL hold the keys of the table; a bad value SHALL be refused naming the key and the value. A key the table does not list (`chunk_timeout_s`, `horizon_cell_m` and `coarse_horizon` of older files) SHALL be ignored.

| Key | Meaning | Default |
|---|---|---|
| `[place]` `anchor = "largest-river"` | largest river, at least `min_height_m` high (0) | or `lat`, `lon` |
| `[[pose]]` `name` | names the PNG | required |
| `altitude_m`, `back_m` | eye over the ground; behind the anchor | required |
| `heading_deg`, `pitch_deg` | clockwise from north; down | required |
| `sun_local_hours` | local solar time | required |
| `sun_declination_deg` | degrees | 0 |

Verify: A
Status: active
Source: TEST-048

#### Scenario: A bad value
- **WHEN** `heading_deg` is a string, or `pitch_deg` is missing, or a name is given twice
- **THEN** the file is refused naming the key and the value
