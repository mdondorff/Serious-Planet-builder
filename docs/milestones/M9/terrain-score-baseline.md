# Terrain scorecard 2026-10-31

Statistics produced using Copernicus WorldDEM-30 © DLR e.V. 2010-2014 and © Airbus Defence and Space GmbH 2014-2018 provided under COPERNICUS by the European Union and ESA; all rights reserved. The organisations in charge of the Copernicus programme by law or by delegation do not incur any liability for any use of the Copernicus WorldDEM-30. Citation: https://doi.org/10.5270/ESA-c5d3d65

Baseline of M9 R1 (opus-planner, 2026-10-31): the scorecard of the generator as of main before any M9 change, produced by `cargo xtask terrain-score` (TEST-079, TEST-080, BUILD-029). Derived statistics only; no tiles and no maps are stored here. Later runs are compared against this file.

Date 2026-10-31. Commit b830eb38. Store C:/DEV/SeriousPlanetData/global16384 (definition 58100a60bf321ffe). Tiles C:/DEV/SeriousPlanetData/dem/glo30-samples. Sites file crates/app/data/terrain-score-sites.toml. Each landform: the central 0.5 degree square of a real GLO-30 tile against the generator at its matched sites, one table per site, all at 30 m. Distance: 0 means equal; for the curves it is the mean gap between the percentiles p1 to p99 in units of the real p5-to-p95 range; for the classes the sum of share gaps (0 to 2); for drainage density the ratio generator / real.

## Brandenburg plain (N52E012)

### Site 1 of Brandenburg plain: (-38.5, 30.5)

| Descriptor | Real p50 | Generator p50 | Distance |
|---|---|---|---|
| Slope (deg) | 1.15 | 0.561 | 0.188 |
| Profile curvature (1/m) | 3.4e-5 | -2.3e-5 | 0.138 |
| Contour curvature (1/m) | -0.001 | -0.002 | 0.060 |
| Relief 1 km (m) | 22.9 | 8.60 | 0.354 |
| Relief 5 km (m) | 50.6 | 30.2 | 0.390 |
| Height above 300 m mean (m) | -0.241 | 0.058 | 0.146 |
| Height above 2 km mean (m) | -1.44 | 0.154 | 0.130 |
| Drainage area (m2) | 2700 | 2700 | 0.053 |
| Landform classes (most common) | flat 23% | flat 91% | 1.39 |
| Drainage density (km/km2) | 2.54 | 3.12 | ratio 1.23 |

Largest gaps: Landform classes (1.39), Relief 5 km (m) (0.390), Relief 1 km (m) (0.354).

### Site 2 of Brandenburg plain: (41.5, 149.5)

| Descriptor | Real p50 | Generator p50 | Distance |
|---|---|---|---|
| Slope (deg) | 1.15 | 0.564 | 0.187 |
| Profile curvature (1/m) | 3.4e-5 | -2.5e-5 | 0.136 |
| Contour curvature (1/m) | -0.001 | -0.002 | 0.051 |
| Relief 1 km (m) | 22.9 | 8.19 | 0.363 |
| Relief 5 km (m) | 50.6 | 26.7 | 0.443 |
| Height above 300 m mean (m) | -0.241 | 0.062 | 0.145 |
| Height above 2 km mean (m) | -1.44 | 0.234 | 0.140 |
| Drainage area (m2) | 2700 | 1800 | 0.015 |
| Landform classes (most common) | flat 23% | flat 90% | 1.38 |
| Drainage density (km/km2) | 2.54 | 2.74 | ratio 1.08 |

Largest gaps: Landform classes (1.38), Relief 5 km (m) (0.443), Relief 1 km (m) (0.363).

## Loess hills and plains (N41W096)

### Site 1 of Loess hills and plains: (-15.5, 150.5)

| Descriptor | Real p50 | Generator p50 | Distance |
|---|---|---|---|
| Slope (deg) | 3.88 | 0.851 | 0.394 |
| Profile curvature (1/m) | 1.2e-4 | -4.0e-5 | 0.161 |
| Contour curvature (1/m) | -1.8e-6 | -0.002 | 0.057 |
| Relief 1 km (m) | 38.2 | 12.6 | 0.780 |
| Relief 5 km (m) | 69.8 | 41.2 | 0.637 |
| Height above 300 m mean (m) | -0.588 | 0.102 | 0.210 |
| Height above 2 km mean (m) | 0.300 | 0.360 | 0.164 |
| Drainage area (m2) | 2700 | 2700 | 0.089 |
| Landform classes (most common) | valley 23% | flat 52% | 1.48 |
| Drainage density (km/km2) | 2.12 | 2.88 | ratio 1.36 |

