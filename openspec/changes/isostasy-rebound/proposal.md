## Why
Owner 2026-11-05 (planner inbox #72 item 1): eroded mass unloads the crust and a flexural response raises the column. Held on 2026-11-05 (planner inbox #74): no code, no further runs; the planner recommends dropping the rebound from M9 and the owner decides. This note keeps the measurement.

## Probe
- Throwaway: a rebound in pass 1's carve: after each step the mass removed (`max(0, z_before - z_after)`) is smoothed by a Gaussian of 61 cells (50 km, three box passes) and 0.82 of it is added back to the non-outlet cells. Seed 14, tile 2/12/10, production grid; pass 1 of face 2 took 1,942 s.
- Flexural parameter for the width: `alpha = (E Te^3 / (12 (1 - nu^2) rho_m g))^(1/4)`, about 23 km at `Te` 10 km and 46 km at 25 km.

## Result (relief on the spike's measure, p95 - p5 in blocks at least 95 % land and 90 % one class; p50)
| Measure | Without | With |
|---|---|---|
| 3 km, mountains | 534 m | 540 m |
| 3 km, high mountains | 695.2 m | 694.8 m |
| 10 km, mountains | 1,215 m | 1,226 m |
| 10 km, high mountains | 1,545 m | 1,545 m |

| Elevation, land cells | Without p50 / p95 / max | With p50 / p95 / max |
|---|---|---|
| plain | 104 / 791 / 1,321 m | 166 / 927 / 1,510 m |
| mountains | 1,841 / 3,303 / 4,515 m | 2,085 / 3,571 / 4,789 m |
| high mountains | 2,899 / 4,281 / 5,215 m | 3,192 / 4,571 / 5,505 m |

- The relief does not change (the filled surface keeps 519 m, 0.96 of the Alps', and 675 m, 0.84 of Everest's); the land rises by about 0.82 of the regional mean erosion (+244 and +293 m at the median, +290 m at the peaks): a base re-tune in effect.
- Separate finding for the tuning round: the elevations are already above GLO-30's (high-mountain p50 2,899 m against 2,157 m on the Everest tile; mountains 1,841 m against 1,076 m on the Alps tile).
- Not measured: other widths (the local limit would lower the relief, wider is the same uplift).

## Decision (owner, 2026-11-05)
Dropped from M9. Kept as a known improvement candidate; no code until a trigger.

## The probe against a fuller algorithm
| Aspect | Probe (measured) | Fuller flexural isostasy |
|---|---|---|
| Load | mass removed by erosion only | erosion unloading, deposition loading (deltas, forelands), topographic load of belts, sea-water load |
| Response | 0.82 of the load (density ratio crust/mantle), smoothed by a fixed Gaussian | thin elastic plate: `D grad^4 w + (rho_m - rho_infill) g w = q`, solved spectrally per face or by iteration |
| Plate strength | one width (50 km) | effective elastic thickness `Te` as a field: low in rifts and young belts (5-15 km), high in cratons (50-100 km) |
| Time | instantaneous each step | viscous relaxation (a time constant per step) |
| Scope | pass 1 only, non-outlet cells | both passes; cube-face seams need a joined solve |

Landforms the fuller version adds that the probe cannot: foreland basins beside belts (flexural moats), peripheral bulges, raised rift shoulders and passive-margin escarpments, subsidence under deltas and sediment basins. Relief at 3 km is expected to stay about the same; the gain is in regional landforms and hypsometry.

Triggers: tuning hypsometry or base elevation against GLO-30, or work on rifts, escarpments or foreland basins. Cost then: a per-face spectral solve (a re-bake of pass 1), plus a `Te` field derived from the lithology provinces (GEN-116).
