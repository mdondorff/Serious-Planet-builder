# SPIKE-09 results

8064 static cases, 26026 walk samples (26 series x 1001)

## Microsoft Basic Render Driver [Dx12]

### form: naive

| altitude | cases | fail >bound | fail % | max err/bound | ground-class mismatches | max rel err t_end | worst case |
|---|---|---|---|---|---|---|---|
| 1 m | 1872 | 177 | 9.5 | 1286.388 | 144 | 2.93e2 | h=1.3125 horizon-1e-6 low sun phi=0.00 |
| 2 km | 1872 | 56 | 3.0 | 26.195 | 72 | 8.77e-1 | h=2000.0000 horizon+1e-6 low sun phi=0.00 |
| 20 km | 1872 | 0 | 0.0 | 0.350 | 0 | 1.38e-3 | h=26250.0000 horizon-1e-6 twilight phi=0.00 |
| 20,000 km | 2448 | 0 | 0.0 | 0.139 | 0 | 4.36e-5 | h=32500000.0000 horizon-1e-6 noon-ish phi=1.57 |

Slow walk, camera rising 1.000 to 2.000 m in 1 mm steps; a step is a frame-to-frame change of the f32 radiance whose difference from the oracle's change exceeds the bound (1 % + 1e-4 of the oracle value).

| series | max err/bound | frames with error > bound | steps beyond bound | max step/bound |
|---|---|---|---|---|
| mu=-1 noon-ish | 0.000 | 0 | 0 | 0.000 |
| mu=-1 low sun | 0.001 | 0 | 0 | 0.001 |
| mu=-0.5 noon-ish | 0.001 | 0 | 0 | 0.001 |
| mu=-0.5 low sun | 0.002 | 0 | 0 | 0.002 |
| mu=-0.1 noon-ish | 0.004 | 0 | 0 | 0.006 |
| mu=-0.1 low sun | 0.005 | 0 | 0 | 0.009 |
| mu=-0.01 noon-ish | 0.042 | 0 | 0 | 0.063 |
| mu=-0.01 low sun | 0.059 | 0 | 0 | 0.092 |
| horizon-1e-4 noon-ish | 77.178 | 361 | 3 | 77.167 |
| horizon-1e-4 low sun | 58.449 | 397 | 3 | 58.453 |
| horizon-1e-6 noon-ish | 77.242 | 247 | 2 | 77.231 |
| horizon-1e-6 low sun | 58.538 | 247 | 2 | 58.546 |
| horizon+1e-6 noon-ish | 77.158 | 251 | 1 | 77.148 |
| horizon+1e-6 low sun | 58.486 | 251 | 1 | 58.493 |
| horizon+1e-4 noon-ish | 0.007 | 0 | 0 | 0.012 |
| horizon+1e-4 low sun | 0.010 | 0 | 0 | 0.018 |
| mu=-1e-6 noon-ish | 0.007 | 0 | 0 | 0.012 |
| mu=-1e-6 low sun | 0.010 | 0 | 0 | 0.019 |
| mu=+1e-6 noon-ish | 0.006 | 0 | 0 | 0.012 |
| mu=+1e-6 low sun | 0.011 | 0 | 0 | 0.019 |
| mu=0.01 noon-ish | 0.006 | 0 | 0 | 0.005 |
| mu=0.01 low sun | 0.008 | 0 | 0 | 0.009 |
| mu=0.5 noon-ish | 0.003 | 0 | 0 | 0.004 |
| mu=0.5 low sun | 0.002 | 0 | 0 | 0.002 |
| mu=+1 noon-ish | 0.003 | 0 | 0 | 0.004 |
| mu=+1 low sun | 0.001 | 0 | 0 | 0.001 |

