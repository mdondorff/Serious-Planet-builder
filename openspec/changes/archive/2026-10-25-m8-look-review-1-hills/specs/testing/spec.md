## MODIFIED Requirements

### Requirement: TEST-050 Poses file
The poses file (TOML) SHALL hold the keys of the table; a bad value SHALL be refused naming the key and the value.

| Key | Meaning | Default |
|---|---|---|
| `[place]` `anchor = "largest-river"` | largest river, at least `min_height_m` high (0) | or `lat`, `lon` |
| `[place]` `relief_radius_m`, `relief_min_m`, `relief_max_m` | only cells whose coupled-run heights within the radius span min to max | none; radius and min go together; max unbounded |
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
