Opened: 2026-10-29
Question: How do slope and local relief of the generator's terrain compare with real terrain (Copernicus GLO-30), per landform?
Why it matters: M9 realism targets (SPIKE-46 listed GLO-30 as the candidate source); the numbers say where the generator's terrain departs from real landforms.
Time box: 2 h
Success criteria: per landform, slope (p50/p90/p99 at 30 m) and local relief (1 km and 5 km windows) for a real tile and for the generator at matching sites; gaps named
Fallback if it fails: report that no GeoTIFF reader is available and stop
Code location: branch `spike/m9-glo30-targets` (local, not pushed; spike code, never merged); patch and scripts in `proto/`
Closed: 2026-10-29

**Licence notice (Copernicus WorldDEM-30, Art. 6).** The statistics in this folder are produced using Copernicus WorldDEM-30 © DLR e.V. 2010-2014 and © Airbus Defence and Space GmbH 2014-2018 provided under COPERNICUS by the European Union and ESA; all rights reserved. The organisations in charge of the Copernicus programme by law or by delegation do not incur any liability for any use of the Copernicus WorldDEM-30. No endorsement by Airbus, ESA, the EU or Copernicus is implied. Raw tiles are not in the repository (`C:/DEV/SeriousPlanetData/dem/glo30-samples/`, `LICENCE-NOTES.md` there). Citation: https://doi.org/10.5270/ESA-c5d3d65.

## Headline
- **Except for the Badlands the generator's hill and mountain terrain is 4 to 9 times too gentle at the 30 m to 5 km scales**, and its plains are smoother than real: against the real tile the median 1 km relief is 0.11 to 0.35 (hills, mountains and plains), the median 5 km relief 0.15 to 0.64, the median slope 0.13 to 0.43, the p99 slope 0.10 to 0.14 for hills and Alps and 0.26 to 0.42 for the Himalaya (ratios of the generator's two sites to the real tile).
- **The generator has no cell with Himalayan or high-Alpine spread**: the standard deviation of height per 1 degree cell (12 x 12 samples) is at most 753 m over the 10,912 land cells (p99 403 m); the Everest tile has 1,648 m and the Tyrol tile 603 m.
- The Badlands (dissected terrain) match at the median and in relief (ratios 0.9 to 1.6) with a lower p99 slope (0.4 to 0.65); the desert is rougher than real at the median (dunes are a different process) and smoother at p99 (no dune slip faces).

## Method
- Real: nine 1 degree GLO-30 tiles (1 arc-second, about 30 m; deflate with the floating-point predictor, decoded in Python with `tifffile` for offsets, `zlib` and `numpy`: `imagecodecs` is not installed and nothing was installed). Pixel spacing from the tile's scale and the mid-latitude (N52 tile: 1.5 arc-seconds in longitude).
- Generator: `global16384` store, read only, no chunk stage (`world.field()`), 30 m east-north grid, a 0.5 degree square (1,853 x 1,853 samples) per site; spike test `glo30_generator_report`.
- Metrics (same Python code for both, `proto/s55_metrics.py.txt`): slope = atan of the central-difference gradient; relief = max minus min over non-overlapping blocks of about 1 km and 5 km; percentiles over the pixels or blocks. No drainage metrics.
- Site matching: a coarse scan of 48,600 one-degree cells (latitude -60 to 75, 12 x 12 samples each): mean and standard deviation of height per cell; for each real tile the two land cells (mean over 0) nearest in (mean, standard deviation), at least 8 degrees apart.
- Correctness only: the CPU may be throttled (planner note 2026-10-29); no timing claims.

