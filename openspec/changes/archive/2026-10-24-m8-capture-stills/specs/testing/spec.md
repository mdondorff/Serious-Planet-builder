## ADDED Requirements

### Requirement: TEST-048 Pose stills
`test-render --scene capture-stills --poses FILE --out-dir DIR` (a world source as in TEST-027; `--size WxH` default 2560x1440; `--views A,B` selects poses by name) SHALL draw each pose on the lit path over a world with its A-net chunk stage on and write `DIR/<pose>.png` and `DIR/stills.txt` (pose, size, eye latitude, longitude and height, heading, pitch, sun hours, chunks read, seconds waited). A bad file or call SHALL be refused before it draws. The tool is not a measurement: no gate, no timestamp query, no perf log.

Verify: A, C
Status: active
Source: opus-planner (2026-10-24), M8-4 and M8-8 look reviews, TEST-027, STRM-046

#### Scenario: Two poses
- **WHEN** the scene runs on the software adapter over a small store with two poses on the ground
- **THEN** it writes one PNG of the requested size per pose and `stills.txt` with at least one chunk read per pose, and the two images differ in their sun

#### Scenario: A bad call
- **WHEN** `--poses` is missing, `--views` names no pose, or `--poses` is given to another scene
- **THEN** the run is refused before it draws

### Requirement: TEST-050 Poses file
The poses file (TOML) SHALL hold the keys of the table; a bad value SHALL be refused naming the key and the value.

| Key | Meaning | Default |
|---|---|---|
| `[place]` `anchor = "largest-river"` | largest river, at least `min_height_m` high (0) | or `lat`, `lon` |
| `chunk_timeout_s` | wait per pose, seconds | 900 |
| `chunk_cell_m` | chunk cell, metres | 8 |
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

### Requirement: TEST-049 Stills wait for their chunks
A still SHALL be drawn only when every chunk read by the tiles of its plan is ready and the plan was made over the field that includes those chunks, so that no tile of the image is a stand-in (STRM-039); the eye SHALL be placed over the ground of that field. When the chunks are not ready within `chunk_timeout_s` the run SHALL fail naming the pose, the number of chunks missing and the first missing keys.

Verify: A, C
Status: active
Source: TEST-048, STRM-039

#### Scenario: Chunks that do not land
- **WHEN** a pose's chunks are not ready within `chunk_timeout_s`
- **THEN** the run fails naming the pose, the count of chunks missing and the first missing keys

#### Scenario: Landed chunks
- **WHEN** a pose is drawn
- **THEN** the chunk share of its plan has no stand-in node and the eye is `altitude_m` over the ground of the field with the chunks