Largest gaps: Landform classes (1.48), Relief 1 km (m) (0.780), Relief 5 km (m) (0.637).

### Site 2 of Loess hills and plains: (-10.5, 155.5)

| Descriptor | Real p50 | Generator p50 | Distance |
|---|---|---|---|
| Slope (deg) | 3.88 | 0.878 | 0.391 |
| Profile curvature (1/m) | 1.2e-4 | -4.0e-5 | 0.161 |
| Contour curvature (1/m) | -1.8e-6 | -0.002 | 0.051 |
| Relief 1 km (m) | 38.2 | 13.3 | 0.757 |
| Relief 5 km (m) | 69.8 | 43.3 | 0.561 |
| Height above 300 m mean (m) | -0.588 | 0.101 | 0.210 |
| Height above 2 km mean (m) | 0.300 | 0.383 | 0.156 |
| Drainage area (m2) | 2700 | 2700 | 0.103 |
| Landform classes (most common) | valley 23% | flat 49% | 1.43 |
| Drainage density (km/km2) | 2.12 | 3.10 | ratio 1.47 |

Largest gaps: Landform classes (1.43), Relief 1 km (m) (0.757), Relief 5 km (m) (0.561).

## Badlands (N43W102)

### Site 1 of Badlands: (13.5, 22.5)

| Descriptor | Real p50 | Generator p50 | Distance |
|---|---|---|---|
| Slope (deg) | 3.04 | 2.51 | 0.124 |
| Profile curvature (1/m) | 4.7e-5 | -1.3e-4 | 0.065 |
| Contour curvature (1/m) | -9.0e-4 | -0.002 | 0.039 |
| Relief 1 km (m) | 39.5 | 34.5 | 0.106 |
| Relief 5 km (m) | 108 | 104 | 0.043 |
| Height above 300 m mean (m) | -0.260 | 0.233 | 0.083 |
| Height above 2 km mean (m) | -1.88 | -0.539 | 0.024 |
| Drainage area (m2) | 1800 | 3600 | 0.245 |
| Landform classes (most common) | slope 28% | slope 36% | 0.308 |
| Drainage density (km/km2) | 2.39 | 4.53 | ratio 1.89 |

Largest gaps: Drainage density (0.639), Landform classes (0.308), Drainage area (m2) (0.245).

### Site 2 of Badlands: (-33.5, 4.5)

| Descriptor | Real p50 | Generator p50 | Distance |
|---|---|---|---|
| Slope (deg) | 3.04 | 4.21 | 0.103 |
| Profile curvature (1/m) | 4.7e-5 | -2.3e-4 | 0.093 |
| Contour curvature (1/m) | -9.0e-4 | -0.002 | 0.058 |
| Relief 1 km (m) | 39.5 | 53.6 | 0.209 |
| Relief 5 km (m) | 108 | 144 | 0.357 |
| Height above 300 m mean (m) | -0.260 | 0.450 | 0.050 |
| Height above 2 km mean (m) | -1.88 | -0.164 | 0.097 |
| Drainage area (m2) | 1800 | 2700 | 0.195 |
| Landform classes (most common) | slope 28% | slope 27% | 0.230 |
| Drainage density (km/km2) | 2.39 | 3.38 | ratio 1.41 |

Largest gaps: Relief 5 km (m) (0.357), Drainage density (0.345), Landform classes (0.230).

## Erg Chebbi desert (N31W004)

### Site 1 of Erg Chebbi desert: (-23.5, -18.5)

| Descriptor | Real p50 | Generator p50 | Distance |
|---|---|---|---|
| Slope (deg) | 0.859 | 2.37 | 0.323 |
| Profile curvature (1/m) | -1.0e-5 | -1.2e-4 | 0.279 |
| Contour curvature (1/m) | -3.4e-4 | -0.002 | 0.058 |
| Relief 1 km (m) | 11.8 | 33.5 | 0.264 |
| Relief 5 km (m) | 40.8 | 106 | 0.295 |
| Height above 300 m mean (m) | 0.007 | 0.236 | 0.170 |
| Height above 2 km mean (m) | 0.072 | -0.506 | 0.155 |
| Drainage area (m2) | 2700 | 3600 | 0.240 |
| Landform classes (most common) | flat 48% | slope 38% | 1.28 |
| Drainage density (km/km2) | 2.44 | 4.31 | ratio 1.76 |

