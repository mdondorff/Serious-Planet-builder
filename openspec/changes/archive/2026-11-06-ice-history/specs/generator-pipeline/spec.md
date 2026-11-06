## ADDED Requirements
### Requirement: GEN-134 Ice history
The glacial-maximum equilibrium line altitude SHALL be a field over the orographic climate: the sea-level temperature of the climate minus the cooling of the glacial maximum, less the temperature an equilibrium line needs at the rain there, over the climate's lapse rate. The needed temperature SHALL rise log-linearly with the rain, held to a range. Ground above the line SHALL carry ice by a smooth cover from the line to a ramp above it, and the sea none; where the line lies under the ground everywhere, the land is a continental sheet. Every value SHALL be a validated field of `IceParams` with Earth's values as default (ADR 0108); the field SHALL be deterministic and never stored.

Verify: A
Status: active
Source: owner 2026-11-05, ADR 0106 point 1, ADR 0108, planner inbox #76, #86, GEN-117

The Earth preset of `IceParams`:

| Parameter | Earth default | Kind |
|---|---|---|
| cooling | 6 C | Earth state |
| dry needed temperature, its rain | -10 C, 300 mm | Earth state |
| rise per decade of rain, range | 10 C, -12 to 2 C | calibration |
| cover ramp, land ramp | 200 m, 50 m | calibration |

#### Scenario: The line
- **WHEN** the needed temperature and the line are read over rain, sea-level temperature and lapse rate
- **THEN** the needed temperature is the dry value at the dry rain, near 0 C at 3,000 mm, monotone in the rain and held to its range; the line stands at the tropical sea-level temperature less the cooling over the lapse rate, lies under sea level where the sea level is cold, and falls with a colder sea level and with more rain

#### Scenario: The cover
- **WHEN** the cover is read below, at and above the line, at sea and over land under a line below sea level
- **THEN** it is 0 below the line and at sea, 1 from the ramp above the line over land, monotone and continuous between

#### Scenario: The field
- **WHEN** the field is read over a climate at 1 and 3 threads
- **THEN** the bits are the same, the cover is in `[0, 1]`, the line falls towards the poles, and the land under ice is of the glacial maximum's order

#### Scenario: Parameters
- **WHEN** a parameter is not finite, a rain or ramp is not positive, the cooling is negative or the range is reversed
- **THEN** construction refuses it

#### Scenario: The preview
- **WHEN** `generate --ice-preview DIR [--seed N | --world F] [--km N]` is run, and combined with another generate command or with `--world` and `--seed`
- **THEN** it writes the ice-extent map, the line map and `ice.txt` (the land share under ice against the glacial maximum of Earth, the line's quantiles, the mid-latitude highlands' share and the glaciated sites' cover), and refuses the combinations, naming them