Geometry at 1 m (max relative error of the ray's end distance t_end over the 16 altitudes 1.0-1.94 m, 3 suns, 3 azimuths; the radiance bound hides a wrong t when the ray is short):

| mu | max rel err t_end | cases with ground/sky class wrong |
|---|---|---|
| mu=-1 | 2.00e-1 | 0 |
| mu=-0.5 | 2.00e-1 | 0 |
| mu=-0.1 | 2.19e-1 | 0 |
| mu=-0.01 | 2.20e-1 | 0 |
| horizon-1e-4 | 2.93e-1 | 0 |
| horizon-1e-6 | 2.93e2 | 45 |
| horizon+1e-6 | 9.98e-1 | 99 |
| horizon+1e-4 | 1.65e-6 | 0 |
| mu=-1e-6 | 1.66e-6 | 0 |
| mu=+1e-6 | 1.65e-6 | 0 |
| mu=0.01 | 1.66e-6 | 0 |
| mu=0.5 | 2.77e-6 | 0 |
| mu=+1 | 2.50e-6 | 0 |

Horizon band: widest |mu - mu_horizon| (radians) at which any case still exceeds the bound; delta<0 is below the geometric horizon.

| altitude | mu_horizon | widest failing below | widest failing above | cases failing |
|---|---|---|---|---|
| 1 m | -5.603e-4 | 1.0e-4 | 3.2e-6 | 74 |
| 10 m | -1.772e-3 | 1.0e-3 | 1.0e-5 | 116 |
| 100 m | -5.603e-3 | 3.2e-5 | 5.6e-6 | 90 |
| 2000 m | -2.505e-2 | 0.0e0 | 1.0e-6 | 32 |
| 20000 m | -7.905e-2 | 0.0e0 | 5.6e-7 | 24 |

### form: primitives

| altitude | cases | fail >bound | fail % | max err/bound | ground-class mismatches | max rel err t_end | worst case |
|---|---|---|---|---|---|---|---|
| 1 m | 1872 | 0 | 0.0 | 0.000 | 0 | 1.01e-6 | h=1.0625 horizon+1e-4 low sun phi=0.00 |
| 2 km | 1872 | 0 | 0.0 | 0.001 | 0 | 6.67e-6 | h=3875.0000 horizon-1e-6 low sun phi=0.00 |
| 20 km | 1872 | 0 | 0.0 | 0.004 | 0 | 1.34e-5 | h=33750.0000 horizon-1e-6 twilight phi=0.00 |
| 20,000 km | 2448 | 0 | 0.0 | 0.191 | 0 | 6.81e-5 | h=38750000.0000 horizon-1e-6 low sun phi=0.00 |

Slow walk, camera rising 1.000 to 2.000 m in 1 mm steps; a step is a frame-to-frame change of the f32 radiance whose difference from the oracle's change exceeds the bound (1 % + 1e-4 of the oracle value).

| series | max err/bound | frames with error > bound | steps beyond bound | max step/bound |
|---|---|---|---|---|
| mu=-1 noon-ish | 0.000 | 0 | 0 | 0.000 |
| mu=-1 low sun | 0.000 | 0 | 0 | 0.000 |
| mu=-0.5 noon-ish | 0.000 | 0 | 0 | 0.000 |
| mu=-0.5 low sun | 0.000 | 0 | 0 | 0.000 |
| mu=-0.1 noon-ish | 0.000 | 0 | 0 | 0.000 |
| mu=-0.1 low sun | 0.000 | 0 | 0 | 0.000 |
| mu=-0.01 noon-ish | 0.000 | 0 | 0 | 0.000 |
| mu=-0.01 low sun | 0.000 | 0 | 0 | 0.000 |
| horizon-1e-4 noon-ish | 0.000 | 0 | 0 | 0.000 |
| horizon-1e-4 low sun | 0.000 | 0 | 0 | 0.000 |
| horizon-1e-6 noon-ish | 0.000 | 0 | 0 | 0.000 |
| horizon-1e-6 low sun | 0.000 | 0 | 0 | 0.000 |
| horizon+1e-6 noon-ish | 0.000 | 0 | 0 | 0.000 |
| horizon+1e-6 low sun | 0.000 | 0 | 0 | 0.000 |
| horizon+1e-4 noon-ish | 0.000 | 0 | 0 | 0.000 |
| horizon+1e-4 low sun | 0.000 | 0 | 0 | 0.000 |
| mu=-1e-6 noon-ish | 0.000 | 0 | 0 | 0.000 |
| mu=-1e-6 low sun | 0.000 | 0 | 0 | 0.000 |
| mu=+1e-6 noon-ish | 0.000 | 0 | 0 | 0.000 |
| mu=+1e-6 low sun | 0.000 | 0 | 0 | 0.000 |
| mu=0.01 noon-ish | 0.000 | 0 | 0 | 0.000 |
| mu=0.01 low sun | 0.000 | 0 | 0 | 0.000 |
| mu=0.5 noon-ish | 0.000 | 0 | 0 | 0.000 |
| mu=0.5 low sun | 0.000 | 0 | 0 | 0.000 |
| mu=+1 noon-ish | 0.000 | 0 | 0 | 0.000 |
| mu=+1 low sun | 0.000 | 0 | 0 | 0.000 |

Geometry at 1 m (max relative error of the ray's end distance t_end over the 16 altitudes 1.0-1.94 m, 3 suns, 3 azimuths; the radiance bound hides a wrong t when the ray is short):

| mu | max rel err t_end | cases with ground/sky class wrong |
|---|---|---|
| mu=-1 | 7.95e-8 | 0 |
| mu=-0.5 | 8.99e-8 | 0 |
| mu=-0.1 | 8.30e-8 | 0 |
| mu=-0.01 | 1.21e-7 | 0 |
| horizon-1e-4 | 2.00e-7 | 0 |
| horizon-1e-6 | 1.01e-6 | 0 |
| horizon+1e-6 | 1.17e-7 | 0 |
| horizon+1e-4 | 1.14e-7 | 0 |
| mu=-1e-6 | 6.79e-8 | 0 |
| mu=+1e-6 | 1.09e-7 | 0 |
| mu=0.01 | 1.18e-7 | 0 |
| mu=0.5 | 8.82e-8 | 0 |
| mu=+1 | 7.81e-8 | 0 |

Horizon band: widest |mu - mu_horizon| (radians) at which any case still exceeds the bound; delta<0 is below the geometric horizon.

| altitude | mu_horizon | widest failing below | widest failing above | cases failing |
|---|---|---|---|---|
| 1 m | -5.603e-4 | 0.0e0 | 0.0e0 | 0 |
| 10 m | -1.772e-3 | 0.0e0 | 0.0e0 | 0 |
| 100 m | -5.603e-3 | 0.0e0 | 0.0e0 | 0 |
| 2000 m | -2.505e-2 | 0.0e0 | 0.0e0 | 0 |
| 20000 m | -7.905e-2 | 0.0e0 | 0.0e0 | 0 |