Largest gaps: Landform classes (1.28), Drainage density (0.567), Slope (deg) (0.323).

### Site 2 of Erg Chebbi desert: (9.5, 18.5)

| Descriptor | Real p50 | Generator p50 | Distance |
|---|---|---|---|
| Slope (deg) | 0.859 | 3.15 | 0.476 |
| Profile curvature (1/m) | -1.0e-5 | -1.7e-4 | 0.434 |
| Contour curvature (1/m) | -3.4e-4 | -0.002 | 0.064 |
| Relief 1 km (m) | 11.8 | 43.2 | 0.368 |
| Relief 5 km (m) | 40.8 | 127 | 0.384 |
| Height above 300 m mean (m) | 0.007 | 0.317 | 0.266 |
| Height above 2 km mean (m) | 0.072 | -0.513 | 0.227 |
| Drainage area (m2) | 2700 | 3600 | 0.271 |
| Landform classes (most common) | flat 48% | slope 32% | 1.30 |
| Drainage density (km/km2) | 2.44 | 3.75 | ratio 1.53 |

Largest gaps: Landform classes (1.30), Slope (deg) (0.476), Profile curvature (1/m) (0.434).

## Valley and Ridge (N40W078)

### Site 1 of Valley and Ridge: (16.5, -111.5)

| Descriptor | Real p50 | Generator p50 | Distance |
|---|---|---|---|
| Slope (deg) | 8.11 | 1.43 | 0.360 |
| Profile curvature (1/m) | 1.0e-4 | -7.0e-5 | 0.154 |
| Contour curvature (1/m) | -5.2e-4 | -0.002 | 0.156 |
| Relief 1 km (m) | 120 | 20.4 | 0.459 |
| Relief 5 km (m) | 397 | 65.2 | 0.875 |
| Height above 300 m mean (m) | -1.87 | 0.134 | 0.183 |
| Height above 2 km mean (m) | -13.2 | -0.112 | 0.188 |
| Drainage area (m2) | 2700 | 2700 | 0.131 |
| Landform classes (most common) | slope 39% | slope 41% | 0.519 |
| Drainage density (km/km2) | 2.16 | 2.88 | ratio 1.33 |

Largest gaps: Relief 5 km (m) (0.875), Landform classes (0.519), Relief 1 km (m) (0.459).

### Site 2 of Valley and Ridge: (-40.5, 1.5)

| Descriptor | Real p50 | Generator p50 | Distance |
|---|---|---|---|
| Slope (deg) | 8.11 | 1.37 | 0.363 |
| Profile curvature (1/m) | 1.0e-4 | -7.1e-5 | 0.154 |
| Contour curvature (1/m) | -5.2e-4 | -0.002 | 0.169 |
| Relief 1 km (m) | 120 | 19.1 | 0.464 |
| Relief 5 km (m) | 397 | 61.7 | 0.883 |
| Height above 300 m mean (m) | -1.87 | 0.152 | 0.185 |
| Height above 2 km mean (m) | -13.2 | 0.277 | 0.191 |
| Drainage area (m2) | 2700 | 1800 | 0.140 |
| Landform classes (most common) | slope 39% | slope 38% | 0.572 |
| Drainage density (km/km2) | 2.16 | 2.70 | ratio 1.25 |

Largest gaps: Relief 5 km (m) (0.883), Landform classes (0.572), Relief 1 km (m) (0.464).

## Black Forest (N48E008)

### Site 1 of Black Forest: (2.5, -124.5)

| Descriptor | Real p50 | Generator p50 | Distance |
|---|---|---|---|
| Slope (deg) | 9.06 | 1.66 | 0.354 |
| Profile curvature (1/m) | -5.4e-5 | -7.8e-5 | 0.149 |
| Contour curvature (1/m) | -5.8e-4 | -0.002 | 0.105 |
| Relief 1 km (m) | 141 | 24.2 | 0.464 |
| Relief 5 km (m) | 315 | 83.7 | 0.542 |
| Height above 300 m mean (m) | 0.737 | 0.141 | 0.172 |
| Height above 2 km mean (m) | 5.31 | -0.072 | 0.165 |
| Drainage area (m2) | 2700 | 2700 | 0.174 |
| Landform classes (most common) | slope 29% | slope 45% | 0.580 |
| Drainage density (km/km2) | 1.97 | 2.86 | ratio 1.45 |

