# SPIKE-09 results

8064 static cases, 26026 walk samples (26 series x 1001)

## NVIDIA GeForce RTX 3080 Ti Laptop GPU [Dx12]

### form: naive

| altitude | cases | fail >bound | fail % | max err/bound | ground-class mismatches | max rel err t_end | worst case |
|---|---|---|---|---|---|---|---|
| 1 m | 1872 | 196 | 10.5 | 1286.426 | 153 | 2.93e2 | h=1.3125 horizon-1e-6 low sun phi=0.00 |
| 2 km | 1872 | 0 | 0.0 | 0.293 | 0 | 3.61e-3 | h=2000.0000 horizon-1e-6 low sun phi=0.00 |
| 20 km | 1872 | 0 | 0.0 | 0.229 | 0 | 7.12e-4 | h=21250.0000 horizon-1e-6 twilight phi=0.00 |
| 20,000 km | 2448 | 0 | 0.0 | 0.226 | 0 | 5.27e-5 | h=28750000.0000 horizon-1e-6 low sun phi=0.00 |

Slow walk, camera rising 1.000 to 2.000 m in 1 mm steps; a step is a frame-to-frame change of the f32 radiance whose difference from the oracle's change exceeds the bound (1 % + 1e-4 of the oracle value).

| series | max err/bound | frames with error > bound | steps beyond bound | max step/bound |
|---|---|---|---|---|
| mu=-1 noon-ish | 0.001 | 0 | 0 | 0.000 |
| mu=-1 low sun | 0.000 | 0 | 0 | 0.001 |
| mu=-0.5 noon-ish | 0.001 | 0 | 0 | 0.001 |
| mu=-0.5 low sun | 0.002 | 0 | 0 | 0.002 |
| mu=-0.1 noon-ish | 0.003 | 0 | 0 | 0.005 |
| mu=-0.1 low sun | 0.006 | 0 | 0 | 0.007 |
| mu=-0.01 noon-ish | 0.037 | 0 | 0 | 0.048 |
| mu=-0.01 low sun | 0.056 | 0 | 0 | 0.070 |
| horizon-1e-4 noon-ish | 76.996 | 407 | 3 | 76.984 |
| horizon-1e-4 low sun | 58.336 | 495 | 3 | 58.342 |
| horizon-1e-6 noon-ish | 78.027 | 251 | 2 | 78.016 |
| horizon-1e-6 low sun | 59.032 | 251 | 2 | 59.040 |
| horizon+1e-6 noon-ish | 78.010 | 251 | 1 | 78.000 |
| horizon+1e-6 low sun | 59.023 | 251 | 1 | 59.029 |
| horizon+1e-4 noon-ish | 0.008 | 0 | 0 | 0.008 |
| horizon+1e-4 low sun | 0.014 | 0 | 0 | 0.013 |
| mu=-1e-6 noon-ish | 0.007 | 0 | 0 | 0.008 |
| mu=-1e-6 low sun | 0.012 | 0 | 0 | 0.014 |
| mu=+1e-6 noon-ish | 0.006 | 0 | 0 | 0.008 |
| mu=+1e-6 low sun | 0.011 | 0 | 0 | 0.014 |
| mu=0.01 noon-ish | 0.005 | 0 | 0 | 0.005 |
| mu=0.01 low sun | 0.009 | 0 | 0 | 0.011 |
| mu=0.5 noon-ish | 0.003 | 0 | 0 | 0.003 |
| mu=0.5 low sun | 0.001 | 0 | 0 | 0.001 |
| mu=+1 noon-ish | 0.002 | 0 | 0 | 0.004 |
| mu=+1 low sun | 0.001 | 0 | 0 | 0.001 |

