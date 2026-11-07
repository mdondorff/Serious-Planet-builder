# 0109 Basin infill of the base surface before the carve; what remains is a lake basin
Status: accepted (owner, 2026-11-07: D7 option a, "sounds plausible, try that approach")
Date: 2026-11-07
Source: owner, 2026-11-07; explorer probes on #1223 (seed 14, tile 2/12/10, 2026-11-06/07); ADR 0101; ADR 0105; ADR 0108

## Context
- With kept depressions (ADR 0101) 11.9 % of land on the probe tile lies in basins, depth p50 139 m, p95 467 m, deepest 710 m; lakes cover about 2-3 % of Earth's land.
- Most of that volume is basins of the base surface, not made by the carve.
- Pass-1 deposition fed by the carve's erosion (ADR 0105 point 1) does not fill them: at G = 1 and G = 4, with the ceiling at the spill or the flood level, pit share and depth grow (12.2-12.9 %, p50 139-145 m) and G = 4 piles sediment up to +1,193 m.
- On Earth most former basins are sediment-filled flats (valley floors, plains, fans); open basins are the exception.

## Decision
1. Before pass 1 an infill step fills the depressions of the base surface: small or shallow basins completely, larger ones up to a residual depth; the size, depth and residual thresholds are Earth-preset parameters (ADR 0108).
2. A basin left after the infill is a lake basin; its depth is the fill-depth channel (ADR 0101 point 2); in arid climates it stays a sink (ADR 0105 point 2).
3. Pass-1 deposition (ADR 0105 point 1) stays for fans, deltas and valley floors; its check is mass balance and flatness, not pit depth.
4. Routing over a filled copy and carving the unfilled heights (ADR 0101 point 1) stay.
5. The check, on the preview: lake share of land against Earth's 2-3 %, basin depth per class against GLO-30, relief against the keep-pits surface.

## Consequences
- Amends ADR 0101 (depressions are kept after the infill, not from the raw base) and ADR 0105 point 1 (deposition no longer fills basins); bake kernel versions change.
- The change `pass1-deposition` (#1223) carries the infill; its deposition code is reused.
- The thresholds are tuned in the preview round (I4b).
