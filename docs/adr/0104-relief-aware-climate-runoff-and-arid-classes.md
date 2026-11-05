# 0104 A relief-aware climate: wind, two-season orographic rain, runoff in the carve, arid class rules
Status: accepted (owner, 2026-11-05: SPIKE-102 D3 option A, "Full d3", with the two-season sweep and the vegetation curve from the D3 review); point 3 amended 2026-11-05 (independent review): runoff enters the carve as runoff-weighted discharge (sum of runoff times area), not as a local scaling of erodibility; the vegetation curve stays a local factor; the precipitation sweep is a fixed-iteration advection over a class-independent base
Date: 2026-11-05
Source: owner, 2026-11-05; SPIKE-102 (`spikes/generator-archetypes/results/results.md` sections 2, 3, 4.1 rows 5-7, 4.3 D3); SPIKE-81 (PET, class shares); Smith and Barstad 2004 (linear orographic precipitation); ADR 0089; ADR 0102; ADR 0103; GEN-117

## Context
- Precipitation is latitude bands times noise; it ignores relief, so no range has a wet and a dry flank and no rain shadow exists.
- The carve ignores rainfall; badlands and sand sea have no class rule and cover 0 % of land.
- Arid lands are about 30-40 % of Earth's ice-free land; monsoon flow decides which flank of many tropical ranges is wet.
- Rainfall in the carve and class rules change the heights erosion sees; adding them after the planet bake means a re-bake and a second tuning.

## Decision
1. A prevailing wind field by latitude band (trades, westerlies, polar easterlies) with noise and a steadiness value, analytic.
2. Precipitation comes from a linear orographic model (Smith and Barstad 2004) swept upwind on a coarse lattice in a fixed order, f64, for two seasons: the bands shifted by the seasonal migration and a land-sea heating term onshore in summer and offshore in winter; the seasons are summed; aridity and seasonality are outputs.
3. The carve scales its erodibility by runoff from that precipitation through a vegetation curve that peaks in semi-arid climates.
4. Badlands and sand sea get default class rules from soft rock, aridity, height, relief and sand supply, through the class-weight interface (ADR 0089).
5. Planet maps of precipitation (annual wind against two seasons), aridity and wind, material colours at the preview regions and the class map with shares against Earth come before any bake that uses them; values come from that preview round.

## Consequences
- The global field version changes; stores baked before it are refused (owner, 2026-11-05: older stores need not stay compatible).
- Material colours that read precipitation change planet-wide (the owner's look, decided on the preview).
- Cold-current coastal deserts, weather and dune shapes stay out; glacial accumulation, playas and dunes can read these fields later.