## Results
Slope in degrees (p50/p90/p99) and relief in metres (p50/p90/p99) at 30 m sampling; generator rows are the two matched sites (centre latitude, longitude).
| Landform (tile) | | slope | relief 1 km | relief 5 km |
|---|---|---|---|---|
| Brandenburg plain (N52E012) | real | 1.3/7.9/17.3 | 25/40/60 | 47/81/110 |
| | generator (-38.5, 30.5) | 0.6/1.1/1.5 | 9/13/18 | 30/44/48 |
| | generator (41.5, 149.5) | 0.6/1.1/1.6 | 8/13/18 | 27/41/58 |
| Loess hills and plains (N41W096) | real | 3.9/8.3/16.7 | 39/57/84 | 73/102/133 |
| | generator (-15.5, 150.5) | 0.9/1.6/2.3 | 13/20/28 | 41/61/74 |
| | generator (-10.5, 155.5) | 0.9/1.6/2.3 | 13/21/28 | 43/65/75 |
| Badlands (N43W102) | real | 2.8/8.1/17.3 | 34/62/95 | 93/134/170 |
| | generator (13.5, 22.5) | 2.5/4.7/6.8 | 34/53/75 | 104/141/191 |
| | generator (-33.5, 4.5) | 4.2/7.8/11.2 | 54/77/104 | 144/180/200 |
| Erg Chebbi desert (N31W004) | real | 1.1/4.8/21.9 | 16/70/145 | 67/171/226 |
| | generator (-23.5, -18.5) | 2.4/4.4/6.3 | 33/51/71 | 106/147/185 |
| | generator (9.5, 18.5) | 3.1/5.9/8.4 | 43/64/86 | 127/155/182 |
| Valley and Ridge (N40W078) | real | 7.2/19.7/29.3 | 107/253/337 | 343/463/514 |
| | generator (16.5, -111.5) | 1.4/2.7/4.0 | 20/31/44 | 65/83/107 |
| | generator (-40.5, 1.5) | 1.4/2.6/3.8 | 19/29/41 | 62/87/117 |
| Black Forest (N48E008) | real | 7.3/24.9/36.6 | 106/273/389 | 267/568/786 |
| | generator (2.5, -124.5) | 1.7/3.2/4.8 | 24/40/61 | 84/116/159 |
| | generator (17.5, -26.5) | 1.4/2.7/4.1 | 22/38/58 | 84/112/144 |
| Scottish Highlands (N57W005) | real | 7.2/18.8/34.2 | 133/278/431 | 424/680/854 |
| | generator (10.5, -122.5) | 1.2/2.4/3.5 | 18/29/44 | 65/103/129 |
| | generator (-26.5, -118.5) | 1.2/2.3/3.4 | 19/30/42 | 69/94/122 |
| Alps, Tyrol (N47E011) | real | 19.3/39.0/54.4 | 366/692/960 | 1050/1719/1980 |
| | generator (11.5, -18.5) | 2.6/5.1/7.6 | 45/77/111 | 169/234/309 |
| | generator (-38.5, 3.5) | 2.5/4.7/7.3 | 40/65/101 | 157/202/270 |
| Himalaya, Everest (N27E086) | real | 29.3/44.0/58.2 | 516/801/1179 | 1529/2339/2813 |
| | generator (-38.5, 1.5) | 6.8/14.4/24.2 | 125/249/430 | 476/729/844 |
| | generator (7.5, -19.5) | 4.2/8.9/15.2 | 73/147/250 | 275/421/551 |

Descriptors the sites were matched on (mean/standard deviation of height per 1 degree cell, metres): real Himalaya 2,611/1,648, Alps 1,178/603, Black Forest 538/238, Highlands 339/228, Valley and Ridge 310/128, Badlands 827/111, desert 943/117, loess 371/40, Brandenburg 57/34. The generator's best sites for the Himalaya (3,026/753 and 2,088/674) and the Alps (1,403/544 and 1,349/488) are the closest it has; the others match within 1 to 6 % (distance 0.02 to 0.25 in normalised units).

## Gaps (generator against real, both sites)
| Landform | Gap |
|---|---|
| Himalaya, Alps | relief 1 km 0.14 to 0.24 and 0.11 of real, 5 km 0.18 to 0.31 and 0.15 to 0.16; slope p50 0.14 to 0.23 and 0.13 of real; no cell reaches the real spread |
| Hills and ridges (Valley and Ridge, Black Forest, Highlands) | relief 1 km 0.14 to 0.23, 5 km 0.15 to 0.31; slope p50 0.17 to 0.23, p99 0.10 to 0.14 |
| Plains and loess | smoother: relief 1 km 0.33 to 0.35, slope p50 0.22 to 0.43 (the real values include DSM noise, below) |
| Badlands | median slope 0.9 to 1.5, relief 1.0 to 1.6 of real; p99 slope 0.4 to 0.65 |
| Desert | rougher at the median (relief 1 km 2.2 to 2.8 times), smoother at p99 (slope 0.29 to 0.38 of real) |

## Limits
- GLO-30 is a surface model (DSM): forest canopy, buildings and 1 to 2 m vertical noise raise the real slopes, most in Brandenburg, the Black Forest and the loess tile (p99 slope 17 degrees on a plain is not terrain); the gaps in the plains are an upper bound. The hill and mountain gaps (factors of 4 to 9) are far larger than that effect.
- One real tile per landform and one 0.5 degree window per site; the two sites per landform agree closely, so the generator side is stable; the real side is a single sample.
- Sites were matched on the coarse descriptor (about 9 km spacing), which the generator can only reproduce where its range of cells reaches the real values; the 30 m statistics are then an independent test.
- Relief by non-overlapping blocks (not a sliding window); tile 1 degree against window 0.5 degree.

## Visuals
Nothing to look at: tables of statistics only, no renders (and no image derived from GLO-30 is in the repository).
