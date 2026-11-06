## Why
ADR 0106 point 1: alpine relief, fjords and the glaciated reference sites are largely glacial. Pass-1 glacial erosion (point 2, a later change) needs the glacial-maximum ice extent, which needs the relief-aware precipitation of ADR 0104. This change is the ice-history field and its preview; the erosion law and the gate are separate changes.

## What Changes
- GEN-134: an analytic ice-history field over the orographic climate (GEN-117): the glacial-maximum equilibrium line altitude (ELA) from the climate's sea-level temperature, a cooling of about 6 C and the rain; the ice cover of ground above it. Never stored; `ICE_VERSION` 1.
- Continental sheets are not a second rule: where the sea-level temperature of the glacial maximum is under the temperature an equilibrium line needs, the ELA lies under the ground everywhere and the land is covered.
- The temperature an equilibrium line needs rises with the rain it receives (log-linear, held to a range): about -10 C at 300 mm (dry continental) and 0 C at 3,000 mm (maritime).
- `generate --ice-preview DIR`: the ice-extent and ELA maps, `ice.txt` with the land share under ice, the ELA quantiles, the mid-latitude highlands' share and the ice cover at the scorecard's glaciated sites.
- No stored format, no key and no existing output change.

## Earth-specific values (ADR 0108)
Every value is a field of `IceParams`, Earth as default (`IceParams::earth()`); a tuning changes the preset, not the kernel.

| Name | Default | Unit | Kind | Parameter |
|---|---|---|---|---|
| glacial-maximum cooling | 6 | C | Earth state (ADR 0106) | `lgm_cooling_c` |
| dry equilibrium temperature | -10 | C at 300 mm | Earth state (continental glaciers) | `ela_dry_temp_c`, `ela_dry_precip_mm` |
| rise per decade of rain | 10 | C | calibration (maritime 3,000 mm near 0 C) | `ela_temp_per_decade_c` |
| needed-temperature range | -12 to 2 | C | calibration | `ela_temp_range_c` |
| cover ramp above the line | 200 | m | calibration | `cover_ramp_m` |
| land ramp above sea level | 50 | m | calibration | `land_ramp_m` |

The lapse rate is the climate's own (the difference of `temperature_c` at 0 and 1,000 m), not a literal here.

## Capabilities
### Modified Capabilities
- `generator-pipeline`: GEN-134 (new)

## Known limits
- The climate's annual temperature falls with latitude as `30 - 55 sin^2 (lat)`: colder at mid latitudes than Earth's (about -2 C at 50 degrees against 7 C), so the ELA is low there and the mid-latitude highlands are almost all under ice; the owner's tuning round sets the cooling or the climate.
- The ice cover follows the base elevation of the climate lattice (about 20 km a cell), not the eroded ground.
- The scorecard's generator sites were matched on the old world and lie in the sea of the current one; the preview also reports the highlands as a class.
- Sheets are extents, not thicknesses or flow; ice below sea level is the bake's later concern (ADR 0106 point 2).
