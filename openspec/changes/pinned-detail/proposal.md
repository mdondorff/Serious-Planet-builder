## Why
- ADR 0122 (accepted, owner 2026-11-13), point 1: where the erosion layer is read, the fine-detail term (ADR 0098's crease octaves) is pinned to the stored samples, so the drawn ground equals the stored B + L at every cell centre.
- Under `--commit` the drawn ground differed from the stored B + L by p50 -2.1 m (min -7.4, max +2.0 m) on R3a, and the whole difference was this term (VIEW-072's probe); on a river line it drew a rising bed (flight finding F2).

## What changes
- GEN-187: where the layer is read the detail is the detail minus its bilinear interpolant from the four layer cell centres around the point, exactly 0 at a centre. The bounds take the detail over the cap widened by two layer cells for the interpolant; the planet-wide bound grows by the detail's span. `PINNED_DETAIL_VERSION` enters the keys of every field that reads the layer.
- Tests: the ground at every cell centre is the base plus the faded change bit for bit; the pinned detail between the centres; three GEN-112/113 tests compare against a layer of zeros (the same pinned detail) instead of the band's field.

## Not in this change
- ADR 0122 point 2 (detail 0 on a river line's trench): waits for crusader's REND-067 line placement.
- Where no layer is read the field is unchanged (ADR 0098 points 2 and 3).

## Impact
generators (erosion.rs, erosion_layer.rs, tests/runtime_layer.rs). Lit goldens over the layer move: golden-review as a declared change. Spec: generator-pipeline (GEN-187 added).
