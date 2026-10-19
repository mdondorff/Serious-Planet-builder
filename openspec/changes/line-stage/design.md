## Context
- Stamp items: kind + three parameter slots, a footprint, a derived content hash, one stage per layer keyed by chain with pass-through (WDEF-010, GEN-026/027, ADR 0067, 0068).
- Owner (2026-10-19): a line is one item with a points list; a line beyond the 40 km reach limit splits into linked items; the combination is max(ridge) + min(channel).
- ADR 0069 (proposed): item file schema 2, point lists, grouped parts.

## Decisions
1. **Item model.** `Item.points: Vec<(lat, lon)>` (empty for schema-1 items), `Item.group: Option<Ulid>`, `Item.part: Option<u32>`. `[params]` of a line: `amount_m`, `width_m`. The anchor equals the first point (the loader refuses a line whose `[anchor]` differs from `[[points]][0]`; the writer sets it).
2. **Schema.** `schema_version` 2 when the item has points, else 1. Keys: `group`, `part` (top level, after `notes`, only when set), then the `[anchor]` and `[params]` tables as before, then the `[[points]]` tables in order. Hash: the existing material, then (only for an item with points) the group (16 bytes or zero), the part, the point count and each point's two numbers as bit patterns; the schema version is already in the material, so a schema-1 item hashes as before.
3. **Stamp.** `Stamp.params = [amount_m, width_m, 0]`; `Stamp.points: Arc<[[f64; 3]]>` (unit vectors, converted at put); the stamp hash covers kind, params and the point bits (ADR 0067 point 1 extended). Stamps without points hash as before.
4. **Distance to the polyline** `d` (metres, `libm`, f64): the minimum over the segments `P[i]..P[i+1]` of the great-circle distance to the arc (the foot `p' = normalize(p - (p·n) n)` with `n = normalize(P[i] x P[i+1])`; on the arc when `(P[i] x p')·n >= 0` and `(p' x P[i+1])·n >= 0`, then `R atan2(|p·n|, |p - (p·n) n|)`, else `R min(angle(p, P[i]), angle(p, P[i+1]))`). `x = d / width_m`.
5. **Profiles** (monotone non-increasing in `x`, zero at `x >= 1`): ridge `amount (1 - x^2)^2`; channel `-amount bed(x)` with `bed = 1` for `x <= 0.3` and `1 - smooth((x - 0.3) / 0.7)` beyond (`smooth(t) = t^2 (3 - 2t)`).
6. **Combination.** Stamps in application order, then `max(0, max ridge delta) + min(0, min channel delta)` over the line items whose reach contains the point.
7. **Bounds.** For a cap with centre `q` and angle `a`: `d_near = max(0, dist(q, polyline) - R a)`, `d_far = dist(q, polyline) + R a`, with the stamps' few-ulp slack; the range is `[profile(x_far), profile(x_near)]` (ridge) or its negation (channel). Conservative.
8. **Reach and limits.** A line has 2 to 64 points; consecutive points differ; `reach_m` is at least `width_m` plus the largest distance of a point from the anchor; at most one cell edge (40 km). The command layer splits longer lines (E4); the world definition sees independent items.
9. **Preview.** E3's preview shader draws stamps only; line items show after their exact tiles arrive (the preview list skips line kinds and counts them).
10. **Layers.** A line item lives in any layer; the usual one is `splines`.

## Risks
- Cost: distance is O(points) per sample; 64 points per item is the cap; a tile reads only items whose footprint meets it.
- A smooth curve is a polyline with enough points: the editor chooses the density; arcs are additive later.
- A line through a pole or across the antimeridian: the math is in 3D unit vectors; a test covers both.
- Schema 2 is hard to reverse once the editor writes line files; schema 1 stays valid.