Geometry at 1 m (max relative error of the ray's end distance t_end over the 16 altitudes 1.0-1.94 m, 3 suns, 3 azimuths; the radiance bound hides a wrong t when the ray is short):

| mu | max rel err t_end | cases with ground/sky class wrong |
|---|---|---|
| mu=-1 | 2.00e-1 | 0 |
| mu=-0.5 | 3.00e-1 | 0 |
| mu=-0.1 | 3.21e-1 | 0 |
| mu=-0.01 | 3.23e-1 | 0 |
| horizon-1e-4 | 4.02e-1 | 0 |
| horizon-1e-6 | 2.93e2 | 27 |
| horizon+1e-6 | 9.98e-1 | 117 |
| horizon+1e-4 | 9.98e-1 | 9 |
| mu=-1e-6 | 1.81e-6 | 0 |
| mu=+1e-6 | 1.80e-6 | 0 |
| mu=0.01 | 1.93e-6 | 0 |
| mu=0.5 | 2.77e-6 | 0 |
| mu=+1 | 2.50e-6 | 0 |

Horizon band: widest |mu - mu_horizon| (radians) at which any case still exceeds the bound; delta<0 is below the geometric horizon.

| altitude | mu_horizon | widest failing below | widest failing above | cases failing |
|---|---|---|---|---|
| 1 m | -5.603e-4 | 5.6e-3 | 3.2e-5 | 123 |
| 10 m | -1.772e-3 | 1.8e-3 | 1.0e-5 | 118 |
| 100 m | -5.603e-3 | 1.0e-5 | 3.2e-6 | 73 |
| 2000 m | -2.505e-2 | 0.0e0 | 5.6e-7 | 32 |
| 20000 m | -7.905e-2 | 0.0e0 | 1.8e-7 | 16 |

### form: primitives

| altitude | cases | fail >bound | fail % | max err/bound | ground-class mismatches | max rel err t_end | worst case |
|---|---|---|---|---|---|---|---|
| 1 m | 1872 | 0 | 0.0 | 0.000 | 0 | 8.92e-7 | h=1.4375 mu=0.01 low sun phi=0.00 |
| 2 km | 1872 | 0 | 0.0 | 0.001 | 0 | 6.03e-6 | h=2375.0000 horizon-1e-6 low sun phi=0.00 |
| 20 km | 1872 | 0 | 0.0 | 0.004 | 0 | 1.36e-5 | h=31250.0000 horizon-1e-6 twilight phi=0.00 |
| 20,000 km | 2448 | 0 | 0.0 | 0.837 | 0 | 5.27e-5 | h=27500000.0000 horizon-1e-6 twilight phi=1.57 |

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
| mu=-1 | 9.54e-8 | 0 |
| mu=-0.5 | 8.99e-8 | 0 |
| mu=-0.1 | 9.84e-8 | 0 |
| mu=-0.01 | 8.35e-8 | 0 |
| horizon-1e-4 | 1.85e-7 | 0 |
| horizon-1e-6 | 8.92e-7 | 0 |
| horizon+1e-6 | 1.17e-7 | 0 |
| horizon+1e-4 | 1.14e-7 | 0 |
| mu=-1e-6 | 9.48e-8 | 0 |
| mu=+1e-6 | 1.09e-7 | 0 |
| mu=0.01 | 1.49e-7 | 0 |
| mu=0.5 | 1.44e-7 | 0 |
| mu=+1 | 1.56e-7 | 0 |

Horizon band: widest |mu - mu_horizon| (radians) at which any case still exceeds the bound; delta<0 is below the geometric horizon.

| altitude | mu_horizon | widest failing below | widest failing above | cases failing |
|---|---|---|---|---|
| 1 m | -5.603e-4 | 0.0e0 | 0.0e0 | 0 |
| 10 m | -1.772e-3 | 0.0e0 | 0.0e0 | 0 |
| 100 m | -5.603e-3 | 0.0e0 | 0.0e0 | 0 |
| 2000 m | -2.505e-2 | 0.0e0 | 0.0e0 | 0 |
| 20000 m | -7.905e-2 | 0.0e0 | 0.0e0 | 0 |

