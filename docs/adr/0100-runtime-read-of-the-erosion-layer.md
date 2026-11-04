# 0100 Runtime read of the erosion layer: C1, clamped, with a coast fade
Status: accepted (owner, 2026-11-04: the erosion remedies "lgtm", including the runtime C1 read and the coast fade); point 4 amended 2026-11-04: the read needs no relief channels; the layer sits on the field-bake lattice read C1 at k = 1 (SPIKE-99, owner B; #1158)
Date: 2026-11-04
Source: owner, 2026-11-03 and 2026-11-04; ADR 0088 point 5; ADR 0099; erosion preview and diagnosis (explorer, 2026-11-03); GEN-112 to GEN-114; change `runtime-layer-read` (#1153)

## Context
- ADR 0088 point 5 adds the layer to the runtime field bilinearly.
- A bilinear (C0) read of the 101.8 m layer leaves strips and checkers of one cell in lit views (erosion preview, 2026-11-03); a Catmull-Rom read removes them.
- An unclamped Catmull-Rom read overshoots by up to 7.4 % of a step (levees, sea-floor dips; ADR 0099).
- The layer moves the 0-3 m coastal plain across the beach material band, which leaves streaks and green patches along coasts.
- Stored tiles have no halo, so each face's read steps by about one cell of change at the cube edges.

## Decision
1. The runtime field reads the layer's change by Catmull-Rom (C1), clamped to the range of the four surrounding cells, and adds it to the relief base unscaled by k.
2. The change fades to 0 within 3 m of elevation above the sea level (smoothstep of the base's height above the definition's sea level).
3. Across a cube edge the two faces' reads are cross-faded over 2 cells with the bake's own weights (GEN-100).
4. The read is behind the relief switch and calls the crease band of ADR 0098.

## Consequences
- Amends ADR 0088 point 5 (bilinear becomes C1, clamped; the coast fade and the edge cross-fade are added).
- The bounds take the layer's min/max pyramid (GEN-102); built when a tile is decoded until the layer stores it (STRM-066).
- The cross-fade of decision 3 can go if the layer ever stores a halo.
