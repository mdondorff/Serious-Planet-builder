# 0106 Minimal glacial erosion in pass 1, behind a switch and a gate
Status: accepted (owner, 2026-11-05: "All four into m9, glaciers with a gate", the independent review's item 3)
Date: 2026-11-05
Source: owner, 2026-11-05; independent review of the M9 bake plan (2026-11-05); SPIKE-102 (F5, P1c); Deal and Prasicek 2021 (sliding ice incision model); Brozovic et al. 1997 (glacial buzzsaw); Harbor 1992; ADR 0088 point 4; ADR 0104

## Context
- Alpine relief, fjords and the glaciated reference sites are largely glacial; deep, flat-floored valleys and an elevation mode near the snow line are missing.
- 3 km relief is already 0.84-0.96 of the references without glaciers; adding them after the bake would overshoot and force a second tuning of class amplitudes, scorecard and materials.
- Full ice-flow models are iterative and too heavy for the bake; a stream-power-like ice law reuses pass 1's solver and receivers.
- The ice extent needs the relief-aware precipitation of ADR 0104.

## Decision
1. An ice-history field: the glacial-maximum equilibrium line from temperature minus about 6 degrees C and the new precipitation; ice extent where ground lies above it, plus continental sheets.
2. Pass 1 adds glacial erosion with the sliding ice incision model: ice discharge accumulated from a capped mass balance over the routing; erosion on every ice-covered cell; erosion below sea level allowed under ice.
3. It is behind a switch. A gate, time-boxed to about two lane-days, compares valley cross-sections, floor width and hypsometry against the glaciated GLO-30 tiles (Sognefjord, Highlands, Alps); if it fails, the planet bake runs with the switch off.
4. Pass-2 U-shaped cross-sections, cirques and moraines are deferred; they need a pass-2 re-run of ice tiles only.

## Consequences
- Extends ADR 0088 point 4; bake cost rises by about 6-12 thread-h (SPIKE-102 estimate).
- The switch off reproduces the bake without glaciers.
- Present-day ice and snow materials stay runtime matters.
