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
| `[[pose]]` `view` | a view of the viewer's J cycle (`lit`, `soil`, `moisture`, `insolation`, ...) drawn instead of the lit one | `lit` |
| `[look]` `ground_detail` | ground detail strength, 0 to 1, replacing the environment's | the environment's (0) |
| `[[look.curve]]` `material`, `input`, `lo`, `hi` | one curve of the ground materials (LOOK-008): `rock`, `sand`, `grass` or `soil`; `slope_cos`, `height`, `temperature`, `precipitation`, `surface_type`, `soil_depth`, `moisture` or `insolation`; replaces the pair | the look's |

Verify: A
Status: active
Source: TEST-048

#### Scenario: A bad value
- **WHEN** `heading_deg` is a string, or `pitch_deg` is missing, or a name is given twice, or a `view`, `material` or `input` names nothing, or `ground_detail` lies outside 0 to 1
- **THEN** the file is refused naming the key and the value
