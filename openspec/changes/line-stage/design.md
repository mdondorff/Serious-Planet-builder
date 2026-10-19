## Context
- Stamp items: kind + three parameter slots (two reserved at 0), a footprint, a derived content hash, one stage per layer keyed by chain with pass-through (WDEF-010, GEN-026/027, ADR 0067, 0068).
- A line must exceed one item's reach (40 km); the item file has scalar parameters only (AUTH-005). ADR 0069 (proposed) chooses a chain of straight segment items.

## Decisions
1. **Segment geometry.** Anchor `c` (unit vector), heading `h` (degrees clockwise from true north, as the anchor spells it), `s = half_length_m / R`. The tangent `t = cos h · north + sin h · east` at `c`; the endpoints `A = c cos s - t sin s`, `B = c cos s + t sin s` (the segment is the great-circle arc `A..B`). `north`/`east` are the local frame of the anchor's latitude/longitude (at a pole: longitude 0, as the file writer spells it).
2. **Parameter slots.** `Stamp.params = [amount_m, half_length_m, heading_deg]` for `ridge` and `channel`; the item file's keys are `amount_m` and `half_length_m` in `[params]`, and the heading is the file's `[anchor] heading` (the file mapping passes it in). Heading therefore sits in the stamp hash, so a rotated segment re-keys the tiles it touches. Half-width `w = reach_m - half_length_m`, at least 1 m.
3. **Distance to the segment** `d` (angle times R, f64, `libm`): with `n = normalize(A × B)`, the foot `p' = normalize(p - (p·n) n)`; if `p'` lies on the arc (`(A × p')·n >= 0` and `(p' × B)·n >= 0`) then `d = R · atan2(|p·n|, |p - (p·n) n|)`, else `d = R · min(angle(p,A), angle(p,B))`; `half_length_m = 0` gives `R · angle(p, c)`. `x = d / w`.
4. **Profiles** (monotone non-increasing in `x`, zero at `x >= 1`): ridge `amount · (1 - x²)²`; channel `-depth · bed(x)` with `bed = 1` for `x <= 0.3` and `1 - smooth((x - 0.3) / 0.7)` beyond (`smooth(t) = t²(3 - 2t)`). A channel has a flat bed and sloped banks, a ridge a rounded crest.
5. **Combination.** The stage applies the stamps (hill, crater, plateau) in application order first, then adds `max(0, max_ridge_delta) + min(0, min_channel_delta)` over the line items whose reach contains the point. Max and min are commutative, so overlapping segments of a chain (and chains crossing) do not depend on order and do not double at a joint.
6. **Bounds.** For a cap with centre `q` and angle `α`: `d_near = max(0, dist(q, seg) - Rα)`, `d_far = dist(q, seg) + Rα` (and the same few-ulp slack as the stamps). The delta range is `[profile(x_far), profile(x_near)]` (ridge) or its negation (channel). Conservative: the true farthest distance of the cap is at most `d_far`.
7. **Footprint.** The existing query returns the line item with its stamp (cap form and tile form); `reach_m` bounds the segment plus its half-width, so a tile whose cap plus halo is within `reach_m` of the anchor reads it (WDEF-007). The 40 km limit applies: `half_length_m + w <= 40 km`.
8. **Preview.** E3's preview shader draws stamps only; a line item shows after its exact tiles arrive (the preview list skips line kinds and reports that). A line preview is a later change.
9. **Layers.** A line item lives in any layer; the usual one is `splines`. The stage of the layer takes both stamp and line items.

## Open for the owner (regions, E5b; not decided here)
1. **"Mask other edits"**: does a region (a) protect its area (edits inside are ignored), (b) clip edits made inside it to the inside, or (c) only label an area? Recommendation: (c) for E5b; the mask semantics decided with the editor commands (E4/E6).
2. **Outline representation**: (a) disc/ellipse items (`semi_major_m`, `semi_minor_m`, heading), regions grown by several items with one `name`; (b) a closed chain of straight outline segments; (c) arrays in the item file (schema version 2). Recommendation: (a) now, (b) when the editor draws outlines.
3. **Surface types**: a data table in the world definition (as liquid kinds, status.md constraint 3), not a code enum. Which entries ship first (rock, soil, sand, snow, grass)?
4. **Debug overlay**: tint by the surface type of the region under the fragment. Disc/ellipse regions can be evaluated analytically per tile from a short region list in the frame plan (like the preview stamps); polygons need a mask texture per tile.

## Risks
- A smooth curve costs many short segments; arcs (a curvature parameter) are additive later.
- Conservative bounds from the cap centre widen tile bounds near a chain at low levels; the morph-interval check on the steep field (ADR 0066 point 5) is a test here.
- A segment that reaches the pole or crosses the antimeridian: the math is in 3D unit vectors, so neither is special; a test covers both.
