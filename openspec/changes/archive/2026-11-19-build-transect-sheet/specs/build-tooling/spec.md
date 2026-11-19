## ADDED Requirements
### Requirement: BUILD-061 The cross-section sheet of a chain directory
`cargo xtask transects --chain DIR [--face F] [--out FILE]` SHALL draw, for every river transect lying inside a chain directory's window (BUILD-060's layout), the start surface `S0`, the chain's output and the matched GLO-30 valley across the river on one vertical scale, and SHALL write each profile's valley depth and its widths at 10 m and 50 m above the river level to a table beside the sheet.

Verify: A
Status: active
Source: Lead 2026-11-19 (spikes #150: step C's cross-section sheet; #150a: transects re-picked on today's default start, the stale flag), SPIKE-119 (the transects and the GLO-30 match rule), BUILD-060, TEST-141
Detail:
- inputs: `chain.txt` (its `window`, `cell_m`, `label`, `commit`; the face from its `switch window F,...` line, or `--face F`, 0 to 5), `s0.f32` and `final.f32` (each optional: a missing one is drawn and tabled as missing; a file of the wrong size is refused, naming it);
- data in the crate: `crates/cache/data/transects.csv` (window, id, face, the line's ends and the river point in face L cells, drained area, relief along the line, and `s0_depth_m`: `S0`'s depth on it at the pick, by this requirement's measure) and `crates/cache/data/transects-glo30.csv` (the matched GLO-30 profile per transect, heights relative to its river point, with the licence's notice for derived data and its liability sentence);
- the transects: SPIKE-119's rule, 3 a window on R1h, R3a, R3b, M1 and F1, picked on g1f8's prototype `S0` of each window (the whole window; drained area from `route_rules` on the plain flood, epsilon 1 mm, seed 7); a transect is drawn when its whole line lies inside the window with one L cell to spare; a window with none is refused, naming its face and box;
- a profile: bilinear samples every 0.5 L cells along the line, `n = floor(length / 0.5) + 1`, at the distance `d` from the line's middle;
- the river level: the lowest sample within 250 m of `d = 0`; the depth: the lower of the two sides' highest samples, less the river level (at least 0);
- the width at `h` (10 m and 50 m): the distance between the first sample above the river level + `h` on each side of the river point, linear between samples; open (`>` on the sheet, `N open` in the table: at least `N` m) when a side never rises that high;
- stale: `S0`'s depth under 10 % of its depth at the pick (`s0_depth_m`; the stored relief when the column is absent); no verdict for a pick depth of 0 or without `S0`; the panel's title ends `STALE` in red and the table's `s0` line ends `stale`;
- the sheet: a title band (label, window, commit, and the GLO-30 licence's notice for derived data with its liability sentence, in full), then one panel per transect: `S0` grey and the final surface orange relative to `S0`'s river level, GLO-30 blue dashed relative to its own river point, one vertical scale per panel with its exaggeration printed, and one text row per profile (depth, widths in km);
- output: `target/review/stages/transects/<label>.png` by default (every character of the label but a letter, a digit or `-` written as `-`; `transects.png` without a label) and the table beside it (`.txt`): `TRANSECT label window id profile river_m depth_m width10_m width50_m`, the label's spaces as `-`;
- the command runs under the heavy lock (BUILD-026), as `stages --chain` does; an unknown option, a missing value, a second `--chain` or no `--chain` is refused.

#### Scenario: A V valley's depth and widths
- **WHEN** a chain directory's `S0` is a V valley of 10 m a cell up each side and 100 m cells, a transect crossing it over 56 L cells
- **THEN** its profile has 113 samples, the river level is the floor at `d = 0`, the depth is 280 m, the widths at 10 m and 50 m are 200 m and 1,000 m, and it is not stale against a stored 300 m (TEST-141)

#### Scenario: Stale, open and refused
- **WHEN** `S0` is flat under the same transect, one side of a profile never rises 10 m, `final.f32` is missing, and `chain.txt` has no face line
- **THEN** the table's `s0` line ends `stale`, the 10 m width is open, the final profile is tabled as missing, and without `--face` the command is refused naming `--face F` (TEST-141)