Largest gaps: Landform classes (0.580), Relief 5 km (m) (0.542), Relief 1 km (m) (0.464).

### Site 2 of Black Forest: (17.5, -26.5)

| Descriptor | Real p50 | Generator p50 | Distance |
|---|---|---|---|
| Slope (deg) | 9.06 | 1.43 | 0.363 |
| Profile curvature (1/m) | -5.4e-5 | -5.9e-5 | 0.159 |
| Contour curvature (1/m) | -5.8e-4 | -0.002 | 0.077 |
| Relief 1 km (m) | 141 | 22.5 | 0.470 |
| Relief 5 km (m) | 315 | 84.0 | 0.541 |
| Height above 300 m mean (m) | 0.737 | 0.105 | 0.176 |
| Height above 2 km mean (m) | 5.31 | -0.301 | 0.168 |
| Drainage area (m2) | 2700 | 2700 | 0.098 |
| Landform classes (most common) | slope 29% | slope 46% | 0.840 |
| Drainage density (km/km2) | 1.97 | 2.66 | ratio 1.35 |

Largest gaps: Landform classes (0.840), Relief 5 km (m) (0.541), Relief 1 km (m) (0.470).

## Scottish Highlands (N57W005)

### Site 1 of Scottish Highlands: (10.5, -122.5)

| Descriptor | Real p50 | Generator p50 | Distance |
|---|---|---|---|
| Slope (deg) | 6.58 | 1.21 | 0.304 |
| Profile curvature (1/m) | 0 | -5.7e-5 | 0.136 |
| Contour curvature (1/m) | -5.8e-4 | -0.002 | 0.178 |
| Relief 1 km (m) | 127 | 18.1 | 0.391 |
| Relief 5 km (m) | 389 | 65.2 | 0.582 |
| Height above 300 m mean (m) | -0.278 | 0.113 | 0.166 |
| Height above 2 km mean (m) | -3.87 | 0.129 | 0.187 |
| Drainage area (m2) | 4500 | 1800 | 0.078 |
| Landform classes (most common) | slope 37% | slope 36% | 0.671 |
| Drainage density (km/km2) | 2.79 | 2.57 | ratio 0.921 |

Largest gaps: Landform classes (0.671), Relief 5 km (m) (0.582), Relief 1 km (m) (0.391).

### Site 2 of Scottish Highlands: (-26.5, -118.5)

| Descriptor | Real p50 | Generator p50 | Distance |
|---|---|---|---|
| Slope (deg) | 6.58 | 1.23 | 0.304 |
| Profile curvature (1/m) | 0 | -5.1e-5 | 0.139 |
| Contour curvature (1/m) | -5.8e-4 | -0.002 | 0.150 |
| Relief 1 km (m) | 127 | 19.3 | 0.388 |
| Relief 5 km (m) | 389 | 68.8 | 0.578 |
| Height above 300 m mean (m) | -0.278 | 0.110 | 0.167 |
| Height above 2 km mean (m) | -3.87 | 0.318 | 0.184 |
| Drainage area (m2) | 4500 | 1800 | 0.040 |
| Landform classes (most common) | slope 37% | slope 40% | 0.689 |
| Drainage density (km/km2) | 2.79 | 2.56 | ratio 0.916 |

Largest gaps: Landform classes (0.689), Relief 5 km (m) (0.578), Relief 1 km (m) (0.388).

## Alps, Tyrol (N47E011)

### Site 1 of Alps, Tyrol: (11.5, -18.5)

| Descriptor | Real p50 | Generator p50 | Distance |
|---|---|---|---|
| Slope (deg) | 22.3 | 2.65 | 0.432 |
| Profile curvature (1/m) | 1.1e-4 | -9.0e-5 | 0.147 |
| Contour curvature (1/m) | -5.2e-4 | -0.002 | 0.084 |
| Relief 1 km (m) | 391 | 44.9 | 0.457 |
| Relief 5 km (m) | 1039 | 169 | 0.572 |
| Height above 300 m mean (m) | -1.47 | 0.109 | 0.180 |
| Height above 2 km mean (m) | -19.4 | -1.22 | 0.199 |
| Drainage area (m2) | 3600 | 2700 | 0.044 |
| Landform classes (most common) | slope 37% | slope 49% | 0.260 |
| Drainage density (km/km2) | 2.33 | 2.74 | ratio 1.18 |

