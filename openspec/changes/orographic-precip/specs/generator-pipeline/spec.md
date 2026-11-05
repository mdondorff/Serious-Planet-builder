## ADDED Requirements
### Requirement: GEN-117 The orographic climate
A relief-aware climate SHALL give, at a direction, a wind vector for two seasons, the annual precipitation as the sum of the two seasons' orographic precipitation, the aridity `P / PET` with `PET = 300 + 45 T`, and a seasonality value in `[0, 1]`: CPU-canonical (`libm`, integer hashing, a fixed number of order-independent iterations, f64, the same bits for any thread count), built in memory from the class source's base elevation, never stored, keyed by `OROGRAPHY_VERSION`. Precipitation SHALL be higher on the windward side of a range than on its lee. The global field of a class-source world SHALL take its precipitation from it, and `GLOBAL_VERSION` SHALL be bumped.

Verify: A
Status: active
Source: owner 2026-11-05, ADR 0104, SPIKE-102 F2 and F3

#### Scenario: Windward and lee
- **WHEN** the field is built on 1 and 8 threads over a synthetic ridge across the wind and over the class source, and read across a face edge
- **THEN** the bytes agree for any thread count, the windward flank is wetter than the lee by a ratio above 1.5 on the ridge, more iterations change the annual precipitation by under 2 %, an interior far from the sea is drier than a coast at the same latitude, seasonality stays in `[0, 1]`, the aridity equals `P / PET`, and the value is continuous across a face edge

### Requirement: GEN-130 The orographic climate preview
`generate --climate-preview DIR [--seed N | --world F] [--km N] [--fine-lattice N]` SHALL write, with no GPU, equirectangular maps of the annual precipitation with the annual wind alone and with two seasons, their difference, the aridity, the seasonality and the wind of both seasons with arrows, the material colour at four regions, and `orography.txt` with the land mean and quantiles of the precipitation, the land share by aridity class and by latitude band, and the windward over lee precipitation ratios across mountain belts (also on a lattice of `--fine-lattice` cells a face edge). The text and the run's line SHALL say the images show the climate, an input, not terrain; the bytes SHALL not depend on the thread count.

Verify: A
Status: active
Source: owner 2026-11-05, ADR 0104 point 5, planner inbox #75, GEN-117

#### Scenario: The preview
- **WHEN** the preview is made on 1 and 4 threads, written to a folder, and the option is combined with another generate command, a definition named twice, `--fine-lattice` alone or out of range
- **THEN** the images and the text agree for any thread count, the nine files exist, the text carries the label, the shares, the latitude bands and the ratios, and each refusal names the option
