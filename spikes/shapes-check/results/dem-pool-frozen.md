# SPIKE-108: the frozen DEM pool

Frozen 2026-11-12 (architect's order): the tile list, the slope bands and the pool statistics below are fixed numbers; every later re-measure compares against them. Statistics derived from Copernicus DEM GLO-30: produced using Copernicus WorldDEM-30 © DLR e.V. 2010-2014 and © Airbus Defence and Space GmbH 2014-2018 provided under COPERNICUS by the European Union and ESA; all rights reserved. The organisations in charge of the Copernicus programme by law or by delegation do not incur any liability for any use of the Copernicus WorldDEM-30. No DEM raster is in the repository.

## 1. Tiles and bands

Source: the GLO-30 sample tiles of `SeriousPlanetData/dem/glo30-samples` (15 tiles of 1 x 1 degree). Method (`results.md` section 2): block mean to cells of about 814 x 803 m, cells at or below 0 m or with half or more no-data are sea, a 10-cell rim is left out of the counts, `route_rules_filled` under the U rules with epsilon 1 mm, channel = MFD area at least 75 km^2. The band is the steepest-neighbour slope p95 of the land of the tile.

| Pool | Band (slope p95) | Tiles (slope p95) |
|---|---|---|
| High relief (8 tiles) | 0.2 to 0.7 | N27 E086 (0.687), N40 W078 (0.217), N42 W123 (0.275), N45 E014 (0.295), N47 E011 (0.591), N48 E008 (0.2165), N57 W005 (0.242), N61 E006 (0.679) |
| Mid relief (4 tiles) | 0.03 to 0.14 | N31 W004 (0.078), N41 W096 (0.0368), N43 W102 (0.0452), S25 E015 (0.1127) |
| Not in a pool (low relief) | below 0.03 | N35 W076 (0.0082), N52 E012 (0.0268), N70 W154 (0.0131) |
| All tiles (15) | none | all of the above |

Use: R1 (slope p95 0.32 on the 620-cell box) against the high pool, R3a (0.069) and R3b (0.133) against the mid pool.

## 2. Pool statistics (routing of main 1041d28c, GEN-173 in; U rules)

| Statistic | High (8) | Mid (4) | All (15) |
|---|---|---|---|
| Land cells | 66,459 | 42,716 | 120,037 |
| Channel steps | 4,217 | 3,459 | 8,480 |
| Chords of 8 / 16 steps | 3,753 / 3,339 | 3,047 / 2,799 | 7,464 / 6,686 |
| Chord 8: share within 2.5 deg of 0/45/90/135 (axis / diagonal) | 0.1303 (0.0757 / 0.0546) | 0.1280 (0.0886 / 0.0394) | 0.1289 (0.0800 / 0.0489) |
| Chord 8: within 5 deg (axis / diagonal) | 0.2070 (0.0757 / 0.1314) | 0.1743 (0.0886 / 0.0857) | 0.1936 (0.0800 / 0.1136) |
| Chord 16: within 2.5 deg (axis / diagonal) | 0.0946 (0.0389 / 0.0557) | 0.0622 (0.0364 / 0.0257) | 0.0823 (0.0378 / 0.0444) |
| Chord 16: within 5 deg (axis / diagonal) | 0.2372 (0.1171 / 0.1201) | 0.1969 (0.1240 / 0.0729) | 0.2236 (0.1207 / 0.1029) |
| Channel length in runs > 10 cells / > 20 cells | 0.0070 / 0.0000 | 0.0113 / 0.0000 | 0.0090 / 0.0000 |
| Runs over 10 cells (count) | 3 | 3 | 7 |
| Diagonal share of channel length | 0.5476 | 0.5084 | 0.5318 |
| Steps per direction W / E / N / S / NW / NE / SW / SE | 0.110 / 0.156 / 0.167 / 0.107 / 0.107 / 0.185 / 0.097 / 0.072 | 0.115 / 0.090 / 0.104 / 0.269 / 0.077 / 0.050 / 0.201 / 0.095 | 0.127 / 0.122 / 0.137 / 0.169 / 0.110 / 0.121 / 0.135 / 0.078 |
| Steps on lifted cells (share) | 0.557 | 0.329 | 0.484 |

- The earlier values of the same pools (main b8c5bbb5, before the grade) are in `results.md` section 4 (run shares over 10 cells 0.0263 high, 0.0113 mid, 0.0200 all); the frozen numbers are these of 1041d28c.
- A later re-measure routes the DEM with the library at hand and reports the pool statistics it gets next to these fixed numbers; the ratios are taken against the fixed numbers. Raw: `raw-gen173/dem-*.txt`, `raw-gen173/dirs-dem-*.txt`.