Largest gaps: Relief 5 km (m) (0.572), Relief 1 km (m) (0.457), Slope (deg) (0.432).

### Site 2 of Alps, Tyrol: (-38.5, 3.5)

| Descriptor | Real p50 | Generator p50 | Distance |
|---|---|---|---|
| Slope (deg) | 22.3 | 2.49 | 0.436 |
| Profile curvature (1/m) | 1.1e-4 | -1.0e-4 | 0.141 |
| Contour curvature (1/m) | -5.2e-4 | -0.002 | 0.141 |
| Relief 1 km (m) | 391 | 40.0 | 0.465 |
| Relief 5 km (m) | 1039 | 157 | 0.586 |
| Height above 300 m mean (m) | -1.47 | 0.170 | 0.181 |
| Height above 2 km mean (m) | -19.4 | -0.809 | 0.203 |
| Drainage area (m2) | 3600 | 2700 | 0.072 |
| Landform classes (most common) | slope 37% | slope 45% | 0.218 |
| Drainage density (km/km2) | 2.33 | 3.04 | ratio 1.31 |

Largest gaps: Relief 5 km (m) (0.586), Relief 1 km (m) (0.465), Slope (deg) (0.436).

## Himalaya, Everest (N27E086)

### Site 1 of Himalaya, Everest: (-38.5, 1.5)

| Descriptor | Real p50 | Generator p50 | Distance |
|---|---|---|---|
| Slope (deg) | 29.1 | 6.78 | 0.626 |
| Profile curvature (1/m) | 6.2e-6 | -1.7e-4 | 0.098 |
| Contour curvature (1/m) | -1.4e-4 | -0.001 | 0.127 |
| Relief 1 km (m) | 524 | 125 | 0.779 |
| Relief 5 km (m) | 1573 | 476 | 0.862 |
| Height above 300 m mean (m) | 0.378 | 0.097 | 0.178 |
| Height above 2 km mean (m) | 0.107 | -4.11 | 0.172 |
| Drainage area (m2) | 3600 | 7200 | 0.262 |
| Landform classes (most common) | slope 42% | slope 45% | 0.087 |
| Drainage density (km/km2) | 2.07 | 5.27 | ratio 2.54 |

Largest gaps: Drainage density (0.934), Relief 5 km (m) (0.862), Relief 1 km (m) (0.779).

### Site 2 of Himalaya, Everest: (7.5, -19.5)

| Descriptor | Real p50 | Generator p50 | Distance |
|---|---|---|---|
| Slope (deg) | 29.1 | 4.16 | 0.713 |
| Profile curvature (1/m) | 6.2e-6 | -1.3e-4 | 0.134 |
| Contour curvature (1/m) | -1.4e-4 | -0.001 | 0.156 |
| Relief 1 km (m) | 524 | 73.3 | 0.892 |
| Relief 5 km (m) | 1573 | 275 | 1.01 |
| Height above 300 m mean (m) | 0.378 | 0.136 | 0.193 |
| Height above 2 km mean (m) | 0.107 | -1.87 | 0.205 |
| Drainage area (m2) | 3600 | 3600 | 0.180 |
| Landform classes (most common) | slope 42% | slope 45% | 0.071 |
| Drainage density (km/km2) | 2.07 | 3.30 | ratio 1.59 |

Largest gaps: Relief 5 km (m) (1.01), Relief 1 km (m) (0.892), Slope (deg) (0.713).

## Notes

- GLO-30 is a surface model: canopy, buildings and 1 to 2 m of vertical noise raise the real slopes, so the slope gaps on plains are upper bounds.
- One real tile per landform and one 0.5 degree window per site; the real side is a single sample.
- The largest gaps of a site rank its own distances (report.json ranks the mean over the sites per landform); drainage density is ranked by the size of the log of its ratio. There is no aggregate score.

Run: store C:/DEV/SeriousPlanetData/global16384 (definition 58100a60bf321ffe), tiles C:/DEV/SeriousPlanetData/dem/glo30-samples, sites crates/app/data/terrain-score-sites.toml, 20 threads. Phases (s): read tiles 0.4, sample generator windows 17.4, descriptors 17.5, collect results 0.0, total 35.4.
