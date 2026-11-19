## Why
- Step C of the river-graph plan needs cross-sections against GLO-30 at matched relief: the critique's discriminating criteria, judged by the owner (lead #150, 2026-11-19).
- SPIKE-119 fixed 15 river transects and a GLO-30 match rule, but drew them with a Python tool outside the crates; G2's runs need a kept tool on the chain layout (BUILD-060).
- SPIKE-119's transects were picked on the axis-noise start; reading 30's turned noise moved the rivers, so they are re-picked once on today's default start (lead #150a).

## What changes
- BUILD-061: `cargo xtask transects --chain DIR [--face F] [--out FILE]`. Per transect inside the chain's window: `S0`, the final surface and the matched GLO-30 valley on one vertical scale. Per profile: valley depth and width at 10 m and 50 m above the river level. A transect whose `S0` depth is under 10 % of its stored relief is marked stale. Writes a PNG and a `.txt` table.
- Data: `crates/cache/data/transects.csv` (the 15 transects, face L cells) and `crates/cache/data/transects-glo30.csv` (the matched GLO-30 profiles; derived data with the Copernicus notice).
- TEST-141: tier-A fixtures for the measures, the sheet, the table and the refusals.

## Impact
cache (`transect_sheet` module, `transect_sheet` example, data files), xtask (`transects.rs`, help); tests. No generator or renderer change.
