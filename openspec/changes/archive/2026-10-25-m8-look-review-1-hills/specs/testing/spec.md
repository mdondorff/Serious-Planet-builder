## MODIFIED Requirements

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

## ADDED Requirements

### Requirement: TEST-051 Anchor relief
The `[place]` table of the poses file MAY give `relief_radius_m` and `relief_min_m` (together) and `relief_max_m` (default unbounded); the `largest-river` anchor SHALL then be the largest river among the cells whose coupled-run heights within the radius span `relief_min_m` to `relief_max_m`. Giving one of the pair alone SHALL be refused.

Verify: A
Status: active
Source: opus-planner (2026-10-25), look review 1 second site, TEST-050
