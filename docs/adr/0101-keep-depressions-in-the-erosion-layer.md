# 0101 Keep depressions in the erosion layer; the fill depth is a stored channel
Status: accepted (owner, 2026-11-05: SPIKE-102 D1, "the most rational"); built by GEN-120, GEN-121 and STRM-076/077: a post-pass fills each basin to one planet-wide spill level across tile and cube edges, from bake-scratch heights; the code (decision 4, owner 2026-11-05): fill depth u16 in 0.25 m steps against the exact base, clamped at 0; amended by ADR 0109 (owner, 2026-11-07): an infill of the base surface runs first, depressions are those left after it
Date: 2026-11-05
Source: owner, 2026-11-05; SPIKE-101; SPIKE-102 (`spikes/generator-archetypes/results/results.md` sections 4.1, 4.3, 6, 8); ADR 0088 point 4; ADR 0099; GEN-120, STRM-076; change `keep-pits` (#1192)

## Context
- Both passes of the erosion layer fill every depression before they route and carve the filled heights; each step raises enclosed valley floors to their spill level.
- With the SPIKE-101 pair (breach 500 m, ridged band), high mountains reach 0.62 of Everest's 3 km relief and mountains 0.86 of the Alps'.
- Carving the unfilled heights while routing over a filled copy reaches 0.85 and 0.94; the filled surface keeps 0.84 (SPIKE-102 section 6).
- The kept depressions are about 10x deeper than real mountain depressions (p50 63 m against 3-7 m); they sit in ravine floors (section 8).
- Lakes, kettles, dolines, calderas and endorheic basins need the depression depth; adding it after the planet bake means a full re-bake.

## Decision
1. Both passes route over a filled copy of the heights and carve the unfilled heights; depressions that survive the breach stay.
2. The fill depth (filled minus unfilled height) is a stored channel next to the drainage area and the deposited depth.
3. The runtime adds the fill depth back, so the surface shows filled basins until lake water and a per-archetype pit tolerance exist.
4. The channel's quantisation is a key parameter; its code is decided before the planet bake.

## Consequences
- Extends ADR 0088 point 4 (as amended by ADR 0099): the carve keeps depressions.
- Bake kernel and channel versions change; stores baked before it are refused by their keys.
- Lake water and fill-as-material rendering (shallow fill as wet ground, deep fill as sediment flats; owner, 2026-11-05) need no re-bake.
- Seams of the fill depth across tile and cube edges and the flatness of the filled surface are measured on the preview before the bake.
